// OCR module for Chonker 7.58
pub mod engine;
pub mod ui;
pub mod models;
pub mod pdf_layer;

pub use engine::{OcrLayer, OcrMode, OcrNeed, OcrResult, TextBlock};
pub use ui::{OcrMenu, OcrStatus, OcrStats};
pub use models::download_models;
pub use pdf_layer::PdfOcrOps;