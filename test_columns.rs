#!/usr/bin/env rust-script
//! Test multi-column detection in chonker7
//! ```cargo
//! [dependencies]
//! chonker7 = { path = "." }
//! anyhow = "1.0"
//! ```

use anyhow::Result;
use std::path::Path;

fn main() -> Result<()> {
    // Test multi-column detection on a known multi-column PDF
    let test_pdf = Path::new("/Users/jack/Desktop/Testing_the_waters_for_floating_class_7.5M___Philadelphia_Daily_News_PA___February_17_2025__pX10.pdf");
    
    if !test_pdf.exists() {
        eprintln!("Test PDF not found. Using any available PDF.");
        // Find any PDF to test with
        let pdfs = std::fs::read_dir("/Users/jack/Desktop")?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().map_or(false, |ext| ext == "pdf"))
            .collect::<Vec<_>>();
            
        if pdfs.is_empty() {
            eprintln!("No PDFs found on desktop");
            return Ok(());
        }
        
        let test_file = &pdfs[0].path();
        eprintln!("Testing with: {}", test_file.display());
        test_extraction(test_file)?;
    } else {
        eprintln!("Testing multi-column PDF: {}", test_pdf.display());
        test_extraction(test_pdf)?;
    }
    
    Ok(())
}

fn test_extraction(pdf_path: &Path) -> Result<()> {
    // Extract first page
    let matrix = chonker7::content_extractor::extract_to_matrix(
        pdf_path,
        0,  // First page
        200, // Width
        100, // Height
    )?;
    
    // Check for column separation
    let mut has_column_gap = false;
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
                    break;
                }
                in_gap = false;
                gap_count = 0;
            }
        }
        
        if has_column_gap {
            break;
        }
    }
    
    // Print a sample of the extracted text
    eprintln!("\n=== Sample extraction (first 10 lines) ===");
    for (i, row) in matrix.iter().take(10).enumerate() {
        let line: String = row.iter().collect();
        eprintln!("{:2}: {}", i, line.trim());
    }
    
    if has_column_gap {
        eprintln!("\n✅ Column separation detected - columns are being kept separate!");
    } else {
        eprintln!("\n⚠️ No clear column separation detected - might be single column or needs adjustment");
    }
    
    Ok(())
}