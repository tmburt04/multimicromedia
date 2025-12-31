use super::{StorageHandle, StorageType};
use crate::error::{CompressionError, Result};
use js_sys::{Object, Uint8Array};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

pub struct OpfsStorage;

impl OpfsStorage {
    pub async fn store(data: &[u8]) -> Result<StorageHandle> {
        let id = format!("compress_{}", js_sys::Date::now() as u64);
        
        // Get OPFS root directory
        let root = Self::get_root().await?;
        
        // Create file
        let options = Object::new();
        js_sys::Reflect::set(&options, &"create".into(), &true.into())
            .map_err(|_| CompressionError::StorageUnavailable {
                reason: "failed to set options".to_string(),
            })?;
        
        let file_handle_promise = js_sys::Reflect::get(&root, &"getFileHandle".into())
            .and_then(|f| {
                let func = f.dyn_ref::<js_sys::Function>().ok_or(JsValue::NULL)?;
                func.call2(&root, &id.clone().into(), &options)
            })
            .map_err(|_| CompressionError::StorageUnavailable {
                reason: "failed to get file handle".to_string(),
            })?;
        
        let file_handle = JsFuture::from(js_sys::Promise::from(file_handle_promise))
            .await
            .map_err(|_| CompressionError::StorageUnavailable {
                reason: "failed to create file".to_string(),
            })?;
        
        // Create writable stream
        let writable_promise = js_sys::Reflect::get(&file_handle, &"createWritable".into())
            .and_then(|f| {
                let func = f.dyn_ref::<js_sys::Function>().ok_or(JsValue::NULL)?;
                func.call0(&file_handle)
            })
            .map_err(|_| CompressionError::StorageUnavailable {
                reason: "failed to create writable".to_string(),
            })?;
        
        let writable = JsFuture::from(js_sys::Promise::from(writable_promise))
            .await
            .map_err(|_| CompressionError::StorageUnavailable {
                reason: "failed to get writable stream".to_string(),
            })?;
        
        // Write data
        let array = Uint8Array::from(data);
        let write_promise = js_sys::Reflect::get(&writable, &"write".into())
            .and_then(|f| {
                let func = f.dyn_ref::<js_sys::Function>().ok_or(JsValue::NULL)?;
                func.call1(&writable, &array)
            })
            .map_err(|_| CompressionError::IoError {
                detail: "failed to write data".to_string(),
            })?;
        
        JsFuture::from(js_sys::Promise::from(write_promise))
            .await
            .map_err(|_| CompressionError::IoError {
                detail: "write failed".to_string(),
            })?;
        
        // Close stream
        let close_promise = js_sys::Reflect::get(&writable, &"close".into())
            .and_then(|f| {
                let func = f.dyn_ref::<js_sys::Function>().ok_or(JsValue::NULL)?;
                func.call0(&writable)
            })
            .map_err(|_| CompressionError::IoError {
                detail: "failed to close stream".to_string(),
            })?;
        
        JsFuture::from(js_sys::Promise::from(close_promise))
            .await
            .map_err(|_| CompressionError::IoError {
                detail: "close failed".to_string(),
            })?;
        
        Ok(StorageHandle {
            storage_type: StorageType::Opfs,
            data: Vec::new(), // Data is stored externally
            id,
        })
    }
    
    pub async fn retrieve(id: &str) -> Result<Vec<u8>> {
        let root = Self::get_root().await?;
        
        // Get file handle
        let file_handle_promise = js_sys::Reflect::get(&root, &"getFileHandle".into())
            .and_then(|f| {
                let func = f.dyn_ref::<js_sys::Function>().ok_or(JsValue::NULL)?;
                func.call1(&root, &id.into())
            })
            .map_err(|_| CompressionError::IoError {
                detail: "file not found".to_string(),
            })?;
        
        let file_handle = JsFuture::from(js_sys::Promise::from(file_handle_promise))
            .await
            .map_err(|_| CompressionError::IoError {
                detail: "failed to get file handle".to_string(),
            })?;
        
        // Get file
        let file_promise = js_sys::Reflect::get(&file_handle, &"getFile".into())
            .and_then(|f| {
                let func = f.dyn_ref::<js_sys::Function>().ok_or(JsValue::NULL)?;
                func.call0(&file_handle)
            })
            .map_err(|_| CompressionError::IoError {
                detail: "failed to get file".to_string(),
            })?;
        
        let file = JsFuture::from(js_sys::Promise::from(file_promise))
            .await
            .map_err(|_| CompressionError::IoError {
                detail: "failed to read file".to_string(),
            })?;
        
        // Read as ArrayBuffer
        let buffer_promise = js_sys::Reflect::get(&file, &"arrayBuffer".into())
            .and_then(|f| {
                let func = f.dyn_ref::<js_sys::Function>().ok_or(JsValue::NULL)?;
                func.call0(&file)
            })
            .map_err(|_| CompressionError::IoError {
                detail: "failed to read buffer".to_string(),
            })?;
        
        let buffer = JsFuture::from(js_sys::Promise::from(buffer_promise))
            .await
            .map_err(|_| CompressionError::IoError {
                detail: "failed to get array buffer".to_string(),
            })?;
        
        let array = Uint8Array::new(&buffer);
        Ok(array.to_vec())
    }
    
    pub async fn delete(id: &str) -> Result<()> {
        let root = Self::get_root().await?;
        
        let options = Object::new();
        js_sys::Reflect::set(&options, &"recursive".into(), &false.into())
            .map_err(|_| CompressionError::IoError {
                detail: "failed to set options".to_string(),
            })?;
        
        let remove_promise = js_sys::Reflect::get(&root, &"removeEntry".into())
            .and_then(|f| {
                let func = f.dyn_ref::<js_sys::Function>().ok_or(JsValue::NULL)?;
                func.call2(&root, &id.into(), &options)
            })
            .map_err(|_| CompressionError::IoError {
                detail: "failed to remove entry".to_string(),
            })?;
        
        JsFuture::from(js_sys::Promise::from(remove_promise))
            .await
            .map_err(|_| CompressionError::IoError {
                detail: "failed to delete file".to_string(),
            })?;
        
        Ok(())
    }
    
    async fn get_root() -> Result<JsValue> {
        let global = js_sys::global();
        
        let navigator = js_sys::Reflect::get(&global, &"navigator".into())
            .map_err(|_| CompressionError::StorageUnavailable {
                reason: "navigator not available".to_string(),
            })?;
        
        let storage = js_sys::Reflect::get(&navigator, &"storage".into())
            .map_err(|_| CompressionError::StorageUnavailable {
                reason: "storage not available".to_string(),
            })?;
        
        let get_dir = js_sys::Reflect::get(&storage, &"getDirectory".into())
            .map_err(|_| CompressionError::StorageUnavailable {
                reason: "OPFS not available".to_string(),
            })?;
        
        let func = get_dir.dyn_ref::<js_sys::Function>()
            .ok_or_else(|| CompressionError::StorageUnavailable {
                reason: "getDirectory is not a function".to_string(),
            })?;
        
        let promise = func.call0(&storage)
            .map_err(|_| CompressionError::StorageUnavailable {
                reason: "failed to call getDirectory".to_string(),
            })?;
        
        JsFuture::from(js_sys::Promise::from(promise))
            .await
            .map_err(|_| CompressionError::StorageUnavailable {
                reason: "failed to get OPFS root".to_string(),
            })
    }
}
