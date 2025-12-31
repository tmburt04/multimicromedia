use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressionStats {
    pub original_size: u64,
    pub compressed_size: u64,
    pub compression_ratio: f64,
    pub time_ms: u64,
    pub format_in: String,
    pub format_out: String,
}

impl CompressionStats {
    pub fn new(
        original_size: u64,
        compressed_size: u64,
        time_ms: u64,
        format_in: &str,
        format_out: &str,
    ) -> Self {
        let compression_ratio = if original_size > 0 {
            1.0 - (compressed_size as f64 / original_size as f64)
        } else {
            0.0
        };
        Self {
            original_size,
            compressed_size,
            compression_ratio,
            time_ms,
            format_in: format_in.to_string(),
            format_out: format_out.to_string(),
        }
    }
}

#[wasm_bindgen]
pub struct CompressionResult {
    data: Vec<u8>,
    stats: CompressionStats,
}

#[wasm_bindgen]
impl CompressionResult {
    pub(crate) fn new(data: Vec<u8>, stats: CompressionStats) -> Self {
        Self { data, stats }
    }

    #[wasm_bindgen(getter)]
    pub fn data(&self) -> Vec<u8> {
        self.data.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn original_size(&self) -> f64 {
        self.stats.original_size as f64
    }

    #[wasm_bindgen(getter)]
    pub fn compressed_size(&self) -> f64 {
        self.stats.compressed_size as f64
    }

    #[wasm_bindgen(getter)]
    pub fn compression_ratio(&self) -> f64 {
        self.stats.compression_ratio
    }

    #[wasm_bindgen(getter)]
    pub fn time_ms(&self) -> f64 {
        self.stats.time_ms as f64
    }

    #[wasm_bindgen(getter)]
    pub fn format_in(&self) -> String {
        self.stats.format_in.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn format_out(&self) -> String {
        self.stats.format_out.clone()
    }

    #[wasm_bindgen]
    pub fn stats_json(&self) -> String {
        serde_json::to_string(&self.stats).unwrap_or_default()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    pub valid: bool,
    pub errors: Vec<ValidationError>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationError {
    pub field: String,
    pub message: String,
}

impl ValidationResult {
    pub fn ok() -> Self {
        Self {
            valid: true,
            errors: vec![],
            warnings: vec![],
        }
    }

    pub fn with_error(field: &str, message: &str) -> Self {
        Self {
            valid: false,
            errors: vec![ValidationError {
                field: field.to_string(),
                message: message.to_string(),
            }],
            warnings: vec![],
        }
    }

    pub fn add_error(&mut self, field: &str, message: &str) {
        self.valid = false;
        self.errors.push(ValidationError {
            field: field.to_string(),
            message: message.to_string(),
        });
    }

    pub fn add_warning(&mut self, message: &str) {
        self.warnings.push(message.to_string());
    }
}
