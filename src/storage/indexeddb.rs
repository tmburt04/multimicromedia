use super::{StorageHandle, StorageType};
use crate::error::{CompressionError, Result};
use js_sys::{Object, Uint8Array};
use wasm_bindgen::prelude::*;

const DB_NAME: &str = "compression_wasm_storage";
const STORE_NAME: &str = "files";
const DB_VERSION: u32 = 1;

pub struct IndexedDbStorage;

impl IndexedDbStorage {
    pub async fn store(data: &[u8]) -> Result<StorageHandle> {
        let id = format!("compress_{}", js_sys::Date::now() as u64);
        let db = Self::open_db().await?;
        
        // Start transaction
        let tx = db
            .dyn_ref::<web_sys::IdbDatabase>()
            .ok_or_else(|| CompressionError::StorageUnavailable {
                reason: "invalid database handle".to_string(),
            })?
            .transaction_with_str_and_mode(STORE_NAME, web_sys::IdbTransactionMode::Readwrite)
            .map_err(|_| CompressionError::StorageUnavailable {
                reason: "failed to start transaction".to_string(),
            })?;
        
        let store = tx.object_store(STORE_NAME).map_err(|_| CompressionError::StorageUnavailable {
            reason: "failed to get object store".to_string(),
        })?;
        
        // Create record
        let array = Uint8Array::from(data);
        let record = Object::new();
        js_sys::Reflect::set(&record, &"id".into(), &id.clone().into())
            .map_err(|_| CompressionError::IoError {
                detail: "failed to set id".to_string(),
            })?;
        js_sys::Reflect::set(&record, &"data".into(), &array)
            .map_err(|_| CompressionError::IoError {
                detail: "failed to set data".to_string(),
            })?;
        
        let request = store.put(&record).map_err(|_| CompressionError::IoError {
            detail: "failed to put data".to_string(),
        })?;
        
        Self::await_request(&request).await?;
        
        Ok(StorageHandle {
            storage_type: StorageType::IndexedDb,
            data: Vec::new(),
            id,
        })
    }
    
    pub async fn retrieve(id: &str) -> Result<Vec<u8>> {
        let db = Self::open_db().await?;
        
        let tx = db
            .dyn_ref::<web_sys::IdbDatabase>()
            .ok_or_else(|| CompressionError::StorageUnavailable {
                reason: "invalid database handle".to_string(),
            })?
            .transaction_with_str(STORE_NAME)
            .map_err(|_| CompressionError::IoError {
                detail: "failed to start transaction".to_string(),
            })?;
        
        let store = tx.object_store(STORE_NAME).map_err(|_| CompressionError::IoError {
            detail: "failed to get object store".to_string(),
        })?;
        
        let request = store.get(&id.into()).map_err(|_| CompressionError::IoError {
            detail: "failed to get data".to_string(),
        })?;
        
        let result = Self::await_request(&request).await?;
        
        if result.is_undefined() || result.is_null() {
            return Err(CompressionError::IoError {
                detail: "file not found".to_string(),
            });
        }
        
        let data = js_sys::Reflect::get(&result, &"data".into())
            .map_err(|_| CompressionError::IoError {
                detail: "failed to get data field".to_string(),
            })?;
        
        let array = data.dyn_ref::<Uint8Array>()
            .ok_or_else(|| CompressionError::IoError {
                detail: "invalid data format".to_string(),
            })?;
        
        Ok(array.to_vec())
    }
    
    pub async fn delete(id: &str) -> Result<()> {
        let db = Self::open_db().await?;
        
        let tx = db
            .dyn_ref::<web_sys::IdbDatabase>()
            .ok_or_else(|| CompressionError::StorageUnavailable {
                reason: "invalid database handle".to_string(),
            })?
            .transaction_with_str_and_mode(STORE_NAME, web_sys::IdbTransactionMode::Readwrite)
            .map_err(|_| CompressionError::IoError {
                detail: "failed to start transaction".to_string(),
            })?;
        
        let store = tx.object_store(STORE_NAME).map_err(|_| CompressionError::IoError {
            detail: "failed to get object store".to_string(),
        })?;
        
        let request = store.delete(&id.into()).map_err(|_| CompressionError::IoError {
            detail: "failed to delete data".to_string(),
        })?;
        
        Self::await_request(&request).await?;
        
        Ok(())
    }
    
    async fn open_db() -> Result<JsValue> {
        let global = js_sys::global();
        
        let indexed_db = js_sys::Reflect::get(&global, &"indexedDB".into())
            .map_err(|_| CompressionError::StorageUnavailable {
                reason: "IndexedDB not available".to_string(),
            })?;
        
        let idb = indexed_db.dyn_ref::<web_sys::IdbFactory>()
            .ok_or_else(|| CompressionError::StorageUnavailable {
                reason: "invalid IndexedDB factory".to_string(),
            })?;
        
        let request = idb.open_with_u32(DB_NAME, DB_VERSION)
            .map_err(|_| CompressionError::StorageUnavailable {
                reason: "failed to open database".to_string(),
            })?;
        
        // Set up upgrade handler
        let onupgrade = Closure::wrap(Box::new(move |event: web_sys::Event| {
            let target = event.target().unwrap();
            let request: web_sys::IdbOpenDbRequest = target.dyn_into().unwrap();
            let db: web_sys::IdbDatabase = request.result().unwrap().dyn_into().unwrap();
            
            if !db.object_store_names().contains(STORE_NAME) {
                let params = web_sys::IdbObjectStoreParameters::new();
                let key_path: JsValue = "id".into();
                params.set_key_path(&key_path);
                let _ = db.create_object_store_with_optional_parameters(STORE_NAME, &params);
            }
        }) as Box<dyn FnMut(_)>);
        
        request.set_onupgradeneeded(Some(onupgrade.as_ref().unchecked_ref()));
        onupgrade.forget();
        
        Self::await_request(&request).await
    }
    
    async fn await_request(request: &web_sys::IdbRequest) -> Result<JsValue> {
        let (tx, rx) = futures::channel::oneshot::channel();
        
        let tx_success = std::rc::Rc::new(std::cell::RefCell::new(Some(tx)));
        let tx_error = tx_success.clone();
        
        let onsuccess = Closure::wrap(Box::new(move |event: web_sys::Event| {
            let target = event.target().unwrap();
            let request: web_sys::IdbRequest = target.dyn_into().unwrap();
            if let Some(tx) = tx_success.borrow_mut().take() {
                let _ = tx.send(Ok(request.result().unwrap_or(JsValue::UNDEFINED)));
            }
        }) as Box<dyn FnMut(_)>);
        
        let onerror = Closure::wrap(Box::new(move |_event: web_sys::Event| {
            if let Some(tx) = tx_error.borrow_mut().take() {
                let _ = tx.send(Err(CompressionError::IoError {
                    detail: "IndexedDB request failed".to_string(),
                }));
            }
        }) as Box<dyn FnMut(_)>);
        
        request.set_onsuccess(Some(onsuccess.as_ref().unchecked_ref()));
        request.set_onerror(Some(onerror.as_ref().unchecked_ref()));
        
        onsuccess.forget();
        onerror.forget();
        
        rx.await.map_err(|_| CompressionError::IoError {
            detail: "channel closed".to_string(),
        })?
    }
}
