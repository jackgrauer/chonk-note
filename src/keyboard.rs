use crate::{App, DisplayMode, MOD_KEY};
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::io;

/// Handle all keyboard input for the application
pub fn handle_input(app: &mut App, key: KeyEvent, runtime: &tokio::runtime::Runtime) -> Result<bool> {
    match key.code {
        KeyCode::Char('q') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.exit_requested = true;
            return Ok(true);
        }
        
        // Arrow keys never navigate PDF pages - they're only for scrolling/cursor movement
        
        KeyCode::Tab => {
            app.toggle_mode();
            // DON'T reload PDF - this was causing the flicker!
            // The existing image will be displayed in the new mode automatically
        }
        
        KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.dark_mode = !app.dark_mode;
            app.status_message = format!("Mode: {}", if app.dark_mode { "Dark" } else { "Light" });
        }
        
        KeyCode::Char('o') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            // Open new file
            crossterm::terminal::disable_raw_mode()?;
            println!("\r\n🐹 Opening file picker...\r");
            
            let new_file = crate::file_picker::pick_pdf_file()?;
            
            crossterm::terminal::enable_raw_mode()?;
            
            if let Some(new_file) = new_file {
                if let Ok(new_app) = App::new(new_file.clone(), 1, "edit") {
                    *app = new_app;
                    app.status_message = format!("Loaded: {}", new_file.display());
                    runtime.block_on(app.load_pdf_page())?;
                }
            }
        }
        
        KeyCode::Char('e') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            runtime.block_on(app.extract_current_page())?;
        }
        
        // EDIT mode keyboard handlers - only active when in EDIT mode with content
        _ if app.display_mode == DisplayMode::PdfEdit && app.edit_data.is_some() => {
            handle_edit_mode_keys(app, key)?;
        }
        
        // MARKDOWN mode keyboard handlers - only active when in MARKDOWN mode with content
        _ if app.display_mode == DisplayMode::PdfMarkdown && app.markdown_data.is_some() => {
            handle_markdown_mode_keys(app, key)?;
        }
        
        
        // Note: Arrow keys and PageUp/PageDown are handled in EDIT and MARKDOWN mode blocks above
        
        _ => {}
    }
    
    Ok(true)
}

/// Handle EDIT mode specific keyboard input
fn handle_edit_mode_keys(app: &mut App, key: KeyEvent) -> Result<()> {
    match key.code {
        // Copy selection
        KeyCode::Char('c') if key.modifiers.contains(MOD_KEY) => {
            if let Some(text) = extract_selection_text(app) {
                if let Err(e) = copy_to_clipboard(&text) {
                    app.status_message = format!("Copy failed: {}", e);
                } else {
                    app.status_message = "Text copied to clipboard".to_string();
                }
            }
        }
        
        // Paste at cursor
        KeyCode::Char('v') if key.modifiers.contains(MOD_KEY) => {
            match paste_from_clipboard() {
                Ok(text) => {
                    paste_at_cursor(app, &text);
                    app.status_message = "Text pasted".to_string();
                }
                Err(e) => {
                    app.status_message = format!("Paste failed: {}", e);
                }
            }
        }
        
        // Arrow key navigation for moving cursor
        KeyCode::Up => {
            if app.cursor.1 > 0 {
                app.cursor.1 -= 1;
                // Adjust x position if needed
                if let Some(data) = &app.edit_data {
                    if app.cursor.1 < data.len() {
                        let row_len = data[app.cursor.1].len();
                        if app.cursor.0 > row_len {
                            app.cursor.0 = row_len;
                        }
                    }
                }
                // Auto-scroll to follow cursor
                if let Some(renderer) = &mut app.edit_display {
                    let (_, scroll_y) = renderer.get_scroll();
                    if (app.cursor.1 as u16) < scroll_y {
                        renderer.scroll_up(1);
                    }
                }
            }
        }
        KeyCode::Down => {
            if let Some(data) = &app.edit_data {
                if app.cursor.1 < data.len().saturating_sub(1) {
                    app.cursor.1 += 1;
                    // Adjust x position if needed
                    if app.cursor.1 < data.len() {
                        let row_len = data[app.cursor.1].len();
                        if app.cursor.0 > row_len {
                            app.cursor.0 = row_len;
                        }
                    }
                    // Auto-scroll to follow cursor
                    if let Some(renderer) = &mut app.edit_display {
                        let (_, scroll_y) = renderer.get_scroll();
                        let (_, viewport_height) = renderer.get_viewport_size();
                        if (app.cursor.1 as u16) >= scroll_y + viewport_height {
                            renderer.scroll_down(1);
                        }
                    }
                }
            }
        }
        KeyCode::Left => {
            if app.cursor.0 > 0 {
                app.cursor.0 -= 1;
                // Auto-scroll to follow cursor
                if let Some(renderer) = &mut app.edit_display {
                    let (scroll_x, _) = renderer.get_scroll();
                    if (app.cursor.0 as u16) < scroll_x {
                        renderer.scroll_left(1);
                    }
                }
            } else if app.cursor.1 > 0 {
                // Move to end of previous line
                app.cursor.1 -= 1;
                if let Some(data) = &app.edit_data {
                    if app.cursor.1 < data.len() {
                        app.cursor.0 = data[app.cursor.1].len();
                    }
                }
            }
        }
        KeyCode::Right => {
            if let Some(data) = &app.edit_data {
                if app.cursor.1 < data.len() {
                    let row_len = data[app.cursor.1].len();
                    if app.cursor.0 < row_len {
                        app.cursor.0 += 1;
                        // Auto-scroll to follow cursor
                        if let Some(renderer) = &mut app.edit_display {
                            let (scroll_x, _) = renderer.get_scroll();
                            let (viewport_width, _) = renderer.get_viewport_size();
                            if (app.cursor.0 as u16) >= scroll_x + viewport_width {
                                renderer.scroll_right(1);
                            }
                        }
                    } else if app.cursor.1 < data.len() - 1 {
                        // Move to beginning of next line
                        app.cursor.1 += 1;
                        app.cursor.0 = 0;
                    }
                }
            }
        }
        
        // Backspace - delete character before cursor
        KeyCode::Backspace => {
            if let Some(data) = &mut app.edit_data {
                if app.cursor.0 > 0 {
                    // Delete character in current row
                    if app.cursor.1 < data.len() {
                        if app.cursor.0 <= data[app.cursor.1].len() {
                            data[app.cursor.1].remove(app.cursor.0 - 1);
                            app.cursor.0 -= 1;
                            
                            // Update renderer
                            if let Some(renderer) = &mut app.edit_display {
                                renderer.update_buffer(data);
                            }
                        }
                    }
                } else if app.cursor.1 > 0 {
                    // Join current line with previous line
                    let current_line = if app.cursor.1 < data.len() {
                        data.remove(app.cursor.1)
                    } else {
                        vec![]
                    };
                    
                    app.cursor.1 -= 1;
                    app.cursor.0 = data[app.cursor.1].len();
                    data[app.cursor.1].extend(current_line);
                    
                    // Update renderer
                    if let Some(renderer) = &mut app.edit_display {
                        renderer.update_buffer(data);
                    }
                }
            }
        }
        
        // Delete - delete character at cursor
        KeyCode::Delete => {
            if let Some(data) = &mut app.edit_data {
                if app.cursor.1 < data.len() {
                    let row_len = data[app.cursor.1].len();
                    if app.cursor.0 < row_len {
                        // Delete character at cursor position
                        data[app.cursor.1].remove(app.cursor.0);
                        
                        // Update renderer
                        if let Some(renderer) = &mut app.edit_display {
                            renderer.update_buffer(data);
                        }
                    } else if app.cursor.0 == row_len && app.cursor.1 + 1 < data.len() {
                        // Join next line with current line
                        let next_line = data.remove(app.cursor.1 + 1);
                        data[app.cursor.1].extend(next_line);
                        
                        // Update renderer
                        if let Some(renderer) = &mut app.edit_display {
                            renderer.update_buffer(data);
                        }
                    }
                }
            }
        }
        
        // Enter - insert new line
        KeyCode::Enter => {
            if let Some(data) = &mut app.edit_data {
                if app.cursor.1 < data.len() {
                    // Split current line at cursor position
                    let current_row = &mut data[app.cursor.1];
                    let split_point = app.cursor.0.min(current_row.len());
                    let new_line: Vec<char> = current_row.drain(split_point..).collect();
                    
                    // Insert new line after current
                    data.insert(app.cursor.1 + 1, new_line);
                    
                    // Move cursor to beginning of new line
                    app.cursor.1 += 1;
                    app.cursor.0 = 0;
                } else {
                    // Just add a new empty line
                    data.push(vec![]);
                    app.cursor.1 = data.len() - 1;
                    app.cursor.0 = 0;
                }
                
                // Update renderer
                if let Some(renderer) = &mut app.edit_display {
                    renderer.update_buffer(data);
                }
            }
        }
        
        // Type regular characters
        KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) && !key.modifiers.contains(KeyModifiers::ALT) => {
            if let Some(data) = &mut app.edit_data {
                // Ensure we have a row to type into
                while data.len() <= app.cursor.1 {
                    data.push(vec![]);
                }
                
                // Insert character at cursor position
                let row = &mut data[app.cursor.1];
                let insert_pos = app.cursor.0.min(row.len());
                row.insert(insert_pos, c);
                app.cursor.0 += 1;
                
                // Update renderer
                if let Some(renderer) = &mut app.edit_display {
                    renderer.update_buffer(data);
                }
                
                // Clear any selection when typing
                app.selection_start = None;
                app.selection_end = None;
                app.is_selecting = false;
            }
        }
        
        _ => {}
    }
    
    Ok(())
}

/// Handle MARKDOWN mode specific keyboard input
fn handle_markdown_mode_keys(app: &mut App, key: KeyEvent) -> Result<()> {
    match key.code {
        // Arrow key navigation for scrolling
        KeyCode::Up => {
            if let Some(renderer) = &mut app.markdown_renderer {
                renderer.scroll_up(1);
            }
        }
        KeyCode::Down => {
            if let Some(renderer) = &mut app.markdown_renderer {
                renderer.scroll_down(1);
            }
        }
        KeyCode::Left => {
            // Markdown doesn't have horizontal scrolling (word-wrapped)
        }
        KeyCode::Right => {
            // Markdown doesn't have horizontal scrolling (word-wrapped)
        }
        _ => {}
    }
    
    Ok(())
}

/// Extract selected text from the edit buffer
fn extract_selection_text(app: &App) -> Option<String> {
    let (start, end) = match (app.selection_start, app.selection_end) {
        (Some(s), Some(e)) => {
            // Normalize selection (ensure start comes before end)
            if s.1 < e.1 || (s.1 == e.1 && s.0 < e.0) {
                (s, e)
            } else {
                (e, s)
            }
        }
        _ => return None,
    };
    
    if let Some(data) = &app.edit_data {
        let mut text = String::new();
        
        if start.1 == end.1 {
            // Single line selection
            if let Some(row) = data.get(start.1) {
                let start_col = start.0.min(row.len());
                let end_col = end.0.min(row.len());
                for i in start_col..=end_col {
                    if let Some(ch) = row.get(i) {
                        text.push(*ch);
                    }
                }
            }
        } else {
            // Multi-line selection
            for y in start.1..=end.1 {
                if let Some(row) = data.get(y) {
                    let start_col = if y == start.1 { start.0 } else { 0 };
                    let end_col = if y == end.1 { end.0 } else { row.len().saturating_sub(1) };
                    
                    for x in start_col..=end_col.min(row.len().saturating_sub(1)) {
                        if let Some(ch) = row.get(x) {
                            text.push(*ch);
                        }
                    }
                    
                    if y < end.1 {
                        text.push('\n');
                    }
                }
            }
        }
        
        Some(text)
    } else {
        None
    }
}

/// Paste text at cursor position
fn paste_at_cursor(app: &mut App, text: &str) {
    if let Some(data) = &mut app.edit_data {
        // Ensure we have a row to paste into
        while data.len() <= app.cursor.1 {
            data.push(vec![]);
        }
        
        let lines: Vec<&str> = text.lines().collect();
        
        if lines.is_empty() {
            return;
        }
        
        if lines.len() == 1 {
            // Single line paste
            let row = &mut data[app.cursor.1];
            let insert_pos = app.cursor.0.min(row.len());
            
            for (i, ch) in lines[0].chars().enumerate() {
                row.insert(insert_pos + i, ch);
            }
            app.cursor.0 += lines[0].len();
        } else {
            // Multi-line paste
            let current_row = &mut data[app.cursor.1];
            let insert_pos = app.cursor.0.min(current_row.len());
            
            // Split current line at cursor
            let remaining_chars: Vec<char> = current_row.drain(insert_pos..).collect();
            
            // Insert first line
            for ch in lines[0].chars() {
                current_row.push(ch);
            }
            
            // Insert middle lines
            for line in &lines[1..lines.len()-1] {
                let new_line: Vec<char> = line.chars().collect();
                data.insert(app.cursor.1 + 1, new_line);
                app.cursor.1 += 1;
            }
            
            // Insert last line and remaining chars
            if lines.len() > 1 {
                let mut last_line: Vec<char> = lines[lines.len()-1].chars().collect();
                app.cursor.0 = last_line.len();
                last_line.extend(remaining_chars);
                data.insert(app.cursor.1 + 1, last_line);
                app.cursor.1 += 1;
            }
        }
        
        // Update renderer
        if let Some(renderer) = &mut app.edit_display {
            renderer.update_buffer(data);
        }
    }
}

/// Copy text to clipboard
fn copy_to_clipboard(text: &str) -> Result<()> {
    use cli_clipboard::{ClipboardContext, ClipboardProvider};
    
    let mut ctx: ClipboardContext = ClipboardProvider::new()
        .map_err(|e| anyhow::anyhow!("Failed to create clipboard context: {}", e))?;
    
    ctx.set_contents(text.to_owned())
        .map_err(|e| anyhow::anyhow!("Failed to set clipboard contents: {}", e))?;
    
    Ok(())
}

/// Paste text from clipboard
fn paste_from_clipboard() -> Result<String> {
    use cli_clipboard::{ClipboardContext, ClipboardProvider};
    
    let mut ctx: ClipboardContext = ClipboardProvider::new()
        .map_err(|e| anyhow::anyhow!("Failed to create clipboard context: {}", e))?;
    
    ctx.get_contents()
        .map_err(|e| anyhow::anyhow!("Failed to get clipboard contents: {}", e))
}