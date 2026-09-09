mod indexeddb;
mod opfs;

pub use indexeddb::*;
pub use opfs::*;

use crate::error::{js_error_message, CompressionError, Result};
use wasm_bindgen::JsValue;

const IO_CHUNK_SIZE: usize = 4 * 1024 * 1024;

fn storage_error(operation: &str, error: JsValue, required_bytes: u64) -> CompressionError {
    let name = js_sys::Reflect::get(&error, &"name".into())
        .ok()
        .and_then(|value| value.as_string());
    if name.as_deref() == Some("QuotaExceededError") {
        CompressionError::StorageFull { required_bytes }
    } else {
        CompressionError::IoError {
            detail: format!("{operation}: {}", js_error_message(&error)),
        }
    }
}

fn copy_array(data: &js_sys::Uint8Array) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    let length = data.length() as usize;
    bytes
        .try_reserve_exact(length)
        .map_err(|_| CompressionError::MemoryLimitExceeded)?;
    bytes.resize(length, 0);
    data.copy_to(&mut bytes);
    Ok(bytes)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageType {
    Opfs,
    IndexedDb,
    Memory,
}

pub struct StorageHandle {
    storage_type: StorageType,
    data: Vec<u8>,
    id: String,
    size: usize,
}

impl StorageHandle {
    pub fn memory(data: Vec<u8>) -> Self {
        Self {
            storage_type: StorageType::Memory,
            size: data.len(),
            data,
            id: new_storage_id(),
        }
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn storage_type(&self) -> StorageType {
        self.storage_type
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }

    pub fn into_data(self) -> Vec<u8> {
        self.data
    }

    pub fn size(&self) -> usize {
        self.size
    }
}

pub struct StorageManager {
    preferred: StorageType,
    max_memory_size: usize,
}

impl Default for StorageManager {
    fn default() -> Self {
        Self::new()
    }
}

impl StorageManager {
    pub fn new() -> Self {
        Self {
            preferred: detect_best_storage(),
            max_memory_size: 64 * 1024 * 1024, // 64MB default
        }
    }

    pub fn with_max_memory(mut self, size: usize) -> Self {
        self.max_memory_size = size;
        self
    }

    pub async fn store(&self, data: Vec<u8>) -> Result<StorageHandle> {
        let size = data.len();

        // For small data, use memory
        if size <= self.max_memory_size {
            return Ok(StorageHandle::memory(data));
        }

        let mut storage_error = CompressionError::StorageUnavailable {
            reason: "No persistent storage backend is available".into(),
        };
        // Try OPFS first.
        if self.preferred == StorageType::Opfs {
            match OpfsStorage::store(&data).await {
                Ok(handle) => return Ok(handle),
                Err(error) => storage_error = error,
            }
        }

        if is_indexeddb_available() {
            match IndexedDbStorage::store(&data).await {
                Ok(handle) => return Ok(handle),
                Err(error) => storage_error = error,
            }
        }

        // The memory budget is a limit, including when persistent storage fails.
        Err(storage_error)
    }

    pub async fn retrieve(&self, handle: &StorageHandle) -> Result<Vec<u8>> {
        let data = match handle.storage_type {
            StorageType::Memory => {
                let mut data = Vec::new();
                data.try_reserve_exact(handle.data.len())
                    .map_err(|_| CompressionError::MemoryLimitExceeded)?;
                data.extend_from_slice(&handle.data);
                Ok(data)
            }
            StorageType::Opfs => OpfsStorage::retrieve(&handle.id).await,
            StorageType::IndexedDb => IndexedDbStorage::retrieve(&handle.id).await,
        }?;
        if data.len() != handle.size {
            return Err(CompressionError::IoError {
                detail: "Stored file size changed".into(),
            });
        }
        Ok(data)
    }

    pub async fn delete(&self, handle: StorageHandle) -> Result<()> {
        match handle.storage_type {
            StorageType::Memory => Ok(()), // Nothing to clean up
            StorageType::Opfs => OpfsStorage::delete(&handle.id).await,
            StorageType::IndexedDb => IndexedDbStorage::delete(&handle.id).await,
        }
    }
}

fn detect_best_storage() -> StorageType {
    if is_opfs_available() {
        return StorageType::Opfs;
    }

    if is_indexeddb_available() {
        return StorageType::IndexedDb;
    }

    StorageType::Memory
}

fn is_opfs_available() -> bool {
    let global = js_sys::global();

    if let Ok(navigator) = js_sys::Reflect::get(&global, &"navigator".into()) {
        if let Ok(storage) = js_sys::Reflect::get(&navigator, &"storage".into()) {
            if let Ok(get_dir) = js_sys::Reflect::get(&storage, &"getDirectory".into()) {
                return get_dir.is_function();
            }
        }
    }

    false
}

fn is_indexeddb_available() -> bool {
    let global = js_sys::global();

    if let Ok(indexed_db) = js_sys::Reflect::get(&global, &"indexedDB".into()) {
        return !indexed_db.is_undefined() && !indexed_db.is_null();
    }

    false
}

fn new_storage_id() -> String {
    format!(
        "compress_{}_{}",
        js_sys::Date::now() as u64,
        js_sys::Math::random()
    )
}
