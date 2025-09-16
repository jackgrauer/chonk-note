// MINIMAL KEYBOARD HANDLING
use crate::{App, MOD_KEY};
use anyhow::Result;
use crate::kitty_native::{KeyCode, KeyEvent, KeyModifiers};

pub async fn handle_input(app: &mut App, key: KeyEvent) -> Result<bool> {

    // Cmd+C - Copy
    if key.code == KeyCode::Char('c') && key.modifiers.contains(MOD_KEY) {
        if let Some(text) = extract_selection_text(app) {
            copy_to_clipboard(&text)?;
            app.status_message = "Copied".to_string();
        }
        return Ok(true);
    }
    
    // Cmd+V - Paste
    if key.code == KeyCode::Char('v') && key.modifiers.contains(MOD_KEY) {
        if let Ok(text) = paste_from_clipboard() {
            paste_at_cursor(app, &text);
            app.status_message = "Pasted".to_string();
        }
        return Ok(true);
    }
    
    // Cmd+A - Select All
    if key.code == KeyCode::Char('a') && key.modifiers.contains(MOD_KEY) {
        if let Some(data) = &app.edit_data {
            app.selection_start = Some((0, 0));
            let last_y = data.len().saturating_sub(1);
            let last_x = data[last_y].len().saturating_sub(1);
            app.selection_end = Some((last_x, last_y));
        }
        return Ok(true);
    }
    
    match key.code {
        KeyCode::Char('q') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.exit_requested = true;
        }
        
        KeyCode::Char('o') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.open_file_picker = true;
        }
        
        
        KeyCode::Char('n') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.next_page();
            if app.current_page_image.is_none() {
                app.load_pdf_page().await?;
            }
        }
        KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.prev_page();
            if app.current_page_image.is_none() {
                app.load_pdf_page().await?;
            }
        }

        KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.refresh_extraction().await?;
        }

        KeyCode::Char('m') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.toggle_extraction_method().await?;
        }

        // Alternative: Use 't' for toggle (more reliable)
        KeyCode::Char('t') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.toggle_extraction_method().await?;
        }
        
        // Arrow keys for cursor movement (with shift for selection)
        KeyCode::Up => {
            let speed = app.update_key_repeat(KeyCode::Up);
            if let Some(data) = &app.edit_data {
                if app.cursor.1 > 0 {
                    let move_amount = speed.min(app.cursor.1);
                    let new_y = app.cursor.1 - move_amount;

                    // Clamp cursor X to new line length
                    let new_line_len = if new_y < data.len() { data[new_y].len() } else { 0 };
                    let new_x = app.cursor.0.min(new_line_len);

                    if key.modifiers.contains(KeyModifiers::SHIFT) {
                        if app.selection_start.is_none() {
                            app.selection_start = Some(app.cursor);
                        }
                        app.cursor = (new_x, new_y);
                        app.selection_end = Some(app.cursor);
                    } else {
                        app.cursor = (new_x, new_y);
                        app.selection_start = None;
                        app.selection_end = None;
                    }
                    // Follow cursor with viewport
                    if let Some(renderer) = &mut app.edit_display {
                        renderer.follow_cursor(app.cursor.0, app.cursor.1, 3);
                    }
                }
            }
        }
        KeyCode::Down => {
            let speed = app.update_key_repeat(KeyCode::Down);
            if let Some(data) = &app.edit_data {
                if app.cursor.1 < data.len() - 1 {
                    let max_move = (data.len() - 1) - app.cursor.1;
                    let move_amount = speed.min(max_move);
                    let new_y = app.cursor.1 + move_amount;

                    // Clamp cursor X to new line length
                    let new_line_len = if new_y < data.len() { data[new_y].len() } else { 0 };
                    let new_x = app.cursor.0.min(new_line_len);

                    if key.modifiers.contains(KeyModifiers::SHIFT) {
                        if app.selection_start.is_none() {
                            app.selection_start = Some(app.cursor);
                        }
                        app.cursor = (new_x, new_y);
                        app.selection_end = Some(app.cursor);
                    } else {
                        app.cursor = (new_x, new_y);
                        app.selection_start = None;
                        app.selection_end = None;
                    }
                    // Follow cursor with viewport
                    if let Some(renderer) = &mut app.edit_display {
                        renderer.follow_cursor(app.cursor.0, app.cursor.1, 3);
                    }
                }
            }
        }
        KeyCode::Left => {
            let speed = app.update_key_repeat(KeyCode::Left);
            if app.cursor.0 > 0 {
                let move_amount = speed.min(app.cursor.0);
                if key.modifiers.contains(KeyModifiers::SHIFT) {
                    if app.selection_start.is_none() {
                        app.selection_start = Some(app.cursor);
                    }
                    app.cursor.0 -= move_amount;
                    app.selection_end = Some(app.cursor);
                } else {
                    app.cursor.0 -= move_amount;
                    app.selection_start = None;
                    app.selection_end = None;
                }
                // Follow cursor with viewport
                if let Some(renderer) = &mut app.edit_display {
                    renderer.follow_cursor(app.cursor.0, app.cursor.1, 3);
                }
            }
        }
        KeyCode::Right => {
            let speed = app.update_key_repeat(KeyCode::Right);
            if let Some(data) = &app.edit_data {
                if app.cursor.1 < data.len() && app.cursor.0 < data[app.cursor.1].len() {
                    let max_move = data[app.cursor.1].len() - app.cursor.0;
                    let move_amount = speed.min(max_move);
                    if key.modifiers.contains(KeyModifiers::SHIFT) {
                        if app.selection_start.is_none() {
                            app.selection_start = Some(app.cursor);
                        }
                        app.cursor.0 += move_amount;
                        app.selection_end = Some(app.cursor);
                    } else {
                        app.cursor.0 += move_amount;
                        app.selection_start = None;
                        app.selection_end = None;
                    }
                    // Follow cursor with viewport
                    if let Some(renderer) = &mut app.edit_display {
                        renderer.follow_cursor(app.cursor.0, app.cursor.1, 3);
                    }
                }
            }
        }
        
        // Text editing
        KeyCode::Backspace => {
            if let Some(data) = &mut app.edit_data {
                if app.cursor.0 > 0 && app.cursor.1 < data.len() {
                    data[app.cursor.1].remove(app.cursor.0 - 1);
                    app.cursor.0 -= 1;
                    if let Some(renderer) = &mut app.edit_display {
                        renderer.update_buffer(data);
                        renderer.follow_cursor(app.cursor.0, app.cursor.1, 3);
                    }
                }
            }
        }
        
        KeyCode::Enter => {
            // Enter creates new line in text editor
            if let Some(data) = &mut app.edit_data {
                while data.len() <= app.cursor.1 {
                    data.push(vec![]);
                }

                // Split current line at cursor
                let current_row = &mut data[app.cursor.1];
                let remaining: Vec<char> = current_row.drain(app.cursor.0..).collect();

                // Insert new line with remaining characters
                data.insert(app.cursor.1 + 1, remaining);

                // Move cursor to start of new line
                app.cursor.1 += 1;
                app.cursor.0 = 0;

                // Ensure cursor is within valid bounds
                if app.cursor.1 >= data.len() {
                    app.cursor.1 = data.len().saturating_sub(1);
                }
                if app.cursor.1 < data.len() && app.cursor.0 > data[app.cursor.1].len() {
                    app.cursor.0 = data[app.cursor.1].len();
                }

                if let Some(renderer) = &mut app.edit_display {
                    renderer.update_buffer(data);
                    renderer.follow_cursor(app.cursor.0, app.cursor.1, 3);
                }
            }
        }

        KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
            if let Some(data) = &mut app.edit_data {
                while data.len() <= app.cursor.1 {
                    data.push(vec![]);
                }
                let row_len = data[app.cursor.1].len();
                data[app.cursor.1].insert(app.cursor.0.min(row_len), c);
                app.cursor.0 += 1;
                if let Some(renderer) = &mut app.edit_display {
                    renderer.update_buffer(data);
                    renderer.follow_cursor(app.cursor.0, app.cursor.1, 3);
                }
            }
        }
        
        _ => {}
    }
    
    Ok(true)
}

fn extract_selection_text(app: &App) -> Option<String> {
    let (start, end) = match (app.selection_start, app.selection_end) {
        (Some(s), Some(e)) => if s.1 < e.1 || (s.1 == e.1 && s.0 < e.0) { (s, e) } else { (e, s) },
        _ => return None,
    };
    
    if let Some(data) = &app.edit_data {
        let mut text = String::new();
        for y in start.1..=end.1 {
            if let Some(row) = data.get(y) {
                let start_x = if y == start.1 { start.0 } else { 0 };
                let end_x = if y == end.1 { end.0 } else { row.len() - 1 };
                for x in start_x..=end_x.min(row.len() - 1) {
                    text.push(row[x]);
                }
                if y < end.1 { text.push('\n'); }
            }
        }
        Some(text)
    } else {
        None
    }
}

fn paste_at_cursor(app: &mut App, text: &str) {
    if let Some(data) = &mut app.edit_data {
        while data.len() <= app.cursor.1 {
            data.push(vec![]);
        }
        
        for ch in text.chars() {
            if ch == '\n' {
                app.cursor.1 += 1;
                app.cursor.0 = 0;
                if app.cursor.1 >= data.len() {
                    data.push(vec![]);
                }
            } else {
                let row_len = data[app.cursor.1].len();
                data[app.cursor.1].insert(app.cursor.0.min(row_len), ch);
                app.cursor.0 += 1;
            }
        }
        
        if let Some(renderer) = &mut app.edit_display {
            renderer.update_buffer(data);
            renderer.follow_cursor(app.cursor.0, app.cursor.1, 3);
        }
    }
}

fn copy_to_clipboard(text: &str) -> Result<()> {
    // KITTY-NATIVE: Direct pbcopy, no copypasta
    let mut child = std::process::Command::new("pbcopy")
        .stdin(std::process::Stdio::piped())
        .spawn()?;

    if let Some(stdin) = child.stdin.as_mut() {
        use std::io::Write;
        stdin.write_all(text.as_bytes())?;
    }

    child.wait()?;
    Ok(())
}

fn paste_from_clipboard() -> Result<String> {
    // KITTY-NATIVE: Direct pbpaste, no copypasta
    let output = std::process::Command::new("pbpaste")
        .output()?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        Err(anyhow::anyhow!("pbpaste failed"))
    }
}