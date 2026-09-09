use super::{StorageHandle, StorageType};
use crate::error::{js_error_message, CompressionError, Result};
use futures_channel::oneshot;
use js_sys::{Array, Function, Object, Promise, Uint8Array};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

async fn call(receiver: &JsValue, method: &str, args: &[JsValue]) -> Result<JsValue> {
    let arguments = Array::new();
    for arg in args {
        arguments.push(arg);
    }
    let result = js_sys::Reflect::get(receiver, &method.into())
        .and_then(|value| value.dyn_into::<Function>())
        .and_then(|function| function.apply(receiver, &arguments))
        .map_err(|error| super::storage_error(&format!("OPFS {method}"), error, 0))?;
    JsFuture::from(Promise::resolve(&result))
        .await
        .map_err(|error| super::storage_error(&format!("OPFS {method}"), error, 0))
}

struct PendingFile {
    root: JsValue,
    id: String,
    writable: JsValue,
    committed: bool,
}

impl PendingFile {
    async fn create(root: JsValue, id: String) -> Result<Self> {
        let mut pending = Self {
            root,
            id,
            writable: JsValue::UNDEFINED,
            committed: false,
        };
        let result = async {
            let options = Object::new();
            js_sys::Reflect::set(&options, &"create".into(), &true.into())
                .map_err(|error| super::storage_error("OPFS: create options", error, 0))?;
            let handle = call(
                &pending.root,
                "getFileHandle",
                &[pending.id.clone().into(), options.into()],
            )
            .await?;
            pending.writable = call(&handle, "createWritable", &[]).await?;
            Ok::<(), CompressionError>(())
        }
        .await;
        if let Err(error) = result {
            pending.cleanup().await;
            return Err(error);
        }
        Ok(pending)
    }

    async fn cleanup(&mut self) {
        cleanup_file(&self.root, &self.id, &self.writable).await;
        self.committed = true;
    }
}

async fn cleanup_file(root: &JsValue, id: &str, writable: &JsValue) {
    if !writable.is_undefined() {
        let _ = call(writable, "abort", &[]).await;
    }
    let _ = call(root, "removeEntry", &[id.into()]).await;
}

impl Drop for PendingFile {
    fn drop(&mut self) {
        if self.committed {
            return;
        }
        let root = self.root.clone();
        let id = self.id.clone();
        let writable = self.writable.clone();
        wasm_bindgen_futures::spawn_local(async move {
            cleanup_file(&root, &id, &writable).await;
        });
    }
}

pub struct OpfsStorage;
impl OpfsStorage {
    pub async fn store(data: &[u8]) -> Result<StorageHandle> {
        let id = super::new_storage_id();
        let root = Self::get_root().await?;
        let pending_id = id.clone();
        let (sender, receiver) = oneshot::channel();
        // File/writer creation is not cancelable in the browser. Finish it in a
        // detached task so cancellation can still clean up the resulting handles.
        wasm_bindgen_futures::spawn_local(async move {
            let _ = sender.send(PendingFile::create(root, pending_id).await);
        });
        let mut pending = receiver
            .await
            .map_err(|_| CompressionError::IoError {
                detail: "OPFS: file creation canceled".into(),
            })?
            .map_err(|error| match error {
                CompressionError::StorageFull { .. } => CompressionError::StorageFull {
                    required_bytes: data.len() as u64,
                },
                error => error,
            })?;
        let result = async {
            // Only one bounded JS copy is live at a time, even for large files.
            for chunk in data.chunks(super::IO_CHUNK_SIZE) {
                call(
                    &pending.writable,
                    "write",
                    &[Uint8Array::from(chunk).into()],
                )
                .await?;
            }
            call(&pending.writable, "close", &[]).await?;
            Ok::<(), CompressionError>(())
        }
        .await;
        if let Err(error) = result {
            pending.cleanup().await;
            return Err(match error {
                CompressionError::StorageFull { .. } => CompressionError::StorageFull {
                    required_bytes: data.len() as u64,
                },
                error => error,
            });
        }
        pending.committed = true;
        Ok(StorageHandle {
            storage_type: StorageType::Opfs,
            data: Vec::new(),
            size: data.len(),
            id,
        })
    }

    pub async fn retrieve(id: &str) -> Result<Vec<u8>> {
        let root = Self::get_root().await?;
        let handle = call(&root, "getFileHandle", &[id.into()]).await?;
        let file = call(&handle, "getFile", &[]).await?;
        let size = js_sys::Reflect::get(&file, &"size".into())
            .ok()
            .and_then(|value| value.as_f64())
            .filter(|size| size.is_finite() && *size >= 0.0 && size.fract() == 0.0)
            .ok_or_else(|| CompressionError::IoError {
                detail: "OPFS: invalid file size".into(),
            })?;
        if size >= isize::MAX as f64 {
            return Err(CompressionError::MemoryLimitExceeded);
        }
        let size = size as usize;
        let mut data = Vec::new();
        data.try_reserve_exact(size)
            .map_err(|_| CompressionError::MemoryLimitExceeded)?;
        for start in (0..size).step_by(super::IO_CHUNK_SIZE) {
            let end = start.saturating_add(super::IO_CHUNK_SIZE).min(size);
            let slice = call(
                &file,
                "slice",
                &[(start as f64).into(), (end as f64).into()],
            )
            .await?;
            let buffer = call(&slice, "arrayBuffer", &[]).await?;
            let bytes = Uint8Array::new(&buffer);
            if bytes.length() as usize != end - start {
                return Err(CompressionError::IoError {
                    detail: "OPFS: incomplete file read".into(),
                });
            }
            data.resize(end, 0);
            bytes.copy_to(&mut data[start..end]);
        }
        Ok(data)
    }

    pub async fn delete(id: &str) -> Result<()> {
        let root = Self::get_root().await?;
        call(&root, "removeEntry", &[id.into()]).await?;
        Ok(())
    }

    async fn get_root() -> Result<JsValue> {
        let navigator =
            js_sys::Reflect::get(&js_sys::global(), &"navigator".into()).map_err(|error| {
                CompressionError::StorageUnavailable {
                    reason: js_error_message(&error),
                }
            })?;
        let storage = js_sys::Reflect::get(&navigator, &"storage".into()).map_err(|error| {
            CompressionError::StorageUnavailable {
                reason: js_error_message(&error),
            }
        })?;
        call(&storage, "getDirectory", &[]).await.map_err(|error| {
            CompressionError::StorageUnavailable {
                reason: error.to_string(),
            }
        })
    }
}
