// Keyboard handling for chonk-note
use crate::App;
use crate::kitty_native::{KeyCode, KeyEvent, KeyModifiers};
use anyhow::Result;

pub async fn handle_input(app: &mut App, key: KeyEvent) -> Result<bool> {
    // If editing title, handle title editing keys
    if app.editing_title {
        match key.code {
            KeyCode::Enter => {
                // Save title and exit editing mode
                if let Some(ref mut current_note) = app.notes_mode.current_note {
                    current_note.title = app.title_buffer.clone();
                    let lines = app.grid.to_lines();
                    let content = lines.join("\n");
                    let _ = app.notes_mode.db.update_note(&current_note.id, app.title_buffer.clone(), content, current_note.tags.clone());

                    // Update the note in the list
                    if app.selected_note_index < app.notes_list.len() {
                        app.notes_list[app.selected_note_index].title = app.title_buffer.clone();
                    }
                }
                app.editing_title = false;
                app.sidebar_expanded = false;
                app.needs_redraw = true;
                return Ok(true);
            }
            KeyCode::Esc => {
                // Cancel editing
                app.editing_title = false;
                app.sidebar_expanded = false;
                app.needs_redraw = true;
                return Ok(true);
            }
            KeyCode::Backspace => {
                app.title_buffer.pop();
                app.needs_redraw = true;
                return Ok(true);
            }
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) && !key.modifiers.contains(KeyModifiers::SUPER) => {
                app.title_buffer.push(c);
                app.needs_redraw = true;
                return Ok(true);
            }
            _ => return Ok(true),
        }
    }

    // Ctrl+Q - Quit
    if key.code == KeyCode::Char('q') && key.modifiers.contains(KeyModifiers::CONTROL) {
        app.exit_requested = true;
        return Ok(false);
    }

    // Ctrl+G - Toggle grid lines
    if key.code == KeyCode::Char('g') && key.modifiers.contains(KeyModifiers::CONTROL) {
        app.show_grid_lines = !app.show_grid_lines;
        app.status_message = if app.show_grid_lines {
            "Grid lines ON".to_string()
        } else {
            "Grid lines OFF".to_string()
        };
        app.needs_redraw = true;
        return Ok(true);
    }

    // Ctrl+C - Copy block to system clipboard
    if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
        if let Some(copied) = app.grid.copy_block() {
            app.block_clipboard = Some(copied.clone());

            // Also copy to system clipboard
            if let Ok(mut clipboard) = arboard::Clipboard::new() {
                let text = copied.join("\n");
                let _ = clipboard.set_text(text);
                app.status_message = format!("Copied {} rows to system clipboard", copied.len());
            } else {
                app.status_message = format!("Copied {} rows", copied.len());
            }

            app.needs_redraw = true;
        }
        return Ok(true);
    }

    // Ctrl+X - Cut block to system clipboard
    if key.code == KeyCode::Char('x') && key.modifiers.contains(KeyModifiers::CONTROL) {
        if let Some(cut) = app.grid.cut_block() {
            app.block_clipboard = Some(cut.clone());

            // Also copy to system clipboard
            if let Ok(mut clipboard) = arboard::Clipboard::new() {
                let text = cut.join("\n");
                let _ = clipboard.set_text(text);
                app.status_message = format!("Cut {} rows to system clipboard", cut.len());
            } else {
                app.status_message = format!("Cut {} rows", cut.len());
            }

            app.needs_redraw = true;
        }
        return Ok(true);
    }

    // Ctrl+V - Paste from system clipboard
    if key.code == KeyCode::Char('v') && key.modifiers.contains(KeyModifiers::CONTROL) {
        // Try system clipboard first
        if let Ok(mut clipboard) = arboard::Clipboard::new() {
            if let Ok(text) = clipboard.get_text() {
                let lines: Vec<String> = text.lines().map(|s| s.to_string()).collect();
                app.grid.paste_block(&lines, app.cursor_row, app.cursor_col);
                app.status_message = format!("Pasted {} rows from system clipboard", lines.len());
                app.needs_redraw = true;
                return Ok(true);
            }
        }

        // Fall back to internal block clipboard
        if let Some(ref clipboard) = app.block_clipboard {
            app.grid.paste_block(clipboard, app.cursor_row, app.cursor_col);
            app.status_message = format!("Pasted {} rows", clipboard.len());
            app.needs_redraw = true;
        }
        return Ok(true);
    }

    // Escape - Clear selection
    if key.code == KeyCode::Esc {
        app.grid.clear_selection();
        app.needs_redraw = true;
        return Ok(true);
    }

    // Ctrl+N - New note
    if key.code == KeyCode::Char('n') && key.modifiers.contains(KeyModifiers::CONTROL) {
        // Save current note
        if let Some(ref current_note) = app.notes_mode.current_note {
            let lines = app.grid.to_lines();
            let content = lines.join("\n");
            let _ = app.notes_mode.db.update_note(&current_note.id, current_note.title.clone(), content, current_note.tags.clone());
        }

        // Create new note
        let new_note = app.notes_mode.db.create_note("Untitled".to_string(), String::new(), vec![])?;
        app.notes_mode.current_note = Some(new_note);

        // Clear grid
        app.grid.clear();
        app.cursor_row = 0;
        app.cursor_col = 0;

        // Refresh notes list
        if let Ok(notes) = app.notes_mode.db.list_notes(100) {
            app.notes_list = notes;
        }

        app.needs_redraw = true;
        return Ok(true);
    }

    // Ctrl+Up/Down - Navigate notes
    if key.code == KeyCode::Up && key.modifiers.contains(KeyModifiers::CONTROL) {
        if app.selected_note_index > 0 {
            // Save current note
            if let Some(ref current_note) = app.notes_mode.current_note {
                let lines = app.grid.to_lines();
                let content = lines.join("\n");
                let _ = app.notes_mode.db.update_note(&current_note.id, current_note.title.clone(), content, current_note.tags.clone());
            }

            app.selected_note_index -= 1;
            if app.selected_note_index < app.notes_list_scroll {
                app.notes_list_scroll = app.selected_note_index;
            }

            // Load selected note
            if !app.notes_list.is_empty() {
                let note = &app.notes_list[app.selected_note_index];
                let lines: Vec<String> = note.content.lines().map(|s| s.to_string()).collect();
                app.grid = crate::chunked_grid::ChunkedGrid::from_lines(&lines);
                app.cursor_row = 0;
                app.cursor_col = 0;
                app.notes_mode.current_note = Some(note.clone());
            }

            app.needs_redraw = true;
        }
        return Ok(true);
    }

    if key.code == KeyCode::Down && key.modifiers.contains(KeyModifiers::CONTROL) {
        if app.selected_note_index < app.notes_list.len().saturating_sub(1) {
            // Save current note
            if let Some(ref current_note) = app.notes_mode.current_note {
                let lines = app.grid.to_lines();
                let content = lines.join("\n");
                let _ = app.notes_mode.db.update_note(&current_note.id, current_note.title.clone(), content, current_note.tags.clone());
            }

            app.selected_note_index += 1;
            let visible_count = 30; // Approximate
            if app.selected_note_index >= app.notes_list_scroll + visible_count {
                app.notes_list_scroll = app.selected_note_index - visible_count + 1;
            }

            // Load selected note
            if !app.notes_list.is_empty() {
                let note = &app.notes_list[app.selected_note_index];
                let lines: Vec<String> = note.content.lines().map(|s| s.to_string()).collect();
                app.grid = crate::chunked_grid::ChunkedGrid::from_lines(&lines);
                app.cursor_row = 0;
                app.cursor_col = 0;
                app.notes_mode.current_note = Some(note.clone());
            }

            app.needs_redraw = true;
        }
        return Ok(true);
    }


    // Arrow keys - Move cursor
    match key.code {
        KeyCode::Up if !key.modifiers.contains(KeyModifiers::CONTROL) => {
            if app.cursor_row > 0 {
                app.cursor_row -= 1;
            }
            app.needs_redraw = true;
        }
        KeyCode::Down if !key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.cursor_row += 1;
            app.needs_redraw = true;
        }
        KeyCode::Left => {
            if app.cursor_col > 0 {
                app.cursor_col -= 1;
            }
            app.needs_redraw = true;
        }
        KeyCode::Right => {
            app.cursor_col += 1;
            app.needs_redraw = true;
        }
        KeyCode::Backspace => {
            if app.cursor_col > 0 {
                app.cursor_col -= 1;
                app.grid.delete_at(app.cursor_row, app.cursor_col);
            } else if app.cursor_row > 0 {
                // TODO: Handle backspace at start of line
                app.cursor_row -= 1;
            }
            app.needs_redraw = true;
        }
        KeyCode::Enter => {
            app.cursor_row += 1;
            app.cursor_col = 0;
            app.needs_redraw = true;
        }
        KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) && !key.modifiers.contains(KeyModifiers::SUPER) => {
            // Insert character at cursor position
            app.grid.set(app.cursor_row, app.cursor_col, c);
            app.cursor_col += 1;
            app.needs_redraw = true;
        }
        _ => {}
    }

    // Auto-save after edits
    if let Some(ref current_note) = app.notes_mode.current_note {
        let lines = app.grid.to_lines();
        let content = lines.join("\n");
        let _ = app.notes_mode.db.update_note(&current_note.id, current_note.title.clone(), content, current_note.tags.clone());
    }

    Ok(true)
}
