use crate::content_extractor::CharacterData;
use std::collections::HashMap;

/// Types of entities that can be detected in documents
#[derive(Debug, Clone, PartialEq)]
pub enum EntityType {
    // Financial entities
    Value,           // Monetary values, percentages
    Date,            // Dates and time periods
    Organization,    // Company names, agencies
    Location,        // Addresses, cities, states
    
    // Document structure
    Header,          // Section headers
    Footer,          // Page footers
    TableCell,       // Table data cells
    TableHeader,     // Table column headers
    
    // Form fields
    FieldLabel,      // Form field labels
    FieldValue,      // Form field values
    
    // Generic
    Text,            // Regular text
    Unknown,         // Unclassified
}

/// A detected entity in the document
#[derive(Debug, Clone)]
pub struct Entity {
    pub text: String,
    pub entity_type: EntityType,
    pub confidence: f32,
    pub bbox: BoundingBox,
    pub characters: Vec<CharacterData>,
}

/// Bounding box for spatial location
#[derive(Debug, Clone)]
pub struct BoundingBox {
    pub x1: f32,
    pub y1: f32,
    pub x2: f32,
    pub y2: f32,
}

impl BoundingBox {
    pub fn from_characters(chars: &[CharacterData]) -> Self {
        if chars.is_empty() {
            return Self { x1: 0.0, y1: 0.0, x2: 0.0, y2: 0.0 };
        }
        
        let mut min_x = f32::MAX;
        let mut min_y = f32::MAX;
        let mut max_x = f32::MIN;
        let mut max_y = f32::MIN;
        
        for ch in chars {
            min_x = min_x.min(ch.x);
            min_y = min_y.min(ch.y);
            max_x = max_x.max(ch.x + ch.width);
            max_y = max_y.max(ch.y + ch.height);
        }
        
        Self {
            x1: min_x,
            y1: min_y,
            x2: max_x,
            y2: max_y,
        }
    }
}

/// Detected table structure
#[derive(Debug, Clone)]
pub struct Table {
    pub rows: Vec<TableRow>,
    pub bbox: BoundingBox,
    pub has_header: bool,
}

#[derive(Debug, Clone)]
pub struct TableRow {
    pub cells: Vec<TableCell>,
    pub is_header: bool,
}

#[derive(Debug, Clone)]
pub struct TableCell {
    pub text: String,
    pub row_idx: usize,
    pub col_idx: usize,
    pub row_span: usize,
    pub col_span: usize,
    pub bbox: BoundingBox,
    pub is_numeric: bool,
}

/// Relationship between entities
#[derive(Debug, Clone)]
pub struct EntityRelation {
    pub from_entity: usize, // Index in entities vec
    pub to_entity: usize,
    pub relation_type: RelationType,
    pub confidence: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RelationType {
    LabelValue,      // Form field label -> value
    HeaderContent,   // Header -> content section
    TableRelation,   // Table header -> cell
    Sequential,      // Reading order
}

/// Complete document understanding result
pub struct DocumentUnderstanding {
    pub entities: Vec<Entity>,
    pub tables: Vec<Table>,
    pub relations: Vec<EntityRelation>,
    pub reading_order: Vec<usize>, // Indices into entities vec
    pub metadata: DocumentMetadata,
}

#[derive(Debug, Clone)]
pub struct DocumentMetadata {
    pub page_width: f32,
    pub page_height: f32,
    pub detected_language: Option<String>,
    pub document_type: DocumentType,
    pub confidence_scores: HashMap<String, f32>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DocumentType {
    Form,
    Report,
    Invoice,
    Contract,
    Letter,
    Table,
    Mixed,
    Unknown,
}

impl DocumentUnderstanding {
    pub fn new() -> Self {
        Self {
            entities: Vec::new(),
            tables: Vec::new(),
            relations: Vec::new(),
            reading_order: Vec::new(),
            metadata: DocumentMetadata {
                page_width: 0.0,
                page_height: 0.0,
                detected_language: None,
                document_type: DocumentType::Unknown,
                confidence_scores: HashMap::new(),
            },
        }
    }
    
    pub fn add_entity(&mut self, chars: CharacterData, entity_type: EntityType) {
        let text = chars.unicode.to_string();
        let bbox = BoundingBox::from_characters(&[chars.clone()]);
        
        self.entities.push(Entity {
            text,
            entity_type,
            confidence: 1.0,
            bbox,
            characters: vec![chars],
        });
    }
    
    pub fn add_table(&mut self, table: Table) {
        self.tables.push(table);
    }
    
    pub fn add_relation(&mut self, from: usize, to: usize, rel_type: RelationType) {
        self.relations.push(EntityRelation {
            from_entity: from,
            to_entity: to,
            relation_type: rel_type,
            confidence: 1.0,
        });
    }
    
    /// Convert to enhanced character grid for TEXT tab
    pub fn to_enhanced_grid(&self, width: usize, height: usize) -> Vec<Vec<char>> {
        let mut grid = vec![vec![' '; width]; height];
        
        // Place entities with their types indicated
        for entity in &self.entities {
            let marker = match entity.entity_type {
                EntityType::Value => '$',
                EntityType::Date => '@',
                EntityType::Organization => '©',
                EntityType::Header => '#',
                EntityType::TableCell => '|',
                _ => ' ',
            };
            
            // Place entity text with marker
            for ch in &entity.characters {
                let x = (ch.x * width as f32 / self.metadata.page_width) as usize;
                let y = (ch.y * height as f32 / self.metadata.page_height) as usize;
                
                if x < width && y < height {
                    grid[y][x] = ch.unicode;
                }
            }
        }
        
        grid
    }
    
    /// Convert to rich markdown for READER tab
    pub fn to_markdown(&self) -> String {
        let mut md = String::new();
        
        // Add document type header
        md.push_str(&format!("# Document Type: {:?}\n\n", self.metadata.document_type));
        
        // Group entities by type
        let mut headers = Vec::new();
        let mut values = Vec::new();
        let mut orgs = Vec::new();
        
        for entity in &self.entities {
            match entity.entity_type {
                EntityType::Header => headers.push(&entity.text),
                EntityType::Value => values.push(&entity.text),
                EntityType::Organization => orgs.push(&entity.text),
                _ => {}
            }
        }
        
        if !headers.is_empty() {
            md.push_str("## Headers\n");
            for h in headers {
                md.push_str(&format!("- {}\n", h));
            }
            md.push_str("\n");
        }
        
        if !orgs.is_empty() {
            md.push_str("## Organizations\n");
            for o in orgs {
                md.push_str(&format!("- **{}**\n", o));
            }
            md.push_str("\n");
        }
        
        if !values.is_empty() {
            md.push_str("## Values\n");
            for v in values {
                md.push_str(&format!("- `{}`\n", v));
            }
            md.push_str("\n");
        }
        
        // Add tables
        for table in &self.tables {
            md.push_str("## Table\n\n");
            
            // Table header
            if table.has_header && !table.rows.is_empty() {
                md.push_str("|");
                for cell in &table.rows[0].cells {
                    md.push_str(&format!(" {} |", cell.text));
                }
                md.push_str("\n|");
                for _ in &table.rows[0].cells {
                    md.push_str(" --- |");
                }
                md.push_str("\n");
                
                // Table body
                for row in &table.rows[1..] {
                    md.push_str("|");
                    for cell in &row.cells {
                        if cell.is_numeric {
                            md.push_str(&format!(" `{}` |", cell.text));
                        } else {
                            md.push_str(&format!(" {} |", cell.text));
                        }
                    }
                    md.push_str("\n");
                }
            } else {
                // No header
                for row in &table.rows {
                    md.push_str("|");
                    for cell in &row.cells {
                        md.push_str(&format!(" {} |", cell.text));
                    }
                    md.push_str("\n");
                }
            }
            md.push_str("\n");
        }
        
        // Add confidence scores
        if !self.metadata.confidence_scores.is_empty() {
            md.push_str("---\n\n_Confidence Scores:_\n");
            for (key, score) in &self.metadata.confidence_scores {
                md.push_str(&format!("- {}: {:.1}%\n", key, score * 100.0));
            }
        }
        
        md
    }
}