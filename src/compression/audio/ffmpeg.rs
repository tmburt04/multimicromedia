use crate::error::{CompressionError, Result};
use js_sys::{Array, Object, Uint8Array};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

/// Tries to get the __ffmpeg__ object from the global scope.
/// Works in both browser (window) and Node.js (global/globalThis).
fn get_ffmpeg_object() -> Option<JsValue> {
    let global = js_sys::global();

    // Try globalThis.__ffmpeg__ first (works in modern browsers and Node.js)
    if let Ok(ffmpeg) = js_sys::Reflect::get(&global, &"__ffmpeg__".into()) {
        if !ffmpeg.is_undefined() && !ffmpeg.is_null() {
            return Some(ffmpeg);
        }
    }

    // Try window.__ffmpeg__ (browser fallback)
    if let Ok(window) = js_sys::Reflect::get(&global, &"window".into()) {
        if !window.is_undefined() && !window.is_null() {
            if let Ok(ffmpeg) = js_sys::Reflect::get(&window, &"__ffmpeg__".into()) {
                if !ffmpeg.is_undefined() && !ffmpeg.is_null() {
                    return Some(ffmpeg);
                }
            }
        }
    }

    // Try global.__ffmpeg__ (Node.js fallback)
    if let Ok(node_global) = js_sys::Reflect::get(&global, &"global".into()) {
        if !node_global.is_undefined() && !node_global.is_null() {
            if let Ok(ffmpeg) = js_sys::Reflect::get(&node_global, &"__ffmpeg__".into()) {
                if !ffmpeg.is_undefined() && !ffmpeg.is_null() {
                    return Some(ffmpeg);
                }
            }
        }
    }

    None
}

pub fn is_ffmpeg_available() -> bool {
    let ffmpeg = match get_ffmpeg_object() {
        Some(f) => f,
        None => return false,
    };

    // Check if isAvailable function exists and returns true
    if let Ok(is_available) = js_sys::Reflect::get(&ffmpeg, &"isAvailable".into()) {
        if is_available.is_function() {
            if let Ok(result) =
                js_sys::Reflect::apply(is_available.unchecked_ref(), &ffmpeg, &Array::new())
            {
                return result.as_bool().unwrap_or(false);
            }
        }
    }

    false
}

pub async fn execute_ffmpeg(input: &[u8], args: &[String]) -> Result<Vec<u8>> {
    let ffmpeg = get_ffmpeg_object().ok_or(CompressionError::FfmpegUnavailable)?;

    let execute_fn = js_sys::Reflect::get(&ffmpeg, &"execute".into())
        .map_err(|_| CompressionError::FfmpegUnavailable)?;

    if !execute_fn.is_function() {
        return Err(CompressionError::FfmpegUnavailable);
    }

    // Convert args to JS array
    let js_args = Array::new();
    for arg in args {
        js_args.push(&arg.into());
    }

    // Convert input to Uint8Array
    let input_array = Uint8Array::from(input);

    // Build call arguments
    let call_args = Array::new();
    call_args.push(&js_args);
    call_args.push(&input_array);

    // Execute
    let promise = js_sys::Reflect::apply(
        execute_fn.unchecked_ref(),
        &ffmpeg,
        &call_args,
    )
    .map_err(|e| CompressionError::FfmpegFailed {
        detail: format!("failed to call execute: {:?}", e),
    })?;

    let result = JsFuture::from(js_sys::Promise::from(promise))
        .await
        .map_err(|e| CompressionError::FfmpegFailed {
            detail: format!("execution failed: {:?}", e),
        })?;

    // Check for error
    if let Ok(error) = js_sys::Reflect::get(&result, &"error".into()) {
        if !error.is_undefined() && !error.is_null() {
            return Err(CompressionError::FfmpegFailed {
                detail: error.as_string().unwrap_or_else(|| "unknown error".to_string()),
            });
        }
    }

    // Get output data
    let output = js_sys::Reflect::get(&result, &"data".into())
        .map_err(|_| CompressionError::FfmpegFailed {
            detail: "missing output data".to_string(),
        })?;

    let output_array = output.dyn_ref::<Uint8Array>()
        .ok_or_else(|| CompressionError::FfmpegFailed {
            detail: "invalid output format".to_string(),
        })?;

    Ok(output_array.to_vec())
}

pub fn build_ffmpeg_input_file(data: &[u8], filename: &str) -> JsValue {
    let obj = Object::new();
    let _ = js_sys::Reflect::set(&obj, &"name".into(), &filename.into());
    let _ = js_sys::Reflect::set(&obj, &"data".into(), &Uint8Array::from(data));
    obj.into()
}
