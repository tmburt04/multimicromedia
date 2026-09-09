use super::{StorageHandle, StorageType};
use crate::error::{CompressionError, Result};
use futures_channel::oneshot;
use js_sys::{Object, Uint8Array};
use std::{cell::RefCell, rc::Rc};
use wasm_bindgen::prelude::*;
use web_sys::{IdbDatabase, IdbOpenDbRequest, IdbRequest, IdbTransaction, IdbTransactionMode};

const DB_NAME: &str = "compression_wasm_storage";
const STORE_NAME: &str = "files";
const DB_VERSION: u32 = 1;

fn io_error(detail: &str) -> CompressionError {
    CompressionError::IoError {
        detail: detail.into(),
    }
}

// Handlers are detached before their Rust closures are dropped, even on cancellation.
struct RequestHandlers {
    request: IdbOpenDbRequest,
    _callbacks: Vec<Closure<dyn FnMut(web_sys::Event)>>,
}
impl Drop for RequestHandlers {
    fn drop(&mut self) {
        self.request.set_onsuccess(None);
        self.request.set_onerror(None);
        self.request.set_onblocked(None);
        self.request.set_onupgradeneeded(None);
    }
}
struct TransactionHandlers {
    transaction: IdbTransaction,
    _callbacks: Vec<Closure<dyn FnMut(web_sys::Event)>>,
}
impl Drop for TransactionHandlers {
    fn drop(&mut self) {
        // A canceled write must not commit data for which no handle was returned.
        // abort() is harmless (and fails) once the transaction has completed.
        let _ = self.transaction.abort();
        self.transaction.set_oncomplete(None);
        self.transaction.set_onabort(None);
        self.transaction.set_onerror(None);
    }
}
struct Database(IdbDatabase);
impl Drop for Database {
    fn drop(&mut self) {
        self.0.close();
    }
}

struct PendingRecord {
    id: String,
    request: IdbRequest,
    delivered: bool,
}

impl Drop for PendingRecord {
    fn drop(&mut self) {
        // Cancellation can arrive after commit but before the caller receives
        // the handle. At that point aborting the transaction is too late.
        // A successful add() proves this request owns the record. A failed
        // duplicate-key add must never delete the pre-existing record.
        if !self.delivered
            && self
                .request
                .result()
                .ok()
                .and_then(|result| result.as_string())
                .as_deref()
                == Some(self.id.as_str())
        {
            let id = self.id.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let _ = IndexedDbStorage::delete(&id).await;
            });
        }
    }
}

pub struct IndexedDbStorage;
impl IndexedDbStorage {
    pub async fn store(data: &[u8]) -> Result<StorageHandle> {
        let id = super::new_storage_id();
        let db = Self::open_db().await?;
        let transaction =
            db.0.transaction_with_str_and_mode(STORE_NAME, IdbTransactionMode::Readwrite)
                .map_err(|_| io_error("IndexedDB: cannot start write transaction"))?;
        let store = transaction
            .object_store(STORE_NAME)
            .map_err(|_| io_error("IndexedDB: missing files store"))?;
        let record = Object::new();
        js_sys::Reflect::set(&record, &"id".into(), &id.clone().into())
            .map_err(|_| io_error("IndexedDB: cannot set ID"))?;
        js_sys::Reflect::set(&record, &"data".into(), &Uint8Array::from(data))
            .map_err(|_| io_error("IndexedDB: cannot set data"))?;
        let request = store.add(&record).map_err(|error| {
            super::storage_error("IndexedDB: cannot enqueue write", error, data.len() as u64)
        })?;
        let mut pending = PendingRecord {
            id: id.clone(),
            request: request.clone(),
            delivered: false,
        };
        Self::await_transaction(transaction, request, data.len() as u64).await?;
        pending.delivered = true;
        Ok(StorageHandle {
            storage_type: StorageType::IndexedDb,
            data: Vec::new(),
            size: data.len(),
            id,
        })
    }

    pub async fn retrieve(id: &str) -> Result<Vec<u8>> {
        let db = Self::open_db().await?;
        let transaction =
            db.0.transaction_with_str(STORE_NAME)
                .map_err(|_| io_error("IndexedDB: cannot start read transaction"))?;
        let store = transaction
            .object_store(STORE_NAME)
            .map_err(|_| io_error("IndexedDB: missing files store"))?;
        let request = store
            .get(&id.into())
            .map_err(|_| io_error("IndexedDB: cannot enqueue read"))?;
        let record = Self::await_transaction(transaction, request, 0).await?;
        if record.is_undefined() {
            return Err(io_error("IndexedDB: file not found"));
        }
        let data = js_sys::Reflect::get(&record, &"data".into())
            .map_err(|_| io_error("IndexedDB: missing data field"))?;
        data.dyn_ref::<Uint8Array>()
            .ok_or_else(|| io_error("IndexedDB: invalid file data"))
            .and_then(super::copy_array)
    }

    pub async fn delete(id: &str) -> Result<()> {
        let db = Self::open_db().await?;
        let transaction =
            db.0.transaction_with_str_and_mode(STORE_NAME, IdbTransactionMode::Readwrite)
                .map_err(|_| io_error("IndexedDB: cannot start delete transaction"))?;
        let store = transaction
            .object_store(STORE_NAME)
            .map_err(|_| io_error("IndexedDB: missing files store"))?;
        let request = store
            .delete(&id.into())
            .map_err(|_| io_error("IndexedDB: cannot enqueue delete"))?;
        Self::await_transaction(transaction, request, 0).await?;
        Ok(())
    }

    async fn open_db() -> Result<Database> {
        let factory = js_sys::Reflect::get(&js_sys::global(), &"indexedDB".into())
            .ok()
            .and_then(|value| value.dyn_into::<web_sys::IdbFactory>().ok())
            .ok_or_else(|| CompressionError::StorageUnavailable {
                reason: "IndexedDB unavailable".into(),
            })?;
        let request = factory
            .open_with_u32(DB_NAME, DB_VERSION)
            .map_err(|_| io_error("IndexedDB: open failed"))?;
        let (sender, receiver) = oneshot::channel();
        let sender = Rc::new(RefCell::new(Some(sender)));
        let (finished_sender, finished_receiver) = oneshot::channel();
        let finished_sender = Rc::new(RefCell::new(Some(finished_sender)));
        let success_finished = finished_sender.clone();
        let success_sender = sender.clone();
        let success_request = request.clone();
        let success = Closure::wrap(Box::new(move |_: web_sys::Event| {
            let result = success_request
                .result()
                .map_err(|_| io_error("IndexedDB: missing database"))
                .and_then(|value| {
                    value
                        .dyn_into::<IdbDatabase>()
                        .map(Database)
                        .map_err(|_| io_error("IndexedDB: invalid database"))
                });
            if let Some(sender) = success_sender.borrow_mut().take() {
                let _ = sender.send(result);
            }
            if let Some(sender) = success_finished.borrow_mut().take() {
                let _ = sender.send(());
            }
        }) as Box<dyn FnMut(_)>);
        let error_sender = sender.clone();
        let error_request = request.clone();
        let error = Closure::wrap(Box::new(move |_: web_sys::Event| {
            if let Some(sender) = error_sender.borrow_mut().take() {
                let error = js_sys::Reflect::get(error_request.as_ref(), &"error".into())
                    .unwrap_or(JsValue::UNDEFINED);
                let _ = sender.send(Err(super::storage_error(
                    "IndexedDB: open failed",
                    error,
                    0,
                )));
            }
            if let Some(sender) = finished_sender.borrow_mut().take() {
                let _ = sender.send(());
            }
        }) as Box<dyn FnMut(_)>);
        let blocked_sender = sender.clone();
        let blocked = Closure::wrap(Box::new(move |_: web_sys::Event| {
            if let Some(sender) = blocked_sender.borrow_mut().take() {
                let _ = sender.send(Err(io_error(
                    "IndexedDB: upgrade blocked; close other tabs",
                )));
            }
        }) as Box<dyn FnMut(_)>);
        let upgrade_request = request.clone();
        let upgrade = Closure::wrap(Box::new(move |_: web_sys::Event| {
            if sender
                .borrow()
                .as_ref()
                .is_none_or(oneshot::Sender::is_canceled)
            {
                if let Some(transaction) = upgrade_request.transaction() {
                    let _ = transaction.abort();
                }
                return;
            }
            let created = upgrade_request
                .result()
                .ok()
                .and_then(|value| value.dyn_into::<IdbDatabase>().ok())
                .is_some_and(|db| {
                    if db.object_store_names().contains(STORE_NAME) {
                        return true;
                    }
                    let params = web_sys::IdbObjectStoreParameters::new();
                    params.set_key_path(&"id".into());
                    db.create_object_store_with_optional_parameters(STORE_NAME, &params)
                        .is_ok()
                });
            if !created {
                if let Some(transaction) = upgrade_request.transaction() {
                    let _ = transaction.abort();
                }
            }
        }) as Box<dyn FnMut(_)>);
        request.set_onsuccess(Some(success.as_ref().unchecked_ref()));
        request.set_onerror(Some(error.as_ref().unchecked_ref()));
        request.set_onblocked(Some(blocked.as_ref().unchecked_ref()));
        request.set_onupgradeneeded(Some(upgrade.as_ref().unchecked_ref()));
        let handlers = RequestHandlers {
            request,
            _callbacks: vec![success, error, blocked, upgrade],
        };
        // IndexedDB opens cannot be canceled. Retain the handlers until a terminal
        // event so a database opened after cancellation/blocked is still closed.
        wasm_bindgen_futures::spawn_local(async move {
            let _handlers = handlers;
            let _ = finished_receiver.await;
        });
        receiver
            .await
            .map_err(|_| io_error("IndexedDB: open cancelled"))?
    }

    async fn await_transaction(
        transaction: IdbTransaction,
        request: IdbRequest,
        required_bytes: u64,
    ) -> Result<JsValue> {
        let (sender, receiver) = oneshot::channel();
        let sender = Rc::new(RefCell::new(Some(sender)));
        let complete_sender = sender.clone();
        let failed_request = request.clone();
        let complete = Closure::wrap(Box::new(move |_: web_sys::Event| {
            if let Some(sender) = complete_sender.borrow_mut().take() {
                let _ = sender.send(
                    request
                        .result()
                        .map_err(|_| io_error("IndexedDB: missing request result")),
                );
            }
        }) as Box<dyn FnMut(_)>);
        let failed_transaction = transaction.clone();
        let failed = Closure::wrap(Box::new(move |_: web_sys::Event| {
            if let Some(sender) = sender.borrow_mut().take() {
                let error = js_sys::Reflect::get(failed_transaction.as_ref(), &"error".into())
                    .ok()
                    .filter(|error| !error.is_null() && !error.is_undefined())
                    .or_else(|| js_sys::Reflect::get(failed_request.as_ref(), &"error".into()).ok())
                    .unwrap_or(JsValue::UNDEFINED);
                let _ = sender.send(Err(super::storage_error(
                    "IndexedDB: transaction failed or aborted",
                    error,
                    required_bytes,
                )));
            }
        }) as Box<dyn FnMut(_)>);
        transaction.set_oncomplete(Some(complete.as_ref().unchecked_ref()));
        transaction.set_onabort(Some(failed.as_ref().unchecked_ref()));
        transaction.set_onerror(Some(failed.as_ref().unchecked_ref()));
        let _handlers = TransactionHandlers {
            transaction,
            _callbacks: vec![complete, failed],
        };
        // Request success is not durable: wait for the transaction to commit.
        receiver
            .await
            .map_err(|_| io_error("IndexedDB: transaction cancelled"))?
    }
}
