use anyhow::Result;
use crossterm::{
    cursor::MoveTo,
    event::{self, Event, KeyCode, MouseButton, MouseEventKind},
    execute,
    style::{Print, ResetColor, SetForegroundColor, SetBackgroundColor, Attribute, SetAttribute},
    terminal::{self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
};
use nucleo::{Config, Nucleo, Utf32String};
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;

use crate::theme::ChonkerTheme;

/// Use nucleo to pick a PDF file with interactive fuzzy finding
pub fn pick_pdf_file() -> Result<Option<PathBuf>> {
    // First, find all PDF files
    let pdf_files = find_pdf_files()?;
    
    if pdf_files.is_empty() {
        println!("No PDF files found in /Users/jack/Documents");
        return Ok(None);
    }
    
    // Create a simple terminal UI for file selection
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    terminal::enable_raw_mode()?;
    
    // Enable mouse events for trackpad scrolling
    execute!(stdout, crossterm::event::EnableMouseCapture)?;
    
    let result = run_fuzzy_picker(&pdf_files);
    
    // Cleanup
    execute!(stdout, crossterm::event::DisableMouseCapture)?;
    terminal::disable_raw_mode()?;
    execute!(stdout, LeaveAlternateScreen)?;
    
    result
}

/// Run the interactive fuzzy picker
fn run_fuzzy_picker(files: &[String]) -> Result<Option<PathBuf>> {
    let mut stdout = io::stdout();
    
    // Initialize nucleo
    let mut nucleo = Nucleo::<Arc<str>>::new(
        Config::DEFAULT,
        Arc::new(|| {}),
        None,
        1,
    );
    
    // Add all files as items
    let injector = nucleo.injector();
    for file in files {
        let file_arc: Arc<str> = Arc::from(file.as_str());
        let _ = injector.push(file_arc.clone(), |data, cols: &mut [Utf32String]| {
            cols[0] = data.as_ref().into();
        });
    }
    
    // Query string and selection state
    let mut query = String::new();
    let mut selected_index = 0usize;
    let mut scroll_offset = 0usize;
    
    loop {
        // Clear screen
        execute!(stdout, Clear(ClearType::All), MoveTo(0, 0))?;
        
        // Draw uniform header matching other tabs
        let (term_width, _) = terminal::size().unwrap_or((80, 24));
        
        // First header line with tab styling and version
        let header_text = format!("🐹 Chonker {}", env!("CARGO_PKG_VERSION"));
        execute!(
            stdout,
            MoveTo(0, 0),
            SetBackgroundColor(ChonkerTheme::accent_load_file()),
            SetForegroundColor(ChonkerTheme::text_header()),
            SetAttribute(Attribute::Bold),
            Print(format!("  {:<width$}", header_text, width = (term_width - 2) as usize)),
            ResetColor,
            SetAttribute(Attribute::Reset)
        )?;
        
        // Second header line to match other tabs
        execute!(
            stdout,
            MoveTo(0, 1),
            SetBackgroundColor(ChonkerTheme::accent_load_file()),
            Print(" ".repeat(term_width as usize)),
            ResetColor,
            Print("\n")
        )?;
        
        // Draw search box with themed colors
        execute!(
            stdout,
            MoveTo(0, 3),
            SetForegroundColor(ChonkerTheme::accent_text()),
            Print("  Search: "),
            SetForegroundColor(ChonkerTheme::text_primary()),
            Print(&query),
            SetForegroundColor(ChonkerTheme::text_dim()),
            Print("_"),
            ResetColor,
            Print("\n\n")
        )?;
        
        // Get filtered results
        let snapshot = nucleo.snapshot();
        let all_matches = snapshot.matched_items(..).collect::<Vec<_>>();
        
        // Calculate display parameters
        let (term_width, term_height) = terminal::size().unwrap_or((80, 24));
        let max_path_width = (term_width as usize).saturating_sub(5);
        let max_display_items = (term_height as usize).saturating_sub(9).min(15); // Reserve space for header/footer (now 9 lines)
        
        // Update scroll offset to keep selected item visible
        if selected_index >= scroll_offset + max_display_items {
            scroll_offset = selected_index.saturating_sub(max_display_items - 1);
        } else if selected_index < scroll_offset {
            scroll_offset = selected_index;
        }
        
        // Get visible matches with scrolling
        let visible_matches = all_matches
            .iter()
            .skip(scroll_offset)
            .take(max_display_items)
            .collect::<Vec<_>>();
        
        // Draw matches
        for (display_i, item) in visible_matches.iter().enumerate() {
            let actual_index = scroll_offset + display_i;
            let path = item.data.as_ref();
            
            // Strip the /Users/jack/Documents/ prefix for cleaner display
            let clean_path = if path.starts_with("/Users/jack/Documents/") {
                &path[22..] // Length of "/Users/jack/Documents/"
            } else {
                path
            };
            
            // Calculate current line position (header: 2 lines, spacing: 1 line, search: 2 lines, then matches)
            let line_pos = 6 + display_i as u16;
            
            // Move to the correct line and clear it
            execute!(
                stdout,
                MoveTo(0, line_pos),
                Clear(ClearType::CurrentLine)
            )?;
            
            // Force truncate to terminal width - be very strict
            let display_str = if clean_path.len() > max_path_width {
                // Try to show just the filename if it fits
                if let Some(filename) = clean_path.split('/').last() {
                    if filename.len() <= max_path_width - 4 {
                        format!(".../{}", filename)
                    } else {
                        // Just truncate the filename simply
                        let truncate_len = max_path_width.saturating_sub(3).min(filename.len());
                        format!("{}...", &filename[..truncate_len])
                    }
                } else {
                    let truncate_len = max_path_width.saturating_sub(3).min(clean_path.len());
                    format!("{}...", &clean_path[..truncate_len])
                }
            } else {
                clean_path.to_string()
            };
            
            // Final safety check - hard limit to prevent any wrapping
            let final_display: String = display_str.chars().take(max_path_width).collect();
            
            if actual_index == selected_index {
                execute!(
                    stdout,
                    SetForegroundColor(ChonkerTheme::success()),
                    Print("  ▶ "),
                    SetForegroundColor(ChonkerTheme::text_primary()),
                    Print(&final_display),
                    ResetColor
                )?;
            } else {
                execute!(
                    stdout,
                    Print("    "),
                    SetForegroundColor(ChonkerTheme::text_secondary()),
                    Print(&final_display),
                    ResetColor
                )?;
            }
        }
        
        // Clear any remaining lines from previous render (if list got shorter)
        for i in visible_matches.len()..max_display_items {
            let line_pos = 6 + i as u16;
            execute!(
                stdout,
                MoveTo(0, line_pos),
                Clear(ClearType::CurrentLine)
            )?;
        }
        
        // Draw scroll indicator and help
        let help_line = (6 + max_display_items + 2) as u16;
        let scroll_indicator = if all_matches.len() > max_display_items {
            format!("  Showing {}-{} of {} files", 
                scroll_offset + 1, 
                (scroll_offset + visible_matches.len()).min(all_matches.len()), 
                all_matches.len())
        } else {
            format!("  {} files", all_matches.len())
        };
        
        execute!(
            stdout,
            MoveTo(0, help_line),
            Clear(ClearType::CurrentLine),
            SetForegroundColor(ChonkerTheme::text_dim()),
            Print(&scroll_indicator),
            ResetColor
        )?;
        
        execute!(
            stdout,
            MoveTo(0, help_line + 1),
            Clear(ClearType::CurrentLine),
            SetForegroundColor(ChonkerTheme::text_dim()),
            Print("  ↑/↓ Navigate  •  Scroll/Trackpad  •  Enter Select  •  Esc Cancel  •  Type to search"),
            ResetColor
        )?;
        
        stdout.flush()?;
        
        // Handle input
        if event::poll(std::time::Duration::from_millis(100))? {
            match event::read()? {
                Event::Key(key) => {
                    // Handle Ctrl commands first
                    if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) {
                        match key.code {
                            KeyCode::Char('q') | KeyCode::Char('Q') => {
                                // Ctrl+Q - quit the entire application
                                return Ok(None);
                            }
                            KeyCode::Char('c') | KeyCode::Char('C') => {
                                // Ctrl+C - also quit
                                return Ok(None);
                            }
                            _ => {}
                        }
                    } else {
                        // Handle regular key input
                        match key.code {
                            KeyCode::Esc => {
                                return Ok(None);
                            }
                            KeyCode::Enter => {
                                if !all_matches.is_empty() && selected_index < all_matches.len() {
                                    let selected = all_matches[selected_index].data.as_ref();
                                    return Ok(Some(PathBuf::from(selected)));
                                }
                            }
                            KeyCode::Up => {
                                if selected_index > 0 {
                                    selected_index -= 1;
                                }
                            }
                            KeyCode::Down => {
                                if selected_index < all_matches.len().saturating_sub(1) {
                                    selected_index += 1;
                                }
                            }
                            KeyCode::PageUp => {
                                selected_index = selected_index.saturating_sub(max_display_items);
                            }
                            KeyCode::PageDown => {
                                selected_index = (selected_index + max_display_items).min(all_matches.len().saturating_sub(1));
                            }
                            KeyCode::Home => {
                                selected_index = 0;
                            }
                            KeyCode::End => {
                                selected_index = all_matches.len().saturating_sub(1);
                            }
                            KeyCode::Backspace => {
                                query.pop();
                                selected_index = 0;
                                scroll_offset = 0;
                                // Update nucleo pattern
                                nucleo.pattern.reparse(
                                    0,
                                    &query,
                                    nucleo::pattern::CaseMatching::Smart,
                                    nucleo::pattern::Normalization::Smart,
                                    false
                                );
                            }
                            KeyCode::Char(c) => {
                                query.push(c);
                                selected_index = 0;
                                scroll_offset = 0;
                                // Update nucleo pattern
                                nucleo.pattern.reparse(
                                    0,
                                    &query,
                                    nucleo::pattern::CaseMatching::Smart,
                                    nucleo::pattern::Normalization::Smart,
                                    false
                                );
                            }
                            _ => {}
                        }
                    }
                }
                Event::Mouse(mouse) => {
                    match mouse.kind {
                        MouseEventKind::ScrollUp => {
                            // Scroll up - move selection up
                            if selected_index > 0 {
                                selected_index = selected_index.saturating_sub(3); // Scroll 3 items at a time
                            }
                        }
                        MouseEventKind::ScrollDown => {
                            // Scroll down - move selection down
                            selected_index = (selected_index + 3).min(all_matches.len().saturating_sub(1));
                        }
                        MouseEventKind::Down(MouseButton::Left) => {
                            // Click to select item
                            let click_y = mouse.row;
                            if click_y >= 6 && click_y < (6 + max_display_items as u16) {
                                let clicked_index = scroll_offset + (click_y - 6) as usize;
                                if clicked_index < all_matches.len() {
                                    selected_index = clicked_index;
                                }
                            }
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }
        
        // Let nucleo process
        nucleo.tick(10);
    }
}

/// Find all PDF files in current directory and subdirectories
fn find_pdf_files() -> Result<Vec<String>> {
    // Default to /Users/jack/Documents
    let search_dir = "/Users/jack/Documents";
    
    // Try using fd first (faster), fallback to find
    let output = if command_exists("fd") {
        Command::new("fd")
            .args(&["-e", "pdf", "-t", "f", ".", search_dir])
            .output()?
    } else {
        // Fallback to find command
        Command::new("find")
            .args(&[search_dir, "-name", "*.pdf", "-type", "f"])
            .output()?
    };
    
    if !output.status.success() {
        return Ok(Vec::new());
    }
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    let files: Vec<String> = stdout
        .lines()
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty())
        .collect();
    
    Ok(files)
}

/// Check if a command exists
fn command_exists(cmd: &str) -> bool {
    Command::new("which")
        .arg(cmd)
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}