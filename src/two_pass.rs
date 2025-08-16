// Two-pass extraction architecture
// Pass 1: PDFium raw extraction (deterministic, fast)
// Pass 2: LayoutLM enrichment (probabilistic, slow)

use anyhow::Result;
use lru::LruCache;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use lazy_static::lazy_static;

use crate::content_extractor::{CharacterData, PageData};

// Pass 1: Raw extraction data from PDFium
#[derive(Debug, Clone)]
pub struct Pass1Data {
    pub characters: Vec<CharacterData>,
    pub page_width: f32,
    pub page_height: f32,
    pub reading_order: Vec<usize>,  // Index order for reading
    pub extracted_at: std::time::SystemTime,
}

// Pass 2: ML-enriched semantic understanding
#[derive(Debug, Clone)]
pub struct Pass2Data {
    pub base: Pass1Data,  // Keep Pass1 data
    pub entities: Vec<Entity>,
    pub tables: Vec<TableStructure>,
    pub relations: Vec<Relation>,
    pub layout_regions: Vec<LayoutRegion>,
    pub confidence: f32,
}

#[derive(Debug, Clone)]
pub struct Entity {
    pub text: String,
    pub entity_type: EntityType,
    pub char_indices: Vec<usize>,  // Indices into Pass1Data.characters
    pub confidence: f32,
}

#[derive(Debug, Clone)]
pub enum EntityType {
    Header,
    Value,      // Monetary/numeric
    Label,      // Form field label
    TableCell,
    Text,       // Regular text
}

#[derive(Debug, Clone)]
pub struct TableStructure {
    pub rows: Vec<TableRow>,
    pub confidence: f32,
}

#[derive(Debug, Clone)]
pub struct TableRow {
    pub cells: Vec<TableCell>,
}

#[derive(Debug, Clone)]
pub struct TableCell {
    pub content: String,
    pub char_indices: Vec<usize>,
    pub col_span: usize,
    pub row_span: usize,
}

#[derive(Debug, Clone)]
pub struct Relation {
    pub from_entity: usize,
    pub to_entity: usize,
    pub relation_type: RelationType,
    pub confidence: f32,
}

#[derive(Debug, Clone)]
pub enum RelationType {
    LabelValue,
    HeaderContent,
    TableRelation,
}

#[derive(Debug, Clone)]
pub struct LayoutRegion {
    pub region_type: RegionType,
    pub char_indices: Vec<usize>,
    pub confidence: f32,
}

#[derive(Debug, Clone)]
pub enum RegionType {
    Title,
    Paragraph,
    List,
    Table,
    Caption,
    Footer,
    Header,
}

// Cache for both passes
lazy_static! {
    static ref PASS1_CACHE: Mutex<LruCache<(PathBuf, usize), Pass1Data>> = 
        Mutex::new(LruCache::new(NonZeroUsize::new(20).unwrap()));
    
    static ref PASS2_CACHE: Mutex<LruCache<(PathBuf, usize), Pass2Data>> = 
        Mutex::new(LruCache::new(NonZeroUsize::new(10).unwrap()));
}

/// Extract Pass1 data (PDFium) with caching
pub fn extract_pass1(pdf_path: &Path, page_num: usize) -> Result<Pass1Data> {
    let key = (pdf_path.to_path_buf(), page_num);
    
    // Check cache first
    {
        let mut cache = PASS1_CACHE.lock().unwrap();
        if let Some(cached) = cache.get(&key) {
            eprintln!("Pass1 cache hit for page {}", page_num);
            return Ok(cached.clone());
        }
    }
    
    // Extract using PDFium
    eprintln!("Pass1: Extracting raw data from page {}", page_num);
    let pass1 = extract_raw_from_pdfium(pdf_path, page_num)?;
    
    // Cache the result
    {
        let mut cache = PASS1_CACHE.lock().unwrap();
        cache.put(key, pass1.clone());
    }
    
    Ok(pass1)
}

/// Enrich with Pass2 (ML) with caching and fallback
pub fn enrich_pass2(pass1: &Pass1Data, pdf_path: &Path, page_num: usize) -> Result<Pass2Data> {
    let key = (pdf_path.to_path_buf(), page_num);
    
    // Check cache first
    {
        let mut cache = PASS2_CACHE.lock().unwrap();
        if let Some(cached) = cache.get(&key) {
            eprintln!("Pass2 cache hit for page {}", page_num);
            return Ok(cached.clone());
        }
    }
    
    // Try ML enrichment
    eprintln!("Pass2: Enriching with ML for page {}", page_num);
    
    #[cfg(feature = "ml")]
    {
        match enrich_with_ml(pass1, pdf_path, page_num) {
            Ok(pass2) => {
                // Cache successful enrichment
                let mut cache = PASS2_CACHE.lock().unwrap();
                cache.put(key, pass2.clone());
                return Ok(pass2);
            }
            Err(e) => {
                eprintln!("Pass2 ML enrichment failed: {}, falling back to Pass1", e);
            }
        }
    }
    
    // Fallback: Convert Pass1 to minimal Pass2
    Ok(Pass2Data {
        base: pass1.clone(),
        entities: Vec::new(),
        tables: Vec::new(),
        relations: Vec::new(),
        layout_regions: Vec::new(),
        confidence: 0.0,
    })
}

/// Clear all caches
pub fn clear_caches() {
    PASS1_CACHE.lock().unwrap().clear();
    PASS2_CACHE.lock().unwrap().clear();
    eprintln!("Caches cleared");
}

// Internal implementation functions

fn extract_raw_from_pdfium(pdf_path: &Path, page_num: usize) -> Result<Pass1Data> {
    use pdfium_render::prelude::*;
    
    let pdfium = crate::pdf_renderer::get_pdfium_instance();
    let document = pdfium.load_pdf_from_file(pdf_path, None)?;
    let page = document.pages().get(page_num as u16)?;
    
    let page_width = page.width().value;
    let page_height = page.height().value;
    
    // Extract characters
    let characters = crate::content_extractor::extract_characters_from_page(&page)?;
    
    // Simple reading order: left-to-right, top-to-bottom
    let mut reading_order: Vec<usize> = (0..characters.len()).collect();
    reading_order.sort_by(|&a, &b| {
        let char_a = &characters[a];
        let char_b = &characters[b];
        
        // Sort by Y first (with tolerance), then X
        if (char_a.baseline_y - char_b.baseline_y).abs() > 2.0 {
            char_a.baseline_y.partial_cmp(&char_b.baseline_y).unwrap()
        } else {
            char_a.x.partial_cmp(&char_b.x).unwrap()
        }
    });
    
    Ok(Pass1Data {
        characters,
        page_width,
        page_height,
        reading_order,
        extracted_at: std::time::SystemTime::now(),
    })
}

#[cfg(feature = "ml")]
fn enrich_with_ml(pass1: &Pass1Data, _pdf_path: &Path, _page_num: usize) -> Result<Pass2Data> {
    // TODO: Actual LayoutLM integration
    // For now, just do simple pattern-based enrichment
    
    let mut entities = Vec::new();
    let mut current_text = String::new();
    let mut current_indices = Vec::new();
    
    // Simple entity detection based on patterns
    for &idx in &pass1.reading_order {
        let ch = &pass1.characters[idx];
        current_text.push(ch.unicode);
        current_indices.push(idx);
        
        // Detect end of word/entity
        if ch.unicode.is_whitespace() || idx == pass1.reading_order.last().copied().unwrap_or(0) {
            if !current_text.trim().is_empty() {
                let entity_type = detect_entity_type(&current_text);
                if !matches!(entity_type, EntityType::Text) {
                    entities.push(Entity {
                        text: current_text.trim().to_string(),
                        entity_type,
                        char_indices: current_indices.clone(),
                        confidence: 0.8,
                    });
                }
            }
            current_text.clear();
            current_indices.clear();
        }
    }
    
    Ok(Pass2Data {
        base: pass1.clone(),
        entities,
        tables: Vec::new(),
        relations: Vec::new(),
        layout_regions: Vec::new(),
        confidence: 0.7,
    })
}

fn detect_entity_type(text: &str) -> EntityType {
    let trimmed = text.trim();
    
    // Simple heuristics
    if trimmed.starts_with('$') || trimmed.parse::<f64>().is_ok() {
        EntityType::Value
    } else if trimmed.ends_with(':') {
        EntityType::Label
    } else if trimmed.chars().all(|c| c.is_uppercase() || c.is_whitespace()) && trimmed.len() > 3 {
        EntityType::Header
    } else {
        EntityType::Text
    }
}