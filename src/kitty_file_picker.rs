// KITTY-NATIVE FILE PICKER
// Simple directory listing with arrow key navigation, no nucleo
use std::path::PathBuf;
use std::fs;
use anyhow::Result;
use crate::kitty_native::{KittyTerminal, KeyCode, KeyModifiers};
use std::io::Write;

pub enum FilePickerAction {
    Open(PathBuf),
    Cancel,
}

pub fn pick_pdf_file() -> Result<Option<PathBuf>> {
    match pick_pdf_with_action()? {
        FilePickerAction::Open(path) => Ok(Some(path)),
        _ => Ok(None),
    }
}

pub fn pick_pdf_with_action() -> Result<FilePickerAction> {
    // Find all PDF files in Documents
    let docs_dir = PathBuf::from("/Users/jack/Documents");
    let pdf_files = find_pdf_files(&docs_dir)?;

    if pdf_files.is_empty() {
        return Ok(FilePickerAction::Cancel);
    }

    // Always do proper terminal setup for file picker
    KittyTerminal::enable_raw_mode()?;
    KittyTerminal::enter_fullscreen()?;

    let result = run_simple_picker(&pdf_files);

    // Clean exit - restore terminal to normal mode
    // The main app will set it up again for its own use
    KittyTerminal::exit_fullscreen()?;
    KittyTerminal::disable_raw_mode()?;

    // Clear any remaining graphics
    print!("\x1b[2J\x1b[H");  // Clear screen and move to top
    std::io::stdout().flush()?;

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

fn search_pdf_filename(pdf_path: &PathBuf, query: &str) -> bool {
    // Quick exit for empty query
    if query.trim().is_empty() {
        return true; // Show all files if no query
    }

    // Search filename (much faster than content!)
    let filename = pdf_path.file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_lowercase();

    let query_lower = query.to_lowercase();
    filename.contains(&query_lower)
}

fn run_simple_picker(files: &[PathBuf]) -> Result<FilePickerAction> {
    let mut selected_index = 0;
    let mut scroll_offset = 0;
    let mut needs_redraw = true; // Nuclear anti-flicker state tracking
    let mut last_render_time = std::time::Instant::now();
    let mut search_query = String::new();
    let mut search_mode = false;

    loop {
        // Filter files based on filename search (instant!)
        let filtered_files: Vec<&PathBuf> = if search_mode {
            files.iter().filter(|path| {
                search_pdf_filename(path, &search_query)
            }).collect()
        } else {
            files.iter().collect()
        };
        // NUCLEAR ANTI-FLICKER: Only redraw when state actually changes
        let now = std::time::Instant::now();
        let frame_time = now.duration_since(last_render_time);

        if needs_redraw && frame_time.as_millis() >= 33 { // Max 30 FPS
            // Never clear screen - just position and overwrite
            print!("\x1b[H");  // Move to top-left, no clear
            last_render_time = now;
            needs_redraw = false;

        // Header with search mode indicator
        if search_mode {
            print!("\x1b[48;2;169;133;202m\x1b[38;2;255;255;255m🔍 Chonker7 - Search Filenames\x1b[m\r\n");
            print!("Search: {}_\r\n", search_query);
        } else {
            print!("\x1b[48;2;169;133;202m\x1b[38;2;255;255;255m🐹 Chonker7 - Select PDF File\x1b[m\r\n");
            print!("Press '/' to search filenames\r\n");
        }
        print!("\r\n");

        // Get terminal size for display
        let (_, term_height) = KittyTerminal::size().unwrap_or((80, 24));
        let max_display_items = (term_height as usize).saturating_sub(5).min(20);

        // Update scroll offset to keep selected item visible
        if selected_index >= scroll_offset + max_display_items {
            scroll_offset = selected_index.saturating_sub(max_display_items - 1);
        } else if selected_index < scroll_offset {
            scroll_offset = selected_index;
        }


        // Update scroll offset for filtered results
        if selected_index >= scroll_offset + max_display_items {
            scroll_offset = selected_index.saturating_sub(max_display_items - 1);
        } else if selected_index < scroll_offset {
            scroll_offset = selected_index;
        }

        // Display files with simple loop to avoid iterator issues
        for i in 0..max_display_items {
            let file_index = scroll_offset + i;
            if file_index < filtered_files.len() {
                let file = filtered_files[file_index];
                let filename = file.file_name()
                    .unwrap_or_default()
                    .to_string_lossy();

                if file_index == selected_index {
                    // Selected file with explicit line control
                    print!("\x1b[38;2;181;189;104m  ▶ {}\x1b[m\r\n", filename);
                } else {
                    // Unselected file with explicit line control
                    print!("\x1b[38;2;96;99;102m    {}\x1b[m\r\n", filename);
                }
            } else {
                // Clear empty lines from previous renders
                print!("                                                                \r\n");
            }
        }

        // Footer with search instructions
        print!("\r\n");
        if search_mode {
            print!("\x1b[38;2;96;99;102m  ↑/↓ Navigate  •  Enter Open  •  Esc Exit Search\x1b[m");
        } else {
            print!("\x1b[38;2;96;99;102m  ↑/↓ Navigate  •  Enter Open  •  Esc Cancel  •  / Search\x1b[m");
        }

        std::io::stdout().flush()?;

        } // End nuclear anti-flicker render block

        // Handle input
        if let Some(key) = KittyTerminal::read_key()? {
            match key.code {
                KeyCode::Up => {
                    // No need to check max_index for Up arrow
                    if selected_index > 0 {
                        selected_index -= 1;
                        needs_redraw = true;
                    }
                }
                KeyCode::Down => {
                    let max_index = if search_mode && !search_query.is_empty() {
                        filtered_files.len().saturating_sub(1)
                    } else {
                        files.len().saturating_sub(1)
                    };
                    if selected_index < max_index {
                        selected_index += 1;
                        needs_redraw = true;
                    }
                }
                KeyCode::Enter => {
                    let target_files = if search_mode && !search_query.is_empty() {
                        &filtered_files
                    } else {
                        &files.iter().collect::<Vec<_>>()
                    };
                    if selected_index < target_files.len() {
                        return Ok(FilePickerAction::Open(target_files[selected_index].clone()));
                    }
                }
                KeyCode::Esc => {
                    if search_mode {
                        search_mode = false;
                        search_query.clear();
                        selected_index = 0;
                        needs_redraw = true;
                    } else {
                        return Ok(FilePickerAction::Cancel);
                    }
                }
                KeyCode::Char('/') => {
                    search_mode = true;
                    search_query.clear();
                    selected_index = 0;
                    scroll_offset = 0;
                    needs_redraw = true;
                }
                KeyCode::Backspace if search_mode => {
                    search_query.pop();
                    selected_index = 0;
                    scroll_offset = 0;
                    needs_redraw = true;
                }
                KeyCode::Char(c) if search_mode && !key.modifiers.contains(KeyModifiers::CONTROL) => {
                    search_query.push(c);
                    selected_index = 0;
                    scroll_offset = 0;
                    needs_redraw = true;
                }
                KeyCode::Char('q') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    return Ok(FilePickerAction::Cancel);
                }
                _ => {}
            }
        }
    }
}