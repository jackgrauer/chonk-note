// Machine Learning module for intelligent document understanding
// Uses Candle + LayoutLMv3 with CoreML/ANE acceleration on Apple Silicon

pub mod layoutlm;
pub mod coreml_bridge;
pub mod tensor_utils;
pub mod document_understanding;
pub mod model_loader;
pub mod inference;

pub use layoutlm::LayoutLMv3Native;
pub use document_understanding::{DocumentUnderstanding, EntityType, TableCell};

#[cfg(all(target_os = "macos", feature = "coreml"))]
pub use layoutlm::LayoutLMv3CoreML;

use anyhow::Result;

/// Check if CoreML acceleration is available
pub fn is_accelerated() -> bool {
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    {
        // Check for M1/M2/M3 silicon
        std::path::Path::new("/System/Library/Frameworks/CoreML.framework").exists()
    }
    #[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
    {
        false
    }
}

/// Initialize ML models with appropriate backend
pub async fn initialize_models() -> Result<Box<dyn DocumentProcessor>> {
    #[cfg(all(target_os = "macos", feature = "coreml"))]
    {
        if is_accelerated() {
            eprintln!("🚀 Using Apple Neural Engine acceleration");
            return Ok(Box::new(LayoutLMv3CoreML::load_default().await?));
        }
    }
    
    eprintln!("🐢 Using CPU inference (consider using M1/M2/M3 Mac for 10x speedup)");
    Ok(Box::new(LayoutLMv3Native::load_default().await?))
}

/// Trait for document processing backends
pub trait DocumentProcessor: Send + Sync {
    /// Process a page of PDF data
    fn process_page(&self, page_data: &crate::content_extractor::PageData) -> Result<DocumentUnderstanding>;
    
    /// Get backend name for debugging
    fn backend_name(&self) -> &str;
}