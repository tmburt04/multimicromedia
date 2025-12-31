mod indexeddb;
mod opfs;

pub use indexeddb::*;
pub use opfs::*;

use crate::error::{CompressionError, Result};

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
}

impl StorageHandle {
    pub fn memory(data: Vec<u8>) -> Self {
        Self {
            storage_type: StorageType::Memory,
            data,
            id: format!("mem_{}", js_sys::Math::random()),
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
        self.data.len()
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

        // Try OPFS first
        if self.preferred == StorageType::Opfs {
            match store_to_opfs(&data).await {
                Ok(handle) => return Ok(handle),
                Err(_) => {
                    // Fall through to IndexedDB
                }
            }
        }

        // Try IndexedDB
        match store_to_indexeddb(&data).await {
            Ok(handle) => return Ok(handle),
            Err(_) => {
                // Last resort: memory (may fail for very large files)
            }
        }

        // Fallback to memory
        if size > 256 * 1024 * 1024 {
            return Err(CompressionError::MemoryLimitExceeded);
        }

        Ok(StorageHandle::memory(data))
    }

    pub async fn retrieve(&self, handle: &StorageHandle) -> Result<Vec<u8>> {
        match handle.storage_type {
            StorageType::Memory => Ok(handle.data.clone()),
            StorageType::Opfs => retrieve_from_opfs(&handle.id).await,
            StorageType::IndexedDb => retrieve_from_indexeddb(&handle.id).await,
        }
    }

    pub async fn delete(&self, handle: StorageHandle) -> Result<()> {
        match handle.storage_type {
            StorageType::Memory => Ok(()), // Nothing to clean up
            StorageType::Opfs => delete_from_opfs(&handle.id).await,
            StorageType::IndexedDb => delete_from_indexeddb(&handle.id).await,
        }
    }
}

fn detect_best_storage() -> StorageType {
    // Check for OPFS support
    if is_opfs_available() {
        return StorageType::Opfs;
    }

    // Check for IndexedDB support
    if is_indexeddb_available() {
        return StorageType::IndexedDb;
    }

    StorageType::Memory
}

fn is_opfs_available() -> bool {
    // Check if navigator.storage.getDirectory is available
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

async fn store_to_opfs(data: &[u8]) -> Result<StorageHandle> {
    // OPFS implementation
    OpfsStorage::store(data).await
}

async fn retrieve_from_opfs(id: &str) -> Result<Vec<u8>> {
    OpfsStorage::retrieve(id).await
}

async fn delete_from_opfs(id: &str) -> Result<()> {
    OpfsStorage::delete(id).await
}

async fn store_to_indexeddb(data: &[u8]) -> Result<StorageHandle> {
    IndexedDbStorage::store(data).await
}

async fn retrieve_from_indexeddb(id: &str) -> Result<Vec<u8>> {
    IndexedDbStorage::retrieve(id).await
}

async fn delete_from_indexeddb(id: &str) -> Result<()> {
    IndexedDbStorage::delete(id).await
}
