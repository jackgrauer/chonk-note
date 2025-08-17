use anyhow::Result;
use std::path::Path;

// Import from the crate
use chonker7::content_extractor;

#[tokio::main]
async fn main() -> Result<()> {
    // Test multi-column detection on a known multi-column PDF
    let test_pdf = Path::new("/Users/jack/Desktop/Testing_the_waters_for_floating_class_7.5M___Philadelphia_Daily_News_PA___February_17_2025__pX10.pdf");
    
    let pdf_path = if !test_pdf.exists() {
        eprintln!("Test PDF not found. Using any available PDF.");
        // Find any PDF to test with
        let pdfs: Vec<_> = std::fs::read_dir("/Users/jack/Desktop")?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().map_or(false, |ext| ext == "pdf"))
            .collect();
            
        if pdfs.is_empty() {
            eprintln!("No PDFs found on desktop");
            return Ok(());
        }
        
        pdfs[0].path()
    } else {
        test_pdf.to_path_buf()
    };
    
    eprintln!("Testing multi-column PDF: {}", pdf_path.display());
    
    // Extract first page
    let matrix = content_extractor::extract_to_matrix(
        &pdf_path,
        0,  // First page
        200, // Width
        100, // Height
    ).await?;
    
    // Check for column separation
    let mut has_column_gap = false;
    let mut max_gap = 0;
    
    for row in &matrix {
        // Look for rows with significant gaps (indicating column separation)
        let mut gap_count = 0;
        let mut in_gap = false;
        
        for ch in row {
            if *ch == ' ' {
                if !in_gap {
                    in_gap = true;
                }
                gap_count += 1;
            } else {
                if in_gap && gap_count > 10 {
                    // Found a significant gap
                    has_column_gap = true;
                    if gap_count > max_gap {
                        max_gap = gap_count;
                    }
                }
                in_gap = false;
                gap_count = 0;
            }
        }
    }
    
    // Print a sample of the extracted text
    eprintln!("\n=== Sample extraction (first 20 lines) ===");
    for (i, row) in matrix.iter().take(20).enumerate() {
        let line: String = row.iter().collect();
        if !line.trim().is_empty() {
            eprintln!("{:2}: {}", i, line);
        }
    }
    
    eprintln!("\n=== Analysis ===");
    if has_column_gap {
        eprintln!("✅ Column separation detected!");
        eprintln!("   Maximum gap width: {} spaces", max_gap);
        eprintln!("   Columns are being kept separate as intended.");
    } else {
        eprintln!("⚠️ No clear column separation detected");
        eprintln!("   This might be a single-column document or needs threshold adjustment.");
    }
    
    // Look for specific patterns that indicate column merging
    let mut merged_lines = 0;
    for row in &matrix {
        let line: String = row.iter().collect();
        // Check for patterns like mixed content from different columns
        if line.contains("   ") && line.len() > 100 {
            merged_lines += 1;
        }
    }
    
    if merged_lines > 5 {
        eprintln!("⚠️ Warning: {} lines appear to have merged content", merged_lines);
    } else {
        eprintln!("✅ No obvious merged lines detected");
    }
    
    Ok(())
}