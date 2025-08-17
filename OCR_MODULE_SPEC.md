# OCR Module Specification for Chonker 7.51

## Overview
Pure Rust OCR integration using `ocrs` crate for text extraction from scanned PDFs and images.

## Dependencies

```toml
# Add to Cargo.toml
[dependencies]
ocrs = "0.8"           # Pure Rust OCR engine
rten = "0.13"          # ONNX runtime for ocrs
rten-imageproc = "0.13" # Image preprocessing
imageproc = "0.25"     # Image manipulation
dirs = "5.0"           # Cache directory

[features]
ocr = ["ocrs", "rten", "rten-imageproc", "imageproc"]
```

## Core OCR Engine Module

```rust
// src/ocr/engine.rs
use ocrs::{OcrEngine, OcrEngineParams};
use rten::Model;
use image::DynamicImage;
use anyhow::Result;

pub struct OcrLayer {
    engine: Option<OcrEngine>,
    detection_model: Option<Model>,
    recognition_model: Option<Model>,
    initialized: bool,
}

#[derive(Debug, Clone)]
pub enum OcrMode {
    Detect,    // Check if OCR is needed
    Overlay,   // Add text layer to existing
    Replace,   // Strip and rebuild text layer
    Force,     // Force OCR even if text exists
}

#[derive(Debug, Clone)]
pub enum OcrNeed {
    HasText,      // Good text layer exists
    NeedsOcr,     // No text layer found
    BadOcr,       // Corrupted/garbage text
    MixedContent, // Some text, some images
}

#[derive(Debug, Clone)]
pub struct OcrResult {
    pub blocks: Vec<TextBlock>,
    pub words: Vec<Word>,
    pub confidence: f32,
    pub was_needed: bool,
    pub duration_ms: u64,
}

#[derive(Debug, Clone)]
pub struct TextBlock {
    pub text: String,
    pub bbox: BoundingBox,
    pub confidence: f32,
    pub words: Vec<Word>,
}

#[derive(Debug, Clone)]
pub struct Word {
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
            engine: None,
            detection_model: None,
            recognition_model: None,
            initialized: false,
        }
    }
    
    pub async fn lazy_init(&mut self) -> Result<()> {
        if self.initialized {
            return Ok(());
        }
        
        eprintln!("🔍 Initializing OCR engine...");
        let models = ensure_models().await?;
        
        self.detection_model = Some(models.detection);
        self.recognition_model = Some(models.recognition);
        
        let params = OcrEngineParams {
            detection_model: Some(self.detection_model.as_ref().unwrap().clone()),
            recognition_model: Some(self.recognition_model.as_ref().unwrap().clone()),
        };
        
        self.engine = Some(OcrEngine::new(params)?);
        self.initialized = true;
        eprintln!("✅ OCR engine ready");
        
        Ok(())
    }
    
    pub fn analyze_page(&self, page: &PdfPage) -> OcrNeed {
        let extracted_text = page.extract_text();
        let page_area = page.width() * page.height();
        let text_ratio = extracted_text.len() as f32 / page_area;
        
        let has_images = page.images_count() > 0;
        let has_garbage = extracted_text.chars().any(|c| 
            c == '�' || c == '□' || c == '�' || c == '\u{fffd}'
        );
        
        match (text_ratio, has_images, has_garbage) {
            (r, true, _) if r < 0.01 => OcrNeed::NeedsOcr,
            (r, _, true) if r < 0.5 => OcrNeed::BadOcr,
            (r, true, false) if r < 0.3 => OcrNeed::MixedContent,
            _ => OcrNeed::HasText,
        }
    }
    
    pub async fn process(&mut self, 
                        page: &PdfPage, 
                        mode: OcrMode) -> Result<OcrResult> {
        let start = std::time::Instant::now();
        
        // Ensure initialized
        self.lazy_init().await?;
        
        // 1. Render page to high-res image
        let img = render_page_for_ocr(page)?;
        
        // 2. Preprocess
        let processed = preprocess_image(&img)?;
        
        // 3. Run OCR
        let engine = self.engine.as_ref().unwrap();
        let ocr_input = engine.prepare_input(processed)?;
        
        // Detect text regions
        let word_rects = engine.detect_words(&ocr_input)?;
        
        // Recognize text
        let line_texts = engine.recognize_text(&ocr_input, &word_rects)?;
        
        // 4. Build spatial text blocks
        let blocks = build_text_blocks(&line_texts, page.dimensions())?;
        
        let confidence = calculate_confidence(&blocks);
        let duration_ms = start.elapsed().as_millis() as u64;
        
        Ok(OcrResult {
            blocks,
            words: extract_words(&line_texts),
            confidence,
            was_needed: matches!(mode, OcrMode::Force | OcrMode::Replace),
            duration_ms,
        })
    }
}

fn render_page_for_ocr(page: &PdfPage) -> Result<DynamicImage> {
    // Render at 300 DPI for OCR
    let scale = 300.0 / 72.0;  // 72 DPI is default
    let width = (page.width() * scale) as u32;
    let height = (page.height() * scale) as u32;
    
    page.render(width, height)
}

fn preprocess_image(img: &DynamicImage) -> Result<DynamicImage> {
    use imageproc::contrast::adaptive_threshold;
    use imageproc::geometric_transformations::rotate;
    
    let gray = img.to_luma8();
    
    // Deskew if needed
    let angle = detect_skew(&gray)?;
    let deskewed = if angle.abs() > 0.5 {
        rotate(&gray, angle, Interpolation::Bilinear)
    } else {
        gray
    };
    
    // Binarize
    let binary = adaptive_threshold(&deskewed, 11);
    
    Ok(DynamicImage::ImageLuma8(binary))
}
```

## PDF Layer Integration

```rust
// src/ocr/pdf_layer.rs
use pdfium_render::prelude::*;

pub trait PdfOcrOps {
    fn strip_text_layer(&mut self) -> Result<()>;
    fn add_invisible_text(&mut self, blocks: Vec<TextBlock>) -> Result<()>;
    fn has_text_layer(&self) -> bool;
    fn get_text_coverage(&self) -> f32;
}

impl PdfOcrOps for PdfPage {
    fn strip_text_layer(&mut self) -> Result<()> {
        // Remove all text objects from page
        let objects = self.objects_mut();
        objects.retain(|obj| !obj.is_text());
        Ok(())
    }
    
    fn add_invisible_text(&mut self, blocks: Vec<TextBlock>) -> Result<()> {
        for block in blocks {
            // Add transparent text at exact coordinates
            let text_obj = PdfPageTextObject::new(
                self.document(),
                &block.text,
                &Font::helvetica(),
                1.0,  // Very small font
            )?;
            
            // Position at block coordinates
            text_obj.set_matrix(
                1.0, 0.0, 0.0, 1.0,
                block.bbox.x, 
                block.bbox.y,
            )?;
            
            // Make invisible
            text_obj.set_fill_color(Color::new(0, 0, 0, 0))?;
            text_obj.set_render_mode(TextRenderMode::Invisible)?;
            
            self.objects_mut().add_text_object(text_obj)?;
        }
        Ok(())
    }
    
    fn has_text_layer(&self) -> bool {
        !self.text().trim().is_empty()
    }
    
    fn get_text_coverage(&self) -> f32 {
        let text_len = self.text().len() as f32;
        let page_area = self.width() * self.height();
        text_len / page_area
    }
}
```

## UI Integration

```rust
// src/ocr/ui.rs
use crossterm::style::{Color, SetForegroundColor};

#[derive(Debug, Clone)]
pub enum OcrStatus {
    Idle,
    Analyzing,
    Processing(f32),  // Progress 0.0-1.0
    Complete(OcrStats),
    Error(String),
}

#[derive(Debug, Clone)]
pub struct OcrStats {
    pub words: usize,
    pub confidence: f32,
    pub duration_ms: u64,
}

pub struct OcrMenu {
    pub visible: bool,
    pub options: Vec<OcrMenuOption>,
    pub selected: usize,
    pub status: OcrStatus,
}

#[derive(Debug, Clone)]
pub enum OcrMenuOption {
    Auto,     // Detect and OCR if needed
    Force,    // OCR even if text exists
    Repair,   // Strip bad text and re-OCR
    Batch,    // Process entire document
    Cancel,
}

impl OcrMenu {
    pub fn new() -> Self {
        Self {
            visible: false,
            options: vec![
                OcrMenuOption::Auto,
                OcrMenuOption::Force,
                OcrMenuOption::Repair,
                OcrMenuOption::Batch,
                OcrMenuOption::Cancel,
            ],
            selected: 0,
            status: OcrStatus::Idle,
        }
    }
    
    pub fn render(&self, stdout: &mut io::Stdout, area: Rect) -> Result<()> {
        if !self.visible {
            return Ok(());
        }
        
        // Draw OCR overlay at bottom
        let y = area.height - 3;
        
        // Status line
        execute!(
            stdout,
            MoveTo(0, y),
            SetForegroundColor(Color::Yellow),
            Print("─".repeat(area.width as usize)),
            ResetColor
        )?;
        
        // Menu or progress
        execute!(
            stdout,
            MoveTo(0, y + 1),
            match &self.status {
                OcrStatus::Idle => {
                    Print("[A]uto  [F]orce  [R]epair  [B]atch  [ESC] Cancel")
                },
                OcrStatus::Analyzing => {
                    Print("🔍 Analyzing page...")
                },
                OcrStatus::Processing(p) => {
                    let bar_width = 20;
                    let filled = (bar_width as f32 * p) as usize;
                    Print(format!("OCR: [{}{}] {:.0}%", 
                        "█".repeat(filled),
                        "░".repeat(bar_width - filled),
                        p * 100.0
                    ))
                },
                OcrStatus::Complete(stats) => {
                    Print(format!("✅ OCR Complete: {} words, {:.1}% confidence, {}ms",
                        stats.words, stats.confidence * 100.0, stats.duration_ms
                    ))
                },
                OcrStatus::Error(msg) => {
                    SetForegroundColor(Color::Red);
                    Print(format!("❌ OCR Error: {}", msg));
                    ResetColor
                },
            }
        )?;
        
        Ok(())
    }
}
```

## Model Management

```rust
// src/ocr/models.rs
use std::path::PathBuf;
use dirs;

const DETECTION_MODEL_URL: &str = 
    "https://ocrs-models.s3.amazonaws.com/text-detection.rten";
const RECOGNITION_MODEL_URL: &str = 
    "https://ocrs-models.s3.amazonaws.com/text-recognition.rten";

pub struct OcrModels {
    pub detection: Model,
    pub recognition: Model,
}

pub async fn ensure_models() -> Result<OcrModels> {
    let cache_dir = dirs::cache_dir()
        .ok_or_else(|| anyhow!("No cache directory"))?
        .join("chonker7")
        .join("ocr");
    
    fs::create_dir_all(&cache_dir)?;
    
    let detection_path = cache_dir.join("text-detection.rten");
    let recognition_path = cache_dir.join("text-recognition.rten");
    
    // Download if missing
    if !detection_path.exists() {
        eprintln!("📥 Downloading OCR detection model (6MB)...");
        download_file(DETECTION_MODEL_URL, &detection_path).await?;
    }
    
    if !recognition_path.exists() {
        eprintln!("📥 Downloading OCR recognition model (6MB)...");
        download_file(RECOGNITION_MODEL_URL, &recognition_path).await?;
    }
    
    // Load models
    let detection = Model::load(&detection_path)?;
    let recognition = Model::load(&recognition_path)?;
    
    Ok(OcrModels {
        detection,
        recognition,
    })
}

async fn download_file(url: &str, path: &PathBuf) -> Result<()> {
    let response = reqwest::get(url).await?;
    let bytes = response.bytes().await?;
    fs::write(path, bytes)?;
    Ok(())
}
```

## Cache Strategy

```rust
// src/ocr/cache.rs
use lru::LruCache;
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrCache {
    pub version: String,
    pub entries: Vec<OcrCacheEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrCacheEntry {
    pub page: usize,
    pub hash: String,  // Page content hash
    pub result: OcrResult,
    pub timestamp: u64,
}

lazy_static! {
    static ref OCR_CACHE: Mutex<LruCache<(PathBuf, usize), OcrResult>> = 
        Mutex::new(LruCache::new(NonZeroUsize::new(50).unwrap()));
}

pub fn save_ocr_cache(pdf_path: &Path, cache: &OcrCache) -> Result<()> {
    let cache_path = pdf_path.with_extension("ocr");
    let json = serde_json::to_string_pretty(cache)?;
    fs::write(cache_path, json)?;
    Ok(())
}

pub fn load_ocr_cache(pdf_path: &Path) -> Result<Option<OcrCache>> {
    let cache_path = pdf_path.with_extension("ocr");
    if !cache_path.exists() {
        return Ok(None);
    }
    
    let json = fs::read_to_string(cache_path)?;
    let cache: OcrCache = serde_json::from_str(&json)?;
    
    // Check version compatibility
    if cache.version != env!("CARGO_PKG_VERSION") {
        return Ok(None);  // Invalidate old cache
    }
    
    Ok(Some(cache))
}
```

## Integration in Main App

```rust
// main.rs modifications
mod ocr;
use ocr::{OcrLayer, OcrMenu, OcrStatus};

pub struct App {
    // ... existing fields ...
    pub ocr: OcrLayer,
    pub ocr_menu: OcrMenu,
}

// In keyboard.rs
match key.code {
    // Change Load hotkey from Ctrl+O to Ctrl+L
    KeyCode::Char('l') if key.modifiers.contains(MOD_KEY) => {
        app.display_mode = DisplayMode::FilePicker;
    }
    
    // New OCR hotkey: Ctrl+O
    KeyCode::Char('o') if key.modifiers.contains(MOD_KEY) => {
        let need = app.ocr.analyze_page(&current_page);
        app.status_message = match need {
            OcrNeed::HasText => "Page has text layer (OCR not needed)".into(),
            OcrNeed::NeedsOcr => "No text found - Press A for Auto OCR".into(),
            OcrNeed::BadOcr => "Poor text quality - Press R to Repair".into(),
            OcrNeed::MixedContent => "Mixed content - Press F to Force OCR".into(),
        };
        app.ocr_menu.visible = true;
    }
    
    // OCR menu handlers
    KeyCode::Char('a') if app.ocr_menu.visible => {
        app.ocr_menu.status = OcrStatus::Processing(0.0);
        
        tokio::spawn(async move {
            let result = app.ocr.process(&page, OcrMode::Auto).await?;
            app.pdf.add_invisible_text(result.blocks)?;
            app.ocr_menu.status = OcrStatus::Complete(OcrStats {
                words: result.words.len(),
                confidence: result.confidence,
                duration_ms: result.duration_ms,
            });
            app.extract_current_page().await?;  // Refresh
        });
    }
    
    KeyCode::Esc if app.ocr_menu.visible => {
        app.ocr_menu.visible = false;
        app.ocr_menu.status = OcrStatus::Idle;
    }
}
```

## Performance Targets

- **300 DPI A4 page**:
  - Render: 50ms
  - Detection: 200ms  
  - Recognition: 300ms
  - Total: ~600ms per page

- **Memory**:
  - Models: ~12MB (loaded once)
  - Runtime: ~100MB peak

## Visual Feedback States

```
Normal:        [PDF]━━━━━━━━━━━━━━━━━━━━━━━━━━[TEXT]  Page 1/10
OCR Menu:      [PDF]━━━━━━━━━━━━━━━━━━━━━━━━━━[TEXT]  Page 1/10
               ───────────────────────────────────────────────
               [A]uto  [F]orce  [R]epair  [B]atch  [ESC] Cancel

Processing:    [PDF]━━━━━━━━━━━━━━━━━━━━━━━━━━[TEXT]  Page 1/10
               ───────────────────────────────────────────────
               OCR: [████████░░░░░░░░░░] 42%

Complete:      [PDF]━━━━━━━━━━━━━━━━━━━━━━━━━━[TEXT]  Page 1/10
               ───────────────────────────────────────────────
               ✅ OCR Complete: 523 words, 98.5% confidence, 587ms
```

## Error Handling

```rust
pub enum OcrError {
    ModelsNotFound,
    ModelDownloadFailed,
    LowConfidence(f32),
    PageTooComplex(usize),  // element count
    OutOfMemory,
    Timeout,
}

impl Display for OcrError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            OcrError::ModelsNotFound => 
                write!(f, "OCR models not found. Run: chonker7 --download-ocr"),
            OcrError::LowConfidence(conf) => 
                write!(f, "OCR confidence too low ({:.1}%). Try Force mode", conf * 100.0),
            OcrError::PageTooComplex(count) => 
                write!(f, "Page too complex ({} elements), skipping OCR", count),
            _ => write!(f, "OCR error: {:?}", self),
        }
    }
}
```

## Implementation Phases

### Phase 1: Basic OCR (v7.51)
- [x] Add ocrs dependencies
- [ ] Basic OCR engine integration
- [ ] Simple overlay UI
- [ ] Add invisible text to PDF
- [ ] Ctrl+O hotkey

### Phase 2: Advanced Features (v7.52)
- [ ] Progress bar during OCR
- [ ] LRU caching
- [ ] Sidecar file persistence
- [ ] Strip and rebuild mode
- [ ] Batch processing

### Phase 3: Optimization (v7.53)
- [ ] Parallel page processing
- [ ] Smart preprocessing
- [ ] Confidence thresholds
- [ ] Language detection
- [ ] Multi-column support

## Testing

```bash
# Test OCR detection
cargo test ocr::tests::test_detection

# Test with scanned PDF
./target/release/chonker7 test_data/scanned.pdf

# Benchmark
cargo bench ocr_performance
```

## Notes

1. **Pure Rust**: Uses ocrs crate, no external dependencies
2. **Lazy Loading**: Models load only when OCR is triggered
3. **Invisible Text**: Preserves PDF searchability
4. **Smart Detection**: Analyzes if OCR is actually needed
5. **Progress Feedback**: Real-time progress during processing
6. **Cache Friendly**: Results cached in memory and disk

This spec provides a complete OCR integration that maintains Chonker's philosophy of being fast, efficient, and user-friendly while adding powerful OCR capabilities for scanned documents.