// KITTY-NATIVE FILE PICKER
// Simple directory listing with arrow key navigation, no nucleo
use std::path::PathBuf;
use std::fs;
use anyhow::Result;
use crate::kitty_native::{KittyTerminal, KeyCode, KeyModifiers};
use std::io::Write;

pub fn pick_pdf_file() -> Result<Option<PathBuf>> {
    // Find all PDF files in Documents
    let docs_dir = PathBuf::from("/Users/jack/Documents");
    let pdf_files = find_pdf_files(&docs_dir)?;

    if pdf_files.is_empty() {
        return Ok(None);
    }

    // Always do proper terminal setup for file picker
    KittyTerminal::enable_raw_mode()?;
    KittyTerminal::enter_fullscreen()?;

    let result = run_simple_picker(&pdf_files);

    // Clean exit that works whether returning to app or shell
    KittyTerminal::exit_fullscreen()?;
    KittyTerminal::disable_raw_mode()?;

    result
}

fn find_pdf_files(dir: &PathBuf) -> Result<Vec<PathBuf>> {
    let mut pdf_files = Vec::new();

    fn scan_directory(dir: &PathBuf, files: &mut Vec<PathBuf>) -> Result<()> {
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    if let Some(ext) = path.extension() {
                        if ext.to_string_lossy().to_lowercase() == "pdf" {
                            files.push(path);
                        }
                    }
                } else if path.is_dir() && !path.file_name().unwrap_or_default().to_string_lossy().starts_with('.') {
                    // Recursively scan subdirectories (but not hidden dirs)
                    let _ = scan_directory(&path, files);
                }
            }
        }
        Ok(())
    }

    scan_directory(dir, &mut pdf_files)?;

    // Sort by filename
    pdf_files.sort_by(|a, b| {
        a.file_name().cmp(&b.file_name())
    });

    Ok(pdf_files)
}

fn run_simple_picker(files: &[PathBuf]) -> Result<Option<PathBuf>> {
    let mut selected_index = 0;
    let mut scroll_offset = 0;

    loop {
        // Clear screen and draw header
        KittyTerminal::clear_screen()?;
        KittyTerminal::move_to(0, 0)?;

        // Header
        KittyTerminal::set_bg_rgb(169, 133, 202)?; // Purple background
        KittyTerminal::set_fg_rgb(255, 255, 255)?; // White text
        print!("🐹 Chonker7 - Select PDF File");
        KittyTerminal::reset_colors()?;
        println!();
        println!();

        // Get terminal size for display
        let (term_width, term_height) = KittyTerminal::size().unwrap_or((80, 24));
        let max_display_items = (term_height as usize).saturating_sub(5).min(20);

        // Update scroll offset to keep selected item visible
        if selected_index >= scroll_offset + max_display_items {
            scroll_offset = selected_index.saturating_sub(max_display_items - 1);
        } else if selected_index < scroll_offset {
            scroll_offset = selected_index;
        }

        // Display files
        let visible_files = files.iter()
            .skip(scroll_offset)
            .take(max_display_items);

        for (i, file) in visible_files.enumerate() {
            let actual_index = scroll_offset + i;
            let filename = file.file_name()
                .unwrap_or_default()
                .to_string_lossy();

            if actual_index == selected_index {
                // Selected file
                KittyTerminal::set_fg_rgb(181, 189, 104)?; // Green
                print!("  ▶ {}", filename);
                KittyTerminal::reset_colors()?;
            } else {
                // Unselected file
                KittyTerminal::set_fg_rgb(150, 152, 150)?; // Dim
                print!("    {}", filename);
                KittyTerminal::reset_colors()?;
            }
            println!();
        }

        // Footer
        println!();
        KittyTerminal::set_fg_rgb(96, 99, 102)?; // Dim
        print!("  ↑/↓ Navigate  •  Enter Select  •  Esc Cancel");
        KittyTerminal::reset_colors()?;

        std::io::stdout().flush()?;

        // Handle input
        if let Some(key) = KittyTerminal::read_key()? {
            match key.code {
                KeyCode::Up => {
                    if selected_index > 0 {
                        selected_index -= 1;
                    }
                }
                KeyCode::Down => {
                    if selected_index < files.len().saturating_sub(1) {
                        selected_index += 1;
                    }
                }
                KeyCode::Enter => {
                    return Ok(Some(files[selected_index].clone()));
                }
                KeyCode::Esc => {
                    return Ok(None);
                }
                KeyCode::Char('q') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    return Ok(None);
                }
                _ => {}
            }
        }
    }
}