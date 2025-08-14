use anyhow::Result;
use crossterm::{
    cursor::{self, MoveTo},
    event::{self, Event, KeyCode, KeyEvent},
    execute,
    style::{Color, Print, ResetColor, SetForegroundColor},
    terminal::{self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
};
use nucleo::{Config, Nucleo, Utf32String};
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;

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
    
    let result = run_fuzzy_picker(&pdf_files);
    
    // Cleanup
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
    
    // Query string
    let mut query = String::new();
    let mut selected_index = 0usize;
    
    loop {
        // Clear screen
        execute!(stdout, Clear(ClearType::All), MoveTo(0, 0))?;
        
        // Draw header with Ghostty-inspired style
        execute!(
            stdout,
            SetForegroundColor(Color::Rgb { r: 129, g: 162, b: 190 }), // Soft blue
            Print("  CHONKER7 FILE PICKER"),
            ResetColor,
            Print("\n"),
            SetForegroundColor(Color::Rgb { r: 96, g: 99, b: 102 }), // Dim text
            Print("  Select PDF from ~/Documents"),
            ResetColor,
            Print("\n\n")
        )?;
        
        // Draw search box
        execute!(
            stdout,
            SetForegroundColor(Color::Rgb { r: 143, g: 161, b: 179 }), // Muted cyan
            Print("  Search: "),
            SetForegroundColor(Color::Rgb { r: 197, g: 200, b: 198 }), // Primary text
            Print(&query),
            SetForegroundColor(Color::Rgb { r: 96, g: 99, b: 102 }), // Dim
            Print("_"),
            ResetColor,
            Print("\n\n")
        )?;
        
        // Get filtered results
        let snapshot = nucleo.snapshot();
        let matches = snapshot.matched_items(..)
            .take(20)  // Show top 20 matches
            .collect::<Vec<_>>();
        
        // Get terminal width for truncation
        let (term_width, _) = terminal::size().unwrap_or((80, 24));
        let max_path_width = (term_width as usize).saturating_sub(5); // Leave room for "> " prefix and margin
        
        // Draw matches
        for (i, item) in matches.iter().enumerate() {
            let path = item.data.as_ref();
            
            // Strip the /Users/jack/Documents/ prefix for cleaner display
            let clean_path = if path.starts_with("/Users/jack/Documents/") {
                &path[22..] // Length of "/Users/jack/Documents/"
            } else {
                path
            };
            
            // Calculate current line position (header: 3 lines, search: 2 lines, then matches)
            let line_pos = 5 + i as u16;
            
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
            
            if i == selected_index {
                execute!(
                    stdout,
                    SetForegroundColor(Color::Rgb { r: 181, g: 189, b: 104 }), // Success green
                    Print("  ▶ "),
                    SetForegroundColor(Color::Rgb { r: 197, g: 200, b: 198 }), // Primary text
                    Print(&final_display),
                    ResetColor
                )?;
            } else {
                execute!(
                    stdout,
                    Print("    "),
                    SetForegroundColor(Color::Rgb { r: 150, g: 152, b: 150 }), // Secondary text
                    Print(&final_display),
                    ResetColor
                )?;
            }
        }
        
        // Clear any remaining lines from previous render (if list got shorter)
        for i in matches.len()..20 {
            let line_pos = 5 + i as u16;
            execute!(
                stdout,
                MoveTo(0, line_pos),
                Clear(ClearType::CurrentLine)
            )?;
        }
        
        // Draw help at a fixed position
        let help_line = 26; // Fixed position for help text
        execute!(
            stdout,
            MoveTo(0, help_line),
            Clear(ClearType::CurrentLine),
            SetForegroundColor(Color::Rgb { r: 96, g: 99, b: 102 }), // Dim text
            Print("  ↑/↓ Navigate  •  Enter Select  •  Esc Cancel  •  Type to search"),
            ResetColor
        )?;
        
        stdout.flush()?;
        
        // Handle input
        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Esc => {
                        return Ok(None);
                    }
                    KeyCode::Enter => {
                        if !matches.is_empty() && selected_index < matches.len() {
                            let selected = matches[selected_index].data.as_ref();
                            return Ok(Some(PathBuf::from(selected)));
                        }
                    }
                    KeyCode::Up => {
                        if selected_index > 0 {
                            selected_index -= 1;
                        }
                    }
                    KeyCode::Down => {
                        if selected_index < matches.len().saturating_sub(1) {
                            selected_index += 1;
                        }
                    }
                    KeyCode::Backspace => {
                        query.pop();
                        selected_index = 0;
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