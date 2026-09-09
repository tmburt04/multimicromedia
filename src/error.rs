use thiserror::Error;
use wasm_bindgen::prelude::*;

#[derive(Error, Debug, Clone)]
pub enum CompressionError {
    #[error("Unsupported format '{detected}'{}", .fallback.as_ref().map(|f| format!(", try {}", f)).unwrap_or_default())]
    UnsupportedFormat {
        detected: String,
        fallback: Option<String>,
    },

    #[error("Chunk {chunk_id} corrupted{}", if *.can_retry { ", retry available" } else { "" })]
    ChunkCorrupted { chunk_id: u64, can_retry: bool },

    #[error("Storage full: need {required_bytes} bytes")]
    StorageFull { required_bytes: u64 },

    #[error("Storage unavailable: {reason}")]
    StorageUnavailable { reason: String },

    #[error("Invalid config for '{field}': {reason}")]
    InvalidConfig { field: String, reason: String },

    #[error("Failed to decode {format}: {detail}")]
    DecodeFailed { format: String, detail: String },

    #[error("Failed to encode {format}: {detail}")]
    EncodeFailed { format: String, detail: String },

    #[error("FFmpeg unavailable or failed to load")]
    FfmpegUnavailable,

    #[error("FFmpeg execution failed: {detail}")]
    FfmpegFailed { detail: String },

    #[error("I/O error: {detail}")]
    IoError { detail: String },

    #[error("Memory limit exceeded: file too large for in-memory processing")]
    MemoryLimitExceeded,

    #[error("Invalid input: {reason}")]
    InvalidInput { reason: String },

    #[error("Partial result: {completed_chunks}/{total_chunks} chunks processed")]
    PartialResult {
        completed_chunks: u64,
        total_chunks: u64,
        output: Option<String>,
    },
}

impl CompressionError {
    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            CompressionError::ChunkCorrupted {
                can_retry: true,
                ..
            } | CompressionError::StorageFull { .. }
                | CompressionError::PartialResult { .. }
        )
    }

    pub fn error_code(&self) -> &'static str {
        match self {
            CompressionError::UnsupportedFormat { .. } => "E_UNSUPPORTED_FORMAT",
            CompressionError::ChunkCorrupted { .. } => "E_CHUNK_CORRUPTED",
            CompressionError::StorageFull { .. } => "E_STORAGE_FULL",
            CompressionError::StorageUnavailable { .. } => "E_STORAGE_UNAVAILABLE",
            CompressionError::InvalidConfig { .. } => "E_INVALID_CONFIG",
            CompressionError::DecodeFailed { .. } => "E_DECODE_FAILED",
            CompressionError::EncodeFailed { .. } => "E_ENCODE_FAILED",
            CompressionError::FfmpegUnavailable => "E_FFMPEG_UNAVAILABLE",
            CompressionError::FfmpegFailed { .. } => "E_FFMPEG_FAILED",
            CompressionError::IoError { .. } => "E_IO_ERROR",
            CompressionError::MemoryLimitExceeded => "E_MEMORY_LIMIT",
            CompressionError::InvalidInput { .. } => "E_INVALID_INPUT",
            CompressionError::PartialResult { .. } => "W_PARTIAL_RESULT",
        }
    }
}

impl From<CompressionError> for JsValue {
    fn from(err: CompressionError) -> Self {
        let obj = js_sys::Object::new();
        let _ = js_sys::Reflect::set(&obj, &"code".into(), &err.error_code().into());
        let _ = js_sys::Reflect::set(&obj, &"message".into(), &err.to_string().into());
        let _ = js_sys::Reflect::set(&obj, &"recoverable".into(), &err.is_recoverable().into());
        if let CompressionError::InvalidConfig { field, .. } = &err {
            let _ = js_sys::Reflect::set(&obj, &"field".into(), &field.as_str().into());
        }
        obj.into()
    }
}

pub type Result<T> = std::result::Result<T, CompressionError>;

/// Extract useful JS Error/DOMException messages without dumping opaque handles.
pub(crate) fn js_error_message(error: &JsValue) -> String {
    error
        .as_string()
        .or_else(|| {
            js_sys::Reflect::get(error, &"message".into())
                .ok()
                .and_then(|value| value.as_string())
        })
        .unwrap_or_else(|| "JavaScript operation failed".into())
}
