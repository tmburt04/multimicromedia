use crate::error::{CompressionError, Result};
use js_sys::{Array, Object, Uint8Array};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

// js_sys::global resolves the host global in browsers, workers and Node.
fn get_ffmpeg_object() -> Option<JsValue> {
    js_sys::Reflect::get(&js_sys::global(), &"__ffmpeg__".into())
        .ok()
        .filter(|value| !value.is_undefined() && !value.is_null())
}

pub fn is_ffmpeg_available() -> bool {
    let ffmpeg = match get_ffmpeg_object() {
        Some(f) => f,
        None => return false,
    };

    if let Ok(is_available) = js_sys::Reflect::get(&ffmpeg, &"isAvailable".into()) {
        if is_available.is_function() {
            if let Ok(result) = is_available
                .unchecked_ref::<js_sys::Function>()
                .call0(&ffmpeg)
            {
                return result.as_bool().unwrap_or(false);
            }
        }
    }

    false
}

/// Ask an installed bridge about its output-option rules and optional codec inventory.
/// Custom bridges without this hook keep the original execution-only contract.
pub(crate) fn validate_ffmpeg_args(_args: &[String]) -> Result<()> {
    #[cfg(target_arch = "wasm32")]
    {
        let Some(ffmpeg) = get_ffmpeg_object() else {
            return Ok(());
        };
        let Ok(validate) = js_sys::Reflect::get(&ffmpeg, &"validateArgs".into()) else {
            return Ok(());
        };
        if !validate.is_function() {
            return Ok(());
        }
        let args: Array = _args.iter().map(JsValue::from).collect();
        let value = validate
            .unchecked_ref::<js_sys::Function>()
            .call1(&ffmpeg, &args)
            .map_err(|error| CompressionError::InvalidConfig {
                field: "ffmpeg".into(),
                reason: crate::error::js_error_message(&error),
            })?;
        if let Some(reason) = value.as_string() {
            return Err(CompressionError::InvalidConfig {
                field: "ffmpeg".into(),
                reason,
            });
        }
    }
    Ok(())
}

pub async fn execute_ffmpeg(input: &[u8], args: &[String]) -> Result<Vec<u8>> {
    if input.is_empty() {
        return Err(CompressionError::InvalidInput {
            reason: "FFmpeg input must not be empty".into(),
        });
    }
    let ffmpeg = get_ffmpeg_object().ok_or(CompressionError::FfmpegUnavailable)?;

    let execute_fn = js_sys::Reflect::get(&ffmpeg, &"execute".into())
        .map_err(|_| CompressionError::FfmpegUnavailable)?;

    if !execute_fn.is_function() {
        return Err(CompressionError::FfmpegUnavailable);
    }

    let js_args = Array::new();
    for arg in args {
        js_args.push(&arg.into());
    }

    let input_array = Uint8Array::from(input);

    let promise = execute_fn
        .unchecked_ref::<js_sys::Function>()
        .call2(&ffmpeg, &js_args, &input_array)
        .map_err(|e| CompressionError::FfmpegFailed {
            detail: format!("execute: {}", crate::error::js_error_message(&e)),
        })?;

    let result = JsFuture::from(js_sys::Promise::resolve(&promise))
        .await
        .map_err(|e| CompressionError::FfmpegFailed {
            detail: crate::error::js_error_message(&e),
        })?;

    if let Ok(error) = js_sys::Reflect::get(&result, &"error".into()) {
        if !error.is_undefined() && !error.is_null() {
            return Err(CompressionError::FfmpegFailed {
                detail: crate::error::js_error_message(&error),
            });
        }
    }

    let output = js_sys::Reflect::get(&result, &"data".into()).map_err(|_| {
        CompressionError::FfmpegFailed {
            detail: "missing output data".to_string(),
        }
    })?;

    let output_array =
        output
            .dyn_ref::<Uint8Array>()
            .ok_or_else(|| CompressionError::FfmpegFailed {
                detail: "invalid output format".to_string(),
            })?;

    if output_array.length() == 0 {
        return Err(CompressionError::FfmpegFailed {
            detail: "empty output".into(),
        });
    }
    let output_len = output_array.length() as usize;
    let mut data = Vec::new();
    data.try_reserve_exact(output_len)
        .map_err(|_| CompressionError::MemoryLimitExceeded)?;
    data.resize(output_len, 0);
    output_array.copy_to(&mut data);
    Ok(data)
}

pub fn build_ffmpeg_input_file(data: &[u8], filename: &str) -> JsValue {
    let obj = Object::new();
    let _ = js_sys::Reflect::set(&obj, &"name".into(), &filename.into());
    let _ = js_sys::Reflect::set(&obj, &"data".into(), &Uint8Array::from(data));
    obj.into()
}
