// OCR Engine implementation for Chonker 7.58
use anyhow::Result;
use std::path::Path;
use image::DynamicImage;

/// Convert superscript and subscript Unicode characters to ASCII notation
fn convert_super_subscript_to_ascii(ch: char) -> char {
    match ch {
        // Superscript digits
        '⁰' => '0', '¹' => '1', '²' => '2', '³' => '3', '⁴' => '4',
        '⁵' => '5', '⁶' => '6', '⁷' => '7', '⁸' => '8', '⁹' => '9',
        
        // Subscript digits  
        '₀' => '0', '₁' => '1', '₂' => '2', '₃' => '3', '₄' => '4',
        '₅' => '5', '₆' => '6', '₇' => '7', '₈' => '8', '₉' => '9',
        
        // Common superscript letters
        'ᵃ' => 'a', 'ᵇ' => 'b', 'ᶜ' => 'c', 'ᵈ' => 'd', 'ᵉ' => 'e',
        'ᶠ' => 'f', 'ᵍ' => 'g', 'ʰ' => 'h', 'ⁱ' => 'i', 'ʲ' => 'j',
        'ᵏ' => 'k', 'ˡ' => 'l', 'ᵐ' => 'm', 'ⁿ' => 'n', 'ᵒ' => 'o',
        'ᵖ' => 'p', 'ʳ' => 'r', 'ˢ' => 's', 'ᵗ' => 't', 'ᵘ' => 'u',
        'ᵛ' => 'v', 'ʷ' => 'w', 'ˣ' => 'x', 'ʸ' => 'y', 'ᶻ' => 'z',
        
        // Common subscript letters
        'ₐ' => 'a', 'ₑ' => 'e', 'ₕ' => 'h', 'ᵢ' => 'i', 'ⱼ' => 'j',
        'ₖ' => 'k', 'ₗ' => 'l', 'ₘ' => 'm', 'ₙ' => 'n', 'ₒ' => 'o',
        'ₚ' => 'p', 'ᵣ' => 'r', 'ₛ' => 's', 'ₜ' => 't', 'ᵤ' => 'u',
        'ᵥ' => 'v', 'ₓ' => 'x',
        
        // Mathematical superscripts
        '⁺' => '+', '⁻' => '-', '⁼' => '=', '⁽' => '(', '⁾' => ')',
        
        // Mathematical subscripts
        '₊' => '+', '₋' => '-', '₌' => '=', '₍' => '(', '₎' => ')',
        
        // No conversion needed
        _ => ch
    }
}

#[cfg(feature = "ocr")]
use ocrs::{OcrEngine, OcrEngineParams, ImageSource, DimOrder};
#[cfg(feature = "ocr")]
use rten::Model;
#[cfg(feature = "ocr")]
use rten_tensor::{NdTensor, AsView};

pub struct OcrLayer {
    #[cfg(feature = "ocr")]
    engine: Option<OcrEngine>,
    initialized: bool,
}

#[derive(Debug, Clone)]
pub enum OcrMode {
    Detect,    // Check if OCR is needed
    Overlay,   // Add text layer to existing
    Replace,   // Strip and rebuild text layer
    Force,     // Force OCR even if text exists
}

#[derive(Debug, Clone, PartialEq)]
pub enum OcrNeed {
    HasText,      // Good text layer exists
    NeedsOcr,     // No text layer found
    BadOcr,       // Corrupted/garbage text
    MixedContent, // Some text, some images
}

#[derive(Debug, Clone)]
pub struct OcrResult {
    pub blocks: Vec<TextBlock>,
    pub confidence: f32,
    pub was_needed: bool,
    pub duration_ms: u64,
}

#[derive(Debug, Clone)]
pub struct TextBlock {
    pub text: String,
    pub bbox: BoundingBox,
    pub confidence: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct BoundingBox {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl OcrLayer {
    pub fn new() -> Self {
        Self {
            #[cfg(feature = "ocr")]
            engine: None,
            initialized: false,
        }
    }
    
    #[cfg(feature = "ocr")]
    pub async fn lazy_init(&mut self) -> Result<()> {
        if self.initialized {
            return Ok(());
        }
        
        eprintln!("🔍 Initializing OCR engine...");
        
        // Load model data from files
        let cache_dir = dirs::cache_dir()
            .ok_or_else(|| anyhow::anyhow!("No cache directory"))?
            .join("chonker7")
            .join("ocr");
        
        let detection_path = cache_dir.join("text-detection.rten");
        let recognition_path = cache_dir.join("text-recognition.rten");
        
        // Ensure models are downloaded
        if !detection_path.exists() || !recognition_path.exists() {
            eprintln!("📥 Downloading OCR models...");
            super::models::download_models().await?;
        }
        
        // Load models from bytes
        eprintln!("📚 Loading OCR models from disk...");
        let detection_data = std::fs::read(&detection_path)?;
        let recognition_data = std::fs::read(&recognition_path)?;
        
        let detection_model = Model::load(detection_data)?;
        let recognition_model = Model::load(recognition_data)?;
        
        // Create OCR engine with params
        let engine = OcrEngine::new(OcrEngineParams {
            detection_model: Some(detection_model),
            recognition_model: Some(recognition_model),
            alphabet: None,  // Use default alphabet
            decode_method: ocrs::DecodeMethod::Greedy,
            debug: false,
        })?;
        
        self.engine = Some(engine);
        self.initialized = true;
        eprintln!("✅ OCR engine ready");
        
        Ok(())
    }
    
    #[cfg(not(feature = "ocr"))]
    pub async fn lazy_init(&mut self) -> Result<()> {
        eprintln!("⚠️ OCR not available - compile with --features ocr");
        Ok(())
    }
    
    pub fn analyze_page_text(&self, text: &str, has_images: bool) -> OcrNeed {
        let text_len = text.len();
        let has_garbage = text.chars().any(|c| 
            c == '�' || c == '□' || c == '�' || c == '\u{fffd}'
        );
        
        match (text_len, has_images, has_garbage) {
            (0, true, _) => OcrNeed::NeedsOcr,
            (len, _, true) if len < 500 => OcrNeed::BadOcr,
            (len, true, false) if len < 300 => OcrNeed::MixedContent,
            _ => OcrNeed::HasText,
        }
    }
    
    #[cfg(feature = "ocr")]
    pub async fn process(&mut self, 
                        image: &DynamicImage, 
                        mode: OcrMode) -> Result<OcrResult> {
        let start = std::time::Instant::now();
        
        // Ensure initialized
        self.lazy_init().await?;
        
        // Preprocess image
        let processed = preprocess_image(image)?;
        
        // Run OCR
        let engine = self.engine.as_ref().unwrap();
        
        // Convert to grayscale image for ocrs
        let gray_img = processed.to_luma8();
        
        // Convert grayscale ImageBuffer into a tensor for ocrs
        let (width, height) = gray_img.dimensions();
        
        // Convert pixel values to f32 in [0.0, 1.0] range
        let pixels: Vec<f32> = gray_img
            .pixels()
            .flat_map(|p| p.0.iter())
            .map(|&v| v as f32 / 255.0)
            .collect();
        
        // Create tensor with shape [1, height, width] (CHW format)
        // Note: grayscale has 1 channel
        let tensor = NdTensor::from_data(
            [1, height as usize, width as usize],
            pixels
        );
        
        // Create ImageSource from the tensor view
        let img_src = ImageSource::from_tensor(tensor.view(), DimOrder::Chw)?;
        
        // Prepare input for OCR engine
        let ocr_input = engine.prepare_input(img_src)?;
        
        // Detect and recognize text
        let ocr_text = engine.get_text(&ocr_input)?;
        
        // Build text blocks from results  
        let blocks = build_text_blocks(ocr_text)?;
        
        let confidence = calculate_confidence(&blocks);
        let duration_ms = start.elapsed().as_millis() as u64;
        
        eprintln!("✅ OCR complete: {} blocks, {:.1}% confidence, {}ms (Ctrl+R for OCR menu)",
            blocks.len(), confidence * 100.0, duration_ms);
        
        Ok(OcrResult {
            blocks,
            confidence,
            was_needed: matches!(mode, OcrMode::Force | OcrMode::Replace),
            duration_ms,
        })
    }
    
    #[cfg(not(feature = "ocr"))]
    pub async fn process(&mut self, 
                        _image: &DynamicImage, 
                        _mode: OcrMode) -> Result<OcrResult> {
        eprintln!("⚠️ OCR not available - compile with --features ocr");
        Ok(OcrResult {
            blocks: vec![],
            confidence: 0.0,
            was_needed: false,
            duration_ms: 0,
        })
    }
}

#[cfg(feature = "ocr")]
fn preprocess_image(img: &DynamicImage) -> Result<DynamicImage> {
    use imageproc::contrast::adaptive_threshold;
    
    let gray = img.to_luma8();
    
    // Simple binarization for now
    let binary = adaptive_threshold(&gray, 11);
    
    Ok(DynamicImage::ImageLuma8(binary))
}

#[cfg(feature = "ocr")]
fn build_text_blocks(text: String) -> Result<Vec<TextBlock>> {
    let mut blocks = Vec::new();
    
    // If OCR returns very little or no text, it's likely an image
    if text.trim().is_empty() || text.trim().len() < 10 {
        // Add a placeholder for the image
        blocks.push(TextBlock {
            text: "[IMAGE]".to_string(),
            bbox: BoundingBox {
                x: 10.0,
                y: 10.0,
                width: 70.0,
                height: 20.0,
            },
            confidence: 1.0, // We're confident it's an image
        });
        return Ok(blocks);
    }
    
    // Convert OCR text output to our TextBlock format
    for (idx, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        
        // Convert any superscripts/subscripts to ASCII notation
        let text = line.chars()
            .map(|ch| convert_super_subscript_to_ascii(ch))
            .collect::<String>();
        
        // Create a simple bounding box (ocrs doesn't expose detailed bbox)
        let bbox = BoundingBox {
            x: 10.0,
            y: idx as f32 * 25.0, // Simple line spacing
            width: text.len() as f32 * 10.0,
            height: 20.0,
        };
        
        blocks.push(TextBlock {
            text,
            bbox,
            confidence: 0.95, // Default confidence
        });
    }
    
    // If we have very few text blocks, add an [IMAGE] indicator
    if blocks.len() <= 2 {
        blocks.push(TextBlock {
            text: "[IMAGE]".to_string(),
            bbox: BoundingBox {
                x: 10.0,
                y: (blocks.len() as f32 + 1.0) * 25.0,
                width: 70.0,
                height: 20.0,
            },
            confidence: 0.9,
        });
    }
    
    Ok(blocks)
}

#[cfg(feature = "ocr")]
fn calculate_confidence(blocks: &[TextBlock]) -> f32 {
    if blocks.is_empty() {
        return 0.0;
    }
    
    let sum: f32 = blocks.iter().map(|b| b.confidence).sum();
    sum / blocks.len() as f32
}