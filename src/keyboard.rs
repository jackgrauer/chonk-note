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
                    app.save_current_note()?;

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
            KeyCode::Char(_) if key.modifiers.contains(KeyModifiers::CONTROL) || key.modifiers.contains(KeyModifiers::SUPER) => {
                // Let Ctrl/Cmd shortcuts fall through to be handled below
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

    // Ctrl+R - Toggle word wrap
    if key.code == KeyCode::Char('r') && key.modifiers.contains(KeyModifiers::CONTROL) {
        let _ = std::fs::write("/tmp/chonk-debug.log", format!("Ctrl+R detected! wrap={}\n", app.word_wrap));
        app.word_wrap = !app.word_wrap;
        app.status_message = if app.word_wrap {
            "Word wrap ON".to_string()
        } else {
            "Word wrap OFF".to_string()
        };
        app.needs_redraw = true;
        return Ok(true);
    }

    // Ctrl+A - Select all
    if key.code == KeyCode::Char('a') && key.modifiers.contains(KeyModifiers::CONTROL) {
        // Find the bounds of all content
        let lines = app.grid.to_lines();
        if !lines.is_empty() {
            let max_row = lines.len() - 1;
            let max_col = lines.iter()
                .map(|line| line.trim_end().len())
                .max()
                .unwrap_or(0)
                .saturating_sub(1);

            // Select from (0,0) to (max_row, max_col)
            app.grid.start_selection(0, 0);
            app.grid.update_selection(max_row, max_col);
            app.status_message = format!("Selected all ({} rows)", lines.len());
            app.needs_redraw = true;
        }
        return Ok(true);
    }

    // Ctrl+C - Copy block to system clipboard
    if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
        if let Some(copied) = app.grid.copy_block() {
            app.block_clipboard = Some(copied.clone());

            // Also copy to system clipboard
            if let Ok(mut clipboard) = arboard::Clipboard::new() {
                let text = copied.join("\n");
                if let Err(e) = clipboard.set_text(text) {
                    app.status_message = format!("Copied {} rows (clipboard error: {})", copied.len(), e);
                } else {
                    app.status_message = format!("Copied {} rows to system clipboard", copied.len());
                }
            } else {
                app.status_message = format!("Copied {} rows (clipboard unavailable)", copied.len());
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
                if let Err(e) = clipboard.set_text(text) {
                    app.status_message = format!("Cut {} rows (clipboard error: {})", cut.len(), e);
                } else {
                    app.status_message = format!("Cut {} rows to system clipboard", cut.len());
                }
            } else {
                app.status_message = format!("Cut {} rows (clipboard unavailable)", cut.len());
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
        app.save_current_note()?;

        // Create new note
        let new_note = app.notes_mode.db.create_note("Untitled".to_string(), String::new(), vec![])?;
        app.notes_mode.current_note = Some(new_note);

        // Clear grid
        app.grid.clear();
        app.cursor_row = 0;
        app.cursor_col = 0;
        app.viewport_row = 0;
        app.viewport_col = 0;

        // Refresh notes list
        if let Ok(notes) = app.notes_mode.db.list_notes(100) {
            app.notes_list = notes;
        }

        app.needs_redraw = true;
        return Ok(true);
    }

    // Ctrl+D - Delete current note (with confirmation)
    if key.code == KeyCode::Char('d') && key.modifiers.contains(KeyModifiers::CONTROL) {
        if let Some(confirm_note) = app.delete_confirmation_note {
            // Second press - actually delete
            if confirm_note == app.selected_note_index && !app.notes_list.is_empty() {
                let note_id = app.notes_list[app.selected_note_index].id.clone();
                app.notes_mode.db.delete_note(&note_id)?;

                // Refresh notes list
                if let Ok(notes) = app.notes_mode.db.list_notes(100) {
                    app.notes_list = notes;
                }

                // Load first note if any remain
                if !app.notes_list.is_empty() {
                    app.selected_note_index = 0;
                    let first_note = &app.notes_list[0];
                    let lines: Vec<String> = first_note.content.lines().map(|s| s.to_string()).collect();
                    app.grid = crate::chunked_grid::ChunkedGrid::from_lines(&lines);
                    app.notes_mode.current_note = Some(first_note.clone());
                } else {
                    app.grid.clear();
                    app.notes_mode.current_note = None;
                }

                app.cursor_row = 0;
                app.cursor_col = 0;
                app.delete_confirmation_note = None;
                app.status_message = "Note deleted".to_string();
                app.needs_redraw = true;
            }
        } else {
            // First press - ask for confirmation
            app.delete_confirmation_note = Some(app.selected_note_index);
            app.status_message = "Press Ctrl+D again to delete this note".to_string();
            app.needs_redraw = true;
        }

        return Ok(true);
    }

    // Any other key - cancel delete confirmation
    if app.delete_confirmation_note.is_some() {
        app.delete_confirmation_note = None;
        app.status_message = "Delete cancelled".to_string();
        app.needs_redraw = true;
    }

    // Ctrl+Up/Down - Navigate notes
    if key.code == KeyCode::Up && key.modifiers.contains(KeyModifiers::CONTROL) {
        if app.selected_note_index > 0 {
            // Save current note
            app.save_current_note()?;

            app.selected_note_index -= 1;
            if app.selected_note_index < app.notes_list_scroll {
                app.notes_list_scroll = app.selected_note_index;
            }

            // Reload notes list to get fresh data
            if let Ok(notes) = app.notes_mode.db.list_notes(100) {
                app.notes_list = notes;
            }

            // Load selected note
            if !app.notes_list.is_empty() {
                let note = &app.notes_list[app.selected_note_index];
                let lines: Vec<String> = note.content.lines().map(|s| s.to_string()).collect();
                app.grid = crate::chunked_grid::ChunkedGrid::from_lines(&lines);
                app.cursor_row = 0;
                app.cursor_col = 0;
                app.viewport_row = 0;
                app.viewport_col = 0;
                app.notes_mode.current_note = Some(note.clone());
            }

            app.needs_redraw = true;
        }
        return Ok(true);
    }

    if key.code == KeyCode::Down && key.modifiers.contains(KeyModifiers::CONTROL) {
        if app.selected_note_index < app.notes_list.len().saturating_sub(1) {
            // Save current note
            app.save_current_note()?;

            app.selected_note_index += 1;
            if app.selected_note_index >= app.notes_list_scroll + crate::VISIBLE_NOTE_COUNT_APPROX {
                app.notes_list_scroll = app.selected_note_index - crate::VISIBLE_NOTE_COUNT_APPROX + 1;
            }

            // Reload notes list to get fresh data
            if let Ok(notes) = app.notes_mode.db.list_notes(100) {
                app.notes_list = notes;
            }

            // Load selected note
            if !app.notes_list.is_empty() {
                let note = &app.notes_list[app.selected_note_index];
                let lines: Vec<String> = note.content.lines().map(|s| s.to_string()).collect();
                app.grid = crate::chunked_grid::ChunkedGrid::from_lines(&lines);
                app.cursor_row = 0;
                app.cursor_col = 0;
                app.viewport_row = 0;
                app.viewport_col = 0;
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
            // If there's a selection, delete it
            if app.grid.selection.is_some() {
                app.grid.cut_block(); // Just clears the block, no copying
                app.mark_dirty();
                app.needs_redraw = true;
                return Ok(true);
            }

            // Normal backspace behavior
            if app.cursor_col > 0 {
                app.cursor_col -= 1;
                app.grid.delete_at(app.cursor_row, app.cursor_col);
                app.mark_dirty();
            } else if app.cursor_row > 0 {
                // Move to end of previous line
                app.cursor_row -= 1;
                // Find the end of the previous line
                let mut end_col = 0;
                for col in 0..1000 {
                    if app.grid.get(app.cursor_row, col) != ' ' {
                        end_col = col + 1;
                    }
                }
                app.cursor_col = end_col;
            }
            app.needs_redraw = true;
        }
        KeyCode::Enter => {
            // Get everything from cursor to end of line
            let mut chars_after_cursor = Vec::new();
            for col in app.cursor_col..10000 {
                let ch = app.grid.get(app.cursor_row, col);
                if ch != ' ' {
                    chars_after_cursor.push((col, ch));
                } else if !chars_after_cursor.is_empty() {
                    // Hit a space after we found content - keep going to get all content
                    chars_after_cursor.push((col, ch));
                }
            }

            // Find the actual last non-space character
            while let Some(&(_, ch)) = chars_after_cursor.last() {
                if ch == ' ' {
                    chars_after_cursor.pop();
                } else {
                    break;
                }
            }

            // Clear everything after cursor on current line
            for col in app.cursor_col..10000 {
                app.grid.set(app.cursor_row, col, ' ');
            }

            // Move to next line
            app.cursor_row += 1;
            app.cursor_col = 0;

            // Write the chars on the new line
            for (i, (_, ch)) in chars_after_cursor.iter().enumerate() {
                app.grid.set(app.cursor_row, i, *ch);
            }

            app.mark_dirty();
            app.needs_redraw = true;
        }
        KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) && !key.modifiers.contains(KeyModifiers::SUPER) => {
            // Insert character at cursor position
            app.grid.set(app.cursor_row, app.cursor_col, c);
            app.cursor_col += 1;
            app.mark_dirty();
            app.needs_redraw = true;
        }
        _ => {}
    }

    Ok(true)
}
