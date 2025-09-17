// Spotlight Indexing for PDF Content
// Enables system-wide search of PDF content through Spotlight
use anyhow::Result;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::collections::HashMap;

pub struct SpotlightIndexer {
    index_dir: PathBuf,
    metadata_cache: HashMap<PathBuf, PDFMetadata>,
}

#[derive(Clone, Debug)]
pub struct PDFMetadata {
    pub title: String,
    pub author: String,
    pub content: String,
    pub page_count: usize,
    pub file_path: PathBuf,
    pub last_modified: std::time::SystemTime,
    pub keywords: Vec<String>,
}

impl SpotlightIndexer {
    pub fn new() -> Self {
        let index_dir = PathBuf::from(
            std::env::var("HOME").unwrap_or_default()
        ).join(".chonker7").join("spotlight_index");

        fs::create_dir_all(&index_dir).ok();

        Self {
            index_dir,
            metadata_cache: HashMap::new(),
        }
    }

    // Index a PDF file for Spotlight search
    pub fn index_pdf(&mut self, pdf_path: &Path, content: &str) -> Result<()> {
        let metadata = self.extract_metadata(pdf_path, content)?;

        // Store in cache
        self.metadata_cache.insert(pdf_path.to_path_buf(), metadata.clone());

        // Create Spotlight metadata
        self.create_spotlight_metadata(pdf_path, &metadata)?;

        // Import to Spotlight using mdimport
        self.import_to_spotlight(pdf_path)?;

        Ok(())
    }

    // Extract metadata from PDF content
    fn extract_metadata(&self, pdf_path: &Path, content: &str) -> Result<PDFMetadata> {
        let file_meta = fs::metadata(pdf_path)?;

        // Extract keywords from content (simple word frequency)
        let keywords = self.extract_keywords(content);

        // Try to extract title from first line or filename
        let title = content.lines()
            .next()
            .and_then(|line| {
                if line.len() > 5 && line.len() < 100 {
                    Some(line.to_string())
                } else {
                    None
                }
            })
            .unwrap_or_else(|| {
                pdf_path.file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("Untitled")
                    .to_string()
            });

        Ok(PDFMetadata {
            title,
            author: String::from("Unknown"),
            content: content.to_string(),
            page_count: content.lines().count() / 50, // Rough estimate
            file_path: pdf_path.to_path_buf(),
            last_modified: file_meta.modified()?,
            keywords,
        })
    }

    // Extract keywords from content using simple frequency analysis
    fn extract_keywords(&self, content: &str) -> Vec<String> {
        let mut word_count = HashMap::new();
        let stop_words = vec!["the", "a", "an", "and", "or", "but", "in", "on", "at", "to", "for"];

        for word in content.split_whitespace() {
            let clean_word = word.to_lowercase()
                .chars()
                .filter(|c| c.is_alphanumeric())
                .collect::<String>();

            if clean_word.len() > 3 && !stop_words.contains(&clean_word.as_str()) {
                *word_count.entry(clean_word).or_insert(0) += 1;
            }
        }

        let mut words: Vec<_> = word_count.into_iter().collect();
        words.sort_by_key(|&(_, count)| std::cmp::Reverse(count));

        words.into_iter()
            .take(10)
            .map(|(word, _)| word)
            .collect()
    }

    // Create Spotlight metadata attributes
    fn create_spotlight_metadata(&self, pdf_path: &Path, metadata: &PDFMetadata) -> Result<()> {
        // Set extended attributes that Spotlight will index
        let keywords_str = metadata.keywords.join(", ");
        let attributes = vec![
            ("kMDItemTitle", &metadata.title),
            ("kMDItemAuthors", &metadata.author),
            ("kMDItemTextContent", &metadata.content),
            ("kMDItemKeywords", &keywords_str),
        ];

        for (attr, value) in attributes {
            Command::new("xattr")
                .arg("-w")
                .arg(format!("com.apple.metadata:{}", attr))
                .arg(value)
                .arg(pdf_path)
                .output()?;
        }

        // Create a .chonker7 metadata file for rich indexing
        let meta_file = self.index_dir.join(format!(
            "{}.meta",
            pdf_path.file_name().unwrap_or_default().to_string_lossy()
        ));

        let meta_content = format!(
            "Title: {}\nAuthor: {}\nPages: {}\nKeywords: {}\nPath: {}\n\n{}",
            metadata.title,
            metadata.author,
            metadata.page_count,
            metadata.keywords.join(", "),
            pdf_path.display(),
            &metadata.content[..metadata.content.len().min(1000)]
        );

        fs::write(meta_file, meta_content)?;

        Ok(())
    }

    // Import file to Spotlight index
    fn import_to_spotlight(&self, pdf_path: &Path) -> Result<()> {
        Command::new("mdimport")
            .arg("-d1")  // Debug level 1
            .arg(pdf_path)
            .output()?;

        Ok(())
    }

    // Search Spotlight index for content
    pub fn search(&self, query: &str) -> Result<Vec<SearchResult>> {
        let output = Command::new("mdfind")
            .arg("-onlyin")
            .arg(std::env::var("HOME").unwrap_or_default())
            .arg(format!("kMDItemTextContent == '*{}*'", query))
            .output()?;

        let results = String::from_utf8_lossy(&output.stdout);
        let mut search_results = Vec::new();

        for line in results.lines() {
            if line.ends_with(".pdf") {
                let path = PathBuf::from(line);
                if let Some(metadata) = self.metadata_cache.get(&path) {
                    search_results.push(SearchResult {
                        path: path.clone(),
                        title: metadata.title.clone(),
                        snippet: self.extract_snippet(&metadata.content, query),
                        relevance: self.calculate_relevance(&metadata.content, query),
                    });
                }
            }
        }

        search_results.sort_by(|a, b| b.relevance.partial_cmp(&a.relevance).unwrap());
        Ok(search_results)
    }

    // Extract a relevant snippet around the search term
    fn extract_snippet(&self, content: &str, query: &str) -> String {
        let lower_content = content.to_lowercase();
        let lower_query = query.to_lowercase();

        if let Some(pos) = lower_content.find(&lower_query) {
            let start = pos.saturating_sub(50);
            let end = (pos + query.len() + 50).min(content.len());

            let snippet = &content[start..end];
            format!("...{}...", snippet.trim())
        } else {
            content.chars().take(100).collect::<String>() + "..."
        }
    }

    // Calculate relevance score for search results
    fn calculate_relevance(&self, content: &str, query: &str) -> f32 {
        let lower_content = content.to_lowercase();
        let lower_query = query.to_lowercase();

        let count = lower_content.matches(&lower_query).count();
        let length_factor = 1.0 / (content.len() as f32).sqrt();

        count as f32 * length_factor * 100.0
    }

    // Enable Quick Search widget
    pub fn enable_quick_search(&self) -> Result<()> {
        // Register as a Spotlight plugin
        let plist_content = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleIdentifier</key>
    <string>com.chonker7.spotlight</string>
    <key>CFBundleName</key>
    <string>Chonker7 PDF Search</string>
    <key>UTImportedTypeDeclarations</key>
    <array>
        <dict>
            <key>UTTypeIdentifier</key>
            <string>com.chonker7.pdf-content</string>
            <key>UTTypeConformsTo</key>
            <array>
                <string>public.data</string>
                <string>public.content</string>
            </array>
        </dict>
    </array>
</dict>
</plist>"#;

        let plist_path = self.index_dir.join("Info.plist");
        fs::write(plist_path, plist_content)?;

        Ok(())
    }
}

pub struct SearchResult {
    pub path: PathBuf,
    pub title: String,
    pub snippet: String,
    pub relevance: f32,
}

// Integration with App
impl crate::App {
    pub fn index_current_pdf(&mut self) -> Result<()> {
        if let Some(pdf_path) = &self.current_pdf_path {
            let mut indexer = SpotlightIndexer::new();
            let content = self.rope.to_string();
            indexer.index_pdf(pdf_path, &content)?;

            // Log indexing
            if let Ok(mut file) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open("/Users/jack/chonker7_debug.log")
            {
                use std::io::Write;
                writeln!(file, "[SPOTLIGHT] Indexed PDF: {:?}", pdf_path).ok();
            }
        }
        Ok(())
    }

    pub fn search_indexed_pdfs(&self, query: &str) -> Result<Vec<SearchResult>> {
        let indexer = SpotlightIndexer::new();
        indexer.search(query)
    }

    pub fn enable_spotlight_integration(&mut self) -> Result<()> {
        let indexer = SpotlightIndexer::new();
        indexer.enable_quick_search()?;

        // Auto-index on PDF load
        if let Some(pdf_path) = &self.current_pdf_path {
            let mut indexer = SpotlightIndexer::new();
            let content = self.rope.to_string();
            indexer.index_pdf(pdf_path, &content)?;
        }

        Ok(())
    }
}