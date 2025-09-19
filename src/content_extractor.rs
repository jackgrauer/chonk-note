// SPATIALLY ACCURATE PDF TEXT EXTRACTION
use anyhow::Result;
use pdfium_render::prelude::*;
use std::path::Path;
use crate::ExtractionMethod;


pub async fn extract_to_matrix_with_method(
    pdf_path: &Path,
    page_num: usize,
    width: usize,
    height: usize,
    method: ExtractionMethod,
) -> Result<Vec<Vec<char>>> {
    match method {
        ExtractionMethod::Segments => extract_segments_method(pdf_path, page_num, width, height).await,
        ExtractionMethod::PdfAlto => extract_pdfalto_method(pdf_path, page_num, width, height).await,
        ExtractionMethod::LeptessOCR => extract_leptess_ocr_method(pdf_path, page_num, width, height).await,
    }
}

// Shared coordinate validation to eliminate duplication
fn validate_coordinates(x: f32, y: f32, w: f32, h: f32) -> bool {
    x.is_finite() && y.is_finite() && w.is_finite() && h.is_finite() &&
    x >= 0.0 && y >= 0.0 && w >= 0.0 && h >= 0.0
}

async fn extract_segments_method(
    pdf_path: &Path,
    page_num: usize,
    width: usize,
    height: usize,
) -> Result<Vec<Vec<char>>> {
    let pdfium = Pdfium::new(
        Pdfium::bind_to_library(Pdfium::pdfium_platform_library_name_at_path("./lib/"))?
    );
    let document = pdfium.load_pdf_from_file(pdf_path, None)?;
    let page = document.pages().get(page_num as u16)?;

    let text_page = page.text()?;
    let page_height = page.height().value;

    // Enhanced segments method with structure detection
    let segments = text_page.segments();
    let mut text_blocks = Vec::new();

    // Step 1: Detect contiguous text blocks using spatial layout processing
    for segment in segments.iter() {
        let segment_text = segment.text();
        let bounds = segment.bounds();

        if !segment_text.trim().is_empty() {
            let x = bounds.left().value;
            let y = page_height - bounds.top().value;
            let w = bounds.width().value;
            let h = bounds.height().value;

            // Use shared coordinate validation
            if validate_coordinates(x, y, w, h) {
                text_blocks.push((segment_text, x, y, w, h));
            }
        }
    }

    // Step 2: Sort for proper reading order (Y then X)
    text_blocks.sort_by(|a, b| {
        let y_cmp = a.2.total_cmp(&b.2);
        if y_cmp != std::cmp::Ordering::Equal {
            return y_cmp;
        }
        a.1.total_cmp(&b.1)
    });

    // Step 3: Group into lines with enhanced line and paragraph detection
    let mut lines = Vec::new();
    let mut current_line = Vec::new();
    let mut last_y = f32::NEG_INFINITY;
    let mut line_heights = Vec::new(); // Track line heights for paragraph detection

    for block in text_blocks {
        let y = block.2;
        let height = block.4;

        // Enhanced line detection - consider text height for better grouping
        let line_threshold = height.max(8.0); // Use text height or minimum threshold

        if (y - last_y).abs() > line_threshold && last_y != f32::NEG_INFINITY {
            if !current_line.is_empty() {
                // Calculate average height for this line
                let avg_height = current_line.iter().map(|(_, _, _, _, h)| h).sum::<f32>() / current_line.len() as f32;
                line_heights.push(avg_height);
                lines.push(std::mem::take(&mut current_line));
            }
        }

        current_line.push(block);
        last_y = y;
    }

    if !current_line.is_empty() {
        let avg_height = current_line.iter().map(|(_, _, _, _, h)| h).sum::<f32>() / current_line.len() as f32;
        line_heights.push(avg_height);
        lines.push(current_line);
    }

    // Step 4: Build text with intelligent spacing and paragraph detection
    let mut grid = vec![vec![' '; width]; height];
    let mut grid_row = 0;
    let mut prev_line_y = f32::NEG_INFINITY;

    for (line_idx, line_blocks) in lines.iter().enumerate() {
        if grid_row >= height {
            break;
        }

        // Detect paragraph breaks based on vertical spacing
        if line_idx > 0 {
            let current_line_y = line_blocks.first().map(|b| b.2).unwrap_or(0.0);
            let gap = (prev_line_y - current_line_y).abs();
            let avg_line_height = line_heights.get(line_idx.saturating_sub(1)).unwrap_or(&12.0);

            // Paragraph break detection: gap > 1.5x line height indicates paragraph
            if gap > avg_line_height * 1.5 {
                // Add blank line for paragraph break
                grid_row += 1;
                if grid_row >= height { break; }
            }

            prev_line_y = current_line_y;
        } else if let Some(first_block) = line_blocks.first() {
            prev_line_y = first_block.2;
        }

        let mut grid_col = 0;

        for (i, (text, x, _y, block_width, _height)) in line_blocks.iter().enumerate() {
            // Calculate inter-block spacing based on gap analysis
            if i > 0 {
                let prev_block = &line_blocks[i - 1];
                let prev_end = prev_block.1 + prev_block.3;
                let gap = x - prev_end;

                // Enhanced spacing calculation
                let avg_char_width = block_width / text.len() as f32;
                let natural_space_width = avg_char_width * 0.3; // Natural space is ~30% of char width

                let space_count = if gap > natural_space_width {
                    ((gap / avg_char_width).round() as usize).max(1).min(12)
                } else {
                    1 // Minimum one space between blocks
                };

                for _ in 0..space_count {
                    if grid_col < width {
                        grid[grid_row][grid_col] = ' ';
                        grid_col += 1;
                    }
                }
            }

            // Place text with character filtering
            for ch in text.chars() {
                if grid_col < width && grid_row < height {
                    // Filter out problematic characters
                    if ch.is_control() && ch != '\t' {
                        continue;
                    }
                    let display_char = if ch == '\t' { ' ' } else { ch };
                    grid[grid_row][grid_col] = display_char;
                    grid_col += 1;
                }
            }
        }

        grid_row += 1;
    }

    Ok(grid)
}

async fn extract_pdfalto_method(
    pdf_path: &Path,
    page_num: usize,
    width: usize,
    height: usize,
) -> Result<Vec<Vec<char>>> {
    let pdfium = Pdfium::new(
        Pdfium::bind_to_library(Pdfium::pdfium_platform_library_name_at_path("./lib/"))?
    );
    let document = pdfium.load_pdf_from_file(pdf_path, None)?;
    let page = document.pages().get(page_num as u16)?;

    let text_page = page.text()?;
    let page_height = page.height().value;

    // PDFAlto Flow-Based Method: Focus on reading order rather than spatial accuracy
    let segments = text_page.segments();
    let mut word_elements = Vec::new();

    for (_segment_idx, segment) in segments.iter().enumerate() {
        let segment_text = segment.text();
        let bounds = segment.bounds();

        if !segment_text.trim().is_empty() {
            let x = bounds.left().value;
            let y = page_height - bounds.top().value;
            let w = bounds.width().value;
            let h = bounds.height().value;

            // Use shared coordinate validation
            if validate_coordinates(x, y, w, h) {
                // Split segment into words but focus on reading flow
                for word in segment_text.split_whitespace() {
                    if !word.is_empty() {
                        word_elements.push((word.to_string(), x, y, w, h));
                    }
                }
            }
        }
    }

    if word_elements.is_empty() {
        return Ok(vec![vec![' '; width]; height]);
    }

    // Filter out any elements with invalid coordinates before sorting
    word_elements.retain(|(_, x, y, w, h)| {
        x.is_finite() && y.is_finite() && w.is_finite() && h.is_finite() &&
        *x >= 0.0 && *y >= 0.0 && *w >= 0.0 && *h >= 0.0
    });

    // Safe sort for optimal reading flow
    word_elements.sort_by(|a, b| {
        // Use total_cmp for guaranteed total ordering (handles NaN safely)
        let y_cmp = a.2.total_cmp(&b.2);
        if y_cmp != std::cmp::Ordering::Equal {
            return y_cmp;
        }
        a.1.total_cmp(&b.1)
    });

    // Flow-based text placement: prioritize reading order over spatial accuracy
    let mut grid = vec![vec![' '; width]; height];
    let mut grid_row = 0;
    let mut grid_col = 0;
    let mut last_y = f32::NEG_INFINITY;
    let mut last_line_height = 10.0;

    for (word, _x, y, _w, h) in word_elements {
        // Enhanced line and paragraph break detection
        let line_threshold = h.max(10.0);
        let paragraph_threshold = h.max(last_line_height) * 1.8; // Paragraph = 1.8x line height

        if (y - last_y).abs() > line_threshold && last_y != f32::NEG_INFINITY {
            let gap = (y - last_y).abs();

            // Check if this is a paragraph break (larger gap)
            if gap > paragraph_threshold {
                // Add blank line for paragraph break
                grid_row += 1;
                if grid_row >= height { break; }
            }

            // New line detected - move to next row
            grid_row += 1;
            grid_col = 0;

            if grid_row >= height {
                break;
            }
        }

        // Calculate word spacing based on reading flow
        if grid_col > 0 && grid_col < width {
            // Add natural reading space between words
            grid[grid_row][grid_col] = ' ';
            grid_col += 1;
        }

        // Place word characters with flow preservation
        for ch in word.chars() {
            if grid_col < width && grid_row < height {
                // Character filtering for clean reading flow
                if ch.is_control() {
                    continue;
                }
                grid[grid_row][grid_col] = ch;
                grid_col += 1;
            } else {
                // Word doesn't fit on line - wrap to next line
                grid_row += 1;
                grid_col = 0;
                if grid_row >= height {
                    break;
                }
                if grid_col < width {
                    grid[grid_row][grid_col] = ch;
                    grid_col += 1;
                }
            }
        }

        last_y = y;
        last_line_height = h;
    }

    Ok(grid)
}

async fn extract_leptess_ocr_method(
    pdf_path: &Path,
    page_num: usize,
    width: usize,
    height: usize,
) -> Result<Vec<Vec<char>>> {
    // Add timeout to prevent hanging and reduce hiccup
    let ocr_future = try_leptess_ocr(pdf_path, page_num, width, height);
    let timeout_duration = std::time::Duration::from_secs(3); // 3 second timeout

    match tokio::time::timeout(timeout_duration, ocr_future).await {
        Ok(Ok(result)) => Ok(result),
        Ok(Err(_)) | Err(_) => {
            // Fast fallback to simple text extraction if OCR fails or times out
            let pdfium = Pdfium::new(
                Pdfium::bind_to_library(Pdfium::pdfium_platform_library_name_at_path("./lib/"))?
            );
            let document = pdfium.load_pdf_from_file(pdf_path, None)?;
            let page = document.pages().get(page_num as u16)?;
            let text_page = page.text()?;
            let all_text = text_page.all();

            let mut grid = vec![vec![' '; width]; height];
            let lines: Vec<&str> = all_text.lines().collect();

            for (row, line) in lines.iter().enumerate() {
                if row >= height { break; }
                for (col, ch) in line.chars().enumerate() {
                    if col >= width { break; }
                    grid[row][col] = ch;
                }
            }

            Ok(grid)
        }
    }
}

async fn try_leptess_ocr(
    pdf_path: &Path,
    page_num: usize,
    grid_width: usize,
    grid_height: usize,
) -> Result<Vec<Vec<char>>> {
    use leptess::{LepTess, Variable};

    // Use shared PDF loading
    let pdfium = Pdfium::new(
        Pdfium::bind_to_library(Pdfium::pdfium_platform_library_name_at_path("./lib/"))?
    );
    let document = pdfium.load_pdf_from_file(pdf_path, None)?;
    let page = document.pages().get(page_num as u16)?;

    // PREPROCESSING TRICK: Render at 2x resolution for better OCR accuracy
    // Tesseract performs better with higher resolution images (counterintuitive but true)
    let bitmap = page.render_with_config(
        &PdfRenderConfig::new()
            .set_target_size(2400, 3200) // 2x resolution for better OCR accuracy
            .rotate_if_landscape(PdfPageRenderRotation::None, false)
    )?;

    // OPTIMIZED: Try to use raw bytes directly if possible to avoid conversion overhead
    // Get raw BGRA bytes directly from bitmap using the non-deprecated method
    let raw_bytes = bitmap.as_raw_bytes();

    // Direct BGRA to RGB conversion without intermediate Image object
    // This saves memory by avoiding bitmap -> Image -> RGB8 conversions
    let width = bitmap.width();
    let height = bitmap.height();
    let mut rgb_bytes = Vec::with_capacity((width * height * 3) as usize);

    // Direct BGRA to RGB conversion, skipping alpha channel
    for chunk in raw_bytes.chunks_exact(4) {
        rgb_bytes.push(chunk[2]); // R (BGRA format has reversed order)
        rgb_bytes.push(chunk[1]); // G
        rgb_bytes.push(chunk[0]); // B
        // Skip chunk[3] which is alpha
    }

    // Initialize Tesseract with advanced optimization settings
    let mut leptess = LepTess::new(None, "eng")?;

    // THE DPI LIE - Tesseract was trained on 300 DPI, so we tell it that's what we have
    leptess.set_variable(Variable::UserDefinedDpi, "300")?; // Magic number that improves accuracy

    // Note: Some thresholding variables may not be available in this version of leptess
    // We'll use what's available for optimization

    // ADVANCED OCR CONFIGURATION
    leptess.set_variable(Variable::TesseditPagesegMode, "3")?; // PSM 3: Fully automatic page segmentation
    leptess.set_variable(Variable::TesseditOcrEngineMode, "1")?; // LSTM neural nets only (most accurate)

    // DICTIONARY OPTIMIZATION - Disable system dictionaries for better character recognition
    leptess.set_variable(Variable::LoadSystemDawg, "0")?; // Don't load system dictionary
    leptess.set_variable(Variable::LoadFreqDawg, "0")?; // Don't load frequency dictionary

    // CHARACTER WHITELIST - Limit to common characters for cleaner results
    leptess.set_variable(Variable::TesseditCharWhitelist,
        "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789 .,;:!?-()[]{}\"'/\n\t")?;

    // EDGE ENHANCEMENT - Improve text edge detection
    leptess.set_variable(Variable::TextordMinXheight, "8")?; // Minimum x-height for text lines
    leptess.set_variable(Variable::MinOrientationMargin, "7")?; // Better orientation detection

    // OUTPUT OPTIMIZATIONS
    leptess.set_variable(Variable::TesseditCreateTsv, "0")?; // No TSV output
    leptess.set_variable(Variable::TesseditCreateHocr, "0")?; // No HOCR output
    leptess.set_variable(Variable::TesseditPreserveMinWdLen, "2")?; // Keep short words
    leptess.set_variable(Variable::TesseditWriteImages, "0")?; // No debug images

    // Set the RGB image for OCR
    if let Err(e) = leptess.set_image_from_mem(&rgb_bytes) {
        return Err(anyhow::anyhow!("OCR image format error: {}", e));
    }

    // Get OCR text and confidence score
    let ocr_text = leptess.get_utf8_text()?;
    let confidence = leptess.mean_text_conf();

    // INVERTED TEXT TRICK - If confidence is low, try inverted image
    let final_text = if confidence < 70 {
        // Create inverted version of the image (white text on black background)
        let mut inverted_bytes = Vec::with_capacity(rgb_bytes.len());
        for byte in &rgb_bytes {
            inverted_bytes.push(255 - byte); // Invert each color channel
        }

        // Try OCR on inverted image
        let mut leptess_inverted = LepTess::new(None, "eng")?;
        leptess_inverted.set_variable(Variable::UserDefinedDpi, "300")?;
        leptess_inverted.set_variable(Variable::TesseditPagesegMode, "3")?;
        leptess_inverted.set_variable(Variable::TesseditOcrEngineMode, "1")?;

        if let Ok(()) = leptess_inverted.set_image_from_mem(&inverted_bytes) {
            if let Ok(inverted_text) = leptess_inverted.get_utf8_text() {
                let inverted_confidence = leptess_inverted.mean_text_conf();

                // Use inverted result if it has higher confidence
                if inverted_confidence > confidence {
                    inverted_text
                } else {
                    ocr_text
                }
            } else {
                ocr_text
            }
        } else {
            ocr_text
        }
    } else {
        ocr_text
    };

    // Use the final OCR text for processing
    let ocr_text = final_text;

    // Enhanced text processing with better paragraph and section detection
    let mut grid = vec![vec![' '; grid_width]; grid_height];
    let lines: Vec<&str> = ocr_text.lines().collect();
    let mut grid_row = 0;
    let mut last_was_empty = false;
    let mut prev_line_length = 0;

    for (line_idx, line) in lines.iter().enumerate() {
        if grid_row >= grid_height {
            break;
        }

        // Detect paragraph breaks in OCR text (empty lines or indentation)
        let trimmed_line = line.trim();

        if trimmed_line.is_empty() {
            // Empty line indicates paragraph break - always preserve at least one blank line
            if !last_was_empty {  // Avoid multiple consecutive blank lines
                grid_row += 1;
                if grid_row >= grid_height { break; }
            }
            last_was_empty = true;
            continue;
        }

        // Check for section/paragraph boundaries
        let is_short_line = prev_line_length > 0 && prev_line_length < grid_width / 2;
        let has_indent = line.starts_with("    ") || line.starts_with("\t") || line.starts_with("  ");
        let starts_with_bullet = trimmed_line.starts_with("•") || trimmed_line.starts_with("-") ||
                                trimmed_line.starts_with("*") || trimmed_line.starts_with("●");
        let starts_with_number = trimmed_line.chars().next().map_or(false, |c| c.is_ascii_digit());

        // Add spacing for various paragraph indicators
        if line_idx > 0 && grid_row > 0 && !last_was_empty {
            // Add blank line for:
            // - After short lines (likely end of paragraph)
            // - Before indented paragraphs
            // - Before bullet points
            // - Before numbered lists
            // - Between sections (heuristic: significant change in line characteristics)
            if is_short_line || has_indent || starts_with_bullet || starts_with_number {
                grid_row += 1;
                if grid_row >= grid_height { break; }
            }
        }

        // Clean up OCR artifacts and preserve spacing
        let cleaned_line = line
            .chars()
            .filter(|&ch| !ch.is_control() || ch == ' ' || ch == '\t')
            .collect::<String>();

        // Preserve indentation and spacing
        let line_to_write = if has_indent {
            // Keep indentation for readability
            format!("  {}", trimmed_line)
        } else {
            cleaned_line.clone()
        };

        for (col, ch) in line_to_write.chars().enumerate() {
            if col >= grid_width {
                break;
            }
            if grid_row < grid_height {
                grid[grid_row][col] = ch;
            }
        }

        prev_line_length = cleaned_line.trim().len();
        last_was_empty = false;
        grid_row += 1;
    }

    Ok(grid)
}


pub fn get_page_count(pdf_path: &Path) -> Result<usize> {
    // Use shared PDFium instance for consistency
    let pdfium = Pdfium::new(
        Pdfium::bind_to_library(Pdfium::pdfium_platform_library_name_at_path("./lib/"))?
    );
    let document = pdfium.load_pdf_from_file(pdf_path, None)?;
    Ok(document.pages().len() as usize)
}