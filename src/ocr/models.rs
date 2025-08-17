// OCR Model management for Chonker 7.58
use anyhow::{Result, Context};
use std::path::PathBuf;

#[cfg(feature = "ocr")]
use rten::Model;

const DETECTION_MODEL_URL: &str = 
    "https://huggingface.co/robertknight/ocrs/resolve/main/text-detection.rten";
const RECOGNITION_MODEL_URL: &str = 
    "https://huggingface.co/robertknight/ocrs/resolve/main/text-recognition.rten";

#[cfg(feature = "ocr")]
pub async fn download_models() -> Result<()> {
    let cache_dir = dirs::cache_dir()
        .ok_or_else(|| anyhow::anyhow!("No cache directory"))?
        .join("chonker7")
        .join("ocr");
    
    std::fs::create_dir_all(&cache_dir)?;
    
    let detection_path = cache_dir.join("text-detection.rten");
    let recognition_path = cache_dir.join("text-recognition.rten");
    
    // Download if missing
    if !detection_path.exists() {
        eprintln!("📥 Downloading OCR detection model (6MB)...");
        download_file(DETECTION_MODEL_URL, &detection_path).await
            .context("Failed to download detection model")?;
    }
    
    if !recognition_path.exists() {
        eprintln!("📥 Downloading OCR recognition model (6MB)...");
        download_file(RECOGNITION_MODEL_URL, &recognition_path).await
            .context("Failed to download recognition model")?;
    }
    
    Ok(())
}

#[cfg(not(feature = "ocr"))]
pub async fn download_models() -> Result<()> {
    eprintln!("⚠️ OCR not available - compile with --features ocr");
    Ok(())
}

#[cfg(feature = "ocr")]
async fn download_file(url: &str, path: &PathBuf) -> Result<()> {
    let response = reqwest::get(url).await
        .context("Failed to fetch model")?;
    
    let bytes = response.bytes().await
        .context("Failed to read response bytes")?;
    
    std::fs::write(path, bytes)
        .context("Failed to write model file")?;
    
    eprintln!("✅ Downloaded to {}", path.display());
    Ok(())
}