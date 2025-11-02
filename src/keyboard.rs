// Simplified keyboard handling for chonk-note
use crate::App;
use crate::kitty_native::{KeyCode, KeyEvent, KeyModifiers};
use anyhow::Result;

pub async fn handle_input(app: &mut App, key: KeyEvent) -> Result<bool> {
    // Menu navigation
    if app.notes_menu_expanded {
        return handle_notes_menu(app, key).await;
    }

    // Search mode
    if app.search_mode {
        return handle_search_mode(app, key).await;
    }

    // Title editing
    if app.editing_title {
        return handle_title_editing(app, key).await;
    }

    // Normal editing mode
    handle_normal_mode(app, key).await
}

async fn handle_notes_menu(app: &mut App, key: KeyEvent) -> Result<bool> {
    match key.code {
        KeyCode::Up => {
            if app.selected_note_index > 0 {
                app.selected_note_index -= 1;
                app.needs_redraw = true;
            }
        }
        KeyCode::Down => {
            if app.selected_note_index < app.notes_list.len().saturating_sub(1) {
                app.selected_note_index += 1;
                app.needs_redraw = true;
            }
        }
        KeyCode::Enter => {
            if app.editing_title {
                if let Some(ref mut current_note) = app.current_note {
                    current_note.title = app.title_buffer.clone();
                    app.save_current_note()?;
                    if app.selected_note_index < app.notes_list.len() {
                        app.notes_list[app.selected_note_index].title = app.title_buffer.clone();
                    }
                }
                app.editing_title = false;
            }

            if app.selected_note_index < app.notes_list.len() {
                app.save_current_note()?;
                let note = &app.notes_list[app.selected_note_index];
                app.text_buffer = crate::text_buffer::TextBuffer::from_string(&note.content);
                app.cursor_row = 0;
                app.cursor_col = 0;
                app.viewport_row = 0;
                app.viewport_col = 0;
                app.current_note = Some(note.clone());
                app.notes_menu_expanded = false;
                app.needs_redraw = true;
            }
        }
        KeyCode::Esc => {
            if app.editing_title {
                app.editing_title = false;
            } else {
                app.notes_menu_expanded = false;
            }
            app.needs_redraw = true;
        }
        KeyCode::Backspace => {
            if app.editing_title {
                app.title_buffer.pop();
                app.needs_redraw = true;
            } else {
                app.notes_menu_expanded = false;
            }
        }
        KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) && !key.modifiers.contains(KeyModifiers::SUPER) => {
            if !app.editing_title && app.selected_note_index < app.notes_list.len() {
                app.editing_title = true;
                app.title_buffer = c.to_string();
            } else if app.editing_title {
                app.title_buffer.push(c);
            }
            app.needs_redraw = true;
        }
        _ => {
            app.notes_menu_expanded = false;
        }
    }
    Ok(true)
}

async fn handle_search_mode(app: &mut App, key: KeyEvent) -> Result<bool> {
    match key.code {
        KeyCode::Enter => {
            app.perform_search();
            if !app.search_results.is_empty() {
                let (row, col) = app.search_results[0];
                app.cursor_row = row;
                app.cursor_col = col;
                app.status_message = format!("Found {} matches", app.search_results.len());
            } else {
                app.status_message = "No matches found".to_string();
            }
            app.search_mode = false;
            app.needs_redraw = true;
        }
        KeyCode::Esc => {
            app.search_mode = false;
            app.search_query.clear();
            app.search_results.clear();
            app.status_message = "Search cancelled".to_string();
            app.needs_redraw = true;
        }
        KeyCode::Backspace => {
            app.search_query.pop();
            app.status_message = format!("Search: {}", app.search_query);
            app.needs_redraw = true;
        }
        KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) && !key.modifiers.contains(KeyModifiers::SUPER) => {
            app.search_query.push(c);
            app.status_message = format!("Search: {}", app.search_query);
            app.needs_redraw = true;
        }
        _ => {}
    }
    Ok(true)
}

async fn handle_title_editing(app: &mut App, key: KeyEvent) -> Result<bool> {
    match key.code {
        KeyCode::Enter => {
            if let Some(ref mut current_note) = app.current_note {
                current_note.title = app.title_buffer.clone();
                app.save_current_note()?;
                if app.selected_note_index < app.notes_list.len() {
                    app.notes_list[app.selected_note_index].title = app.title_buffer.clone();
                }
            }
            app.editing_title = false;
            app.needs_redraw = true;
        }
        KeyCode::Esc => {
            app.editing_title = false;
            app.needs_redraw = true;
        }
        KeyCode::Backspace => {
            app.title_buffer.pop();
            app.needs_redraw = true;
        }
        KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) && !key.modifiers.contains(KeyModifiers::SUPER) => {
            app.title_buffer.push(c);
            app.needs_redraw = true;
        }
        _ => {}
    }
    Ok(true)
}

async fn handle_normal_mode(app: &mut App, key: KeyEvent) -> Result<bool> {
    // Handle Ctrl+Q for quit
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        if matches!(key.code, KeyCode::Char('q') | KeyCode::Char('Q')) {
            app.save_current_note()?;
            return Ok(false);
        }
    }

    // Handle Cmd+key combinations (all other shortcuts)
    if key.modifiers.contains(KeyModifiers::SUPER) {
        return handle_command_keys(app, key).await;
    }

    // Regular character input and navigation
    match key.code {
        KeyCode::Char(c) => {
            // Save undo state before modification
            app.undo_stack.push_state(&app.text_buffer, app.cursor_row, app.cursor_col);

            // If there's a selection, delete it first
            if app.text_buffer.selection.is_some() {
                let sel = app.text_buffer.selection.as_ref().unwrap();
                let (start_row, start_col, _, _) = sel.normalized();
                app.text_buffer.delete_selection();
                app.cursor_row = start_row;
                app.cursor_col = start_col;
            }

            app.text_buffer.insert_char(app.cursor_row, app.cursor_col, c);
            app.cursor_col += 1;
            app.mark_dirty();
            app.needs_redraw = true;
        }
        KeyCode::Enter => {
            // Save undo state before modification
            app.undo_stack.push_state(&app.text_buffer, app.cursor_row, app.cursor_col);

            // If there's a selection, delete it first
            if app.text_buffer.selection.is_some() {
                let sel = app.text_buffer.selection.as_ref().unwrap();
                let (start_row, start_col, _, _) = sel.normalized();
                app.text_buffer.delete_selection();
                app.cursor_row = start_row;
                app.cursor_col = start_col;
            }

            let (new_row, new_col) = app.text_buffer.insert_newline(app.cursor_row, app.cursor_col);
            app.cursor_row = new_row;
            app.cursor_col = new_col;
            app.mark_dirty();
            app.needs_redraw = true;
        }
        KeyCode::Backspace => {
            // Save undo state before modification
            app.undo_stack.push_state(&app.text_buffer, app.cursor_row, app.cursor_col);

            // If there's a selection, delete it instead of backspacing
            if app.text_buffer.selection.is_some() {
                let sel = app.text_buffer.selection.as_ref().unwrap();
                let (start_row, start_col, _, _) = sel.normalized();
                app.text_buffer.delete_selection();
                app.cursor_row = start_row;
                app.cursor_col = start_col;
                app.mark_dirty();
                app.needs_redraw = true;
            } else if let Some((new_row, new_col)) = app.text_buffer.backspace(app.cursor_row, app.cursor_col) {
                app.cursor_row = new_row;
                app.cursor_col = new_col;
                app.mark_dirty();
                app.needs_redraw = true;
            }
        }
        KeyCode::Delete => {
            // Save undo state before modification
            app.undo_stack.push_state(&app.text_buffer, app.cursor_row, app.cursor_col);

            // If there's a selection, delete it instead of deleting single char
            if app.text_buffer.selection.is_some() {
                let sel = app.text_buffer.selection.as_ref().unwrap();
                let (start_row, start_col, _, _) = sel.normalized();
                app.text_buffer.delete_selection();
                app.cursor_row = start_row;
                app.cursor_col = start_col;
                app.mark_dirty();
                app.needs_redraw = true;
            } else {
                app.text_buffer.delete_char(app.cursor_row, app.cursor_col);
                app.mark_dirty();
                app.needs_redraw = true;
            }
        }
        KeyCode::Left => {
            // Clear selection and move cursor left
            app.text_buffer.clear_selection();
            if app.cursor_col > 0 {
                app.cursor_col -= 1;
            } else if app.cursor_row > 0 {
                app.cursor_row -= 1;
                app.cursor_col = app.text_buffer.get_line_length(app.cursor_row);
            }
            app.needs_redraw = true;
        }
        KeyCode::Right => {
            // Clear selection and move cursor right
            app.text_buffer.clear_selection();
            let line_len = app.text_buffer.get_line_length(app.cursor_row);
            if app.cursor_col < line_len {
                app.cursor_col += 1;
            } else if app.cursor_row < app.text_buffer.line_count() - 1 {
                app.cursor_row += 1;
                app.cursor_col = 0;
            }
            app.needs_redraw = true;
        }
        KeyCode::Up => {
            // Clear selection and move cursor up
            app.text_buffer.clear_selection();
            if app.cursor_row > 0 {
                app.cursor_row -= 1;
                let line_len = app.text_buffer.get_line_length(app.cursor_row);
                app.cursor_col = app.cursor_col.min(line_len);
            }
            app.needs_redraw = true;
        }
        KeyCode::Down => {
            // Clear selection and move cursor down
            app.text_buffer.clear_selection();
            if app.cursor_row < app.text_buffer.line_count() - 1 {
                app.cursor_row += 1;
                let line_len = app.text_buffer.get_line_length(app.cursor_row);
                app.cursor_col = app.cursor_col.min(line_len);
            }
            app.needs_redraw = true;
        }
        KeyCode::Home => {
            // Clear selection and move to start of line
            app.text_buffer.clear_selection();
            app.cursor_col = 0;
            app.needs_redraw = true;
        }
        KeyCode::End => {
            // Clear selection and move to end of line
            app.text_buffer.clear_selection();
            app.cursor_col = app.text_buffer.get_line_length(app.cursor_row);
            app.needs_redraw = true;
        }
        KeyCode::PageUp => {
            // Clear selection and move up one page
            app.text_buffer.clear_selection();
            app.cursor_row = app.cursor_row.saturating_sub(20);
            let line_len = app.text_buffer.get_line_length(app.cursor_row);
            app.cursor_col = app.cursor_col.min(line_len);
            app.needs_redraw = true;
        }
        KeyCode::PageDown => {
            // Clear selection and move down one page
            app.text_buffer.clear_selection();
            app.cursor_row = (app.cursor_row + 20).min(app.text_buffer.line_count() - 1);
            let line_len = app.text_buffer.get_line_length(app.cursor_row);
            app.cursor_col = app.cursor_col.min(line_len);
            app.needs_redraw = true;
        }
        KeyCode::Esc => {
            app.text_buffer.clear_selection();
            app.needs_redraw = true;
        }
        _ => {}
    }

    Ok(true)
}

async fn handle_command_keys(app: &mut App, key: KeyEvent) -> Result<bool> {
    match key.code {
        // Save
        KeyCode::Char('s') | KeyCode::Char('S') => {
            app.save_current_note()?;
            app.status_message = "Saved".to_string();
            app.needs_redraw = true;
        }

        // New note
        KeyCode::Char('n') | KeyCode::Char('N') => {
            if !app.search_results.is_empty() && !app.search_mode {
                app.next_search_result();
            } else {
                app.save_current_note()?;
                let note = app.notes_manager.create_note("Untitled", "")?;
                app.notes_list.insert(0, note.clone());
                app.text_buffer = crate::text_buffer::TextBuffer::from_string(&note.content);
                app.cursor_row = 0;
                app.cursor_col = 0;
                app.current_note = Some(note);
                app.selected_note_index = 0;
                app.status_message = "New note created".to_string();
            }
            app.needs_redraw = true;
        }

        // Previous search result
        KeyCode::Char('p') | KeyCode::Char('P') => {
            if !app.search_results.is_empty() {
                app.prev_search_result();
                app.needs_redraw = true;
            }
        }

        // Delete note
        KeyCode::Char('d') | KeyCode::Char('D') => {
            if app.delete_confirmation_note == Some(app.selected_note_index) {
                if let Some(ref current_note) = app.current_note {
                    app.notes_manager.delete_note(&current_note)?;
                    app.notes_list.remove(app.selected_note_index);

                    if !app.notes_list.is_empty() {
                        app.selected_note_index = app.selected_note_index.min(app.notes_list.len() - 1);
                        let note = &app.notes_list[app.selected_note_index];
                        app.text_buffer = crate::text_buffer::TextBuffer::from_string(&note.content);
                        app.current_note = Some(note.clone());
                    } else {
                        app.text_buffer = crate::text_buffer::TextBuffer::new();
                        app.current_note = None;
                    }

                    app.status_message = "Note deleted".to_string();
                }
                app.delete_confirmation_note = None;
            } else {
                app.delete_confirmation_note = Some(app.selected_note_index);
                app.status_message = "Press Cmd+D again to confirm deletion".to_string();
            }
            app.needs_redraw = true;
        }

        // Search
        KeyCode::Char('f') | KeyCode::Char('F') => {
            app.search_mode = true;
            app.search_query.clear();
            app.search_results.clear();
            app.status_message = "Search: ".to_string();
            app.needs_redraw = true;
        }

        // Undo
        KeyCode::Char('z') | KeyCode::Char('Z') => {
            if app.undo_stack.undo(&mut app.text_buffer, &mut app.cursor_row, &mut app.cursor_col) {
                app.mark_dirty();
                app.status_message = "Undo".to_string();
            } else {
                app.status_message = "Nothing to undo".to_string();
            }
            app.needs_redraw = true;
        }

        // Redo
        KeyCode::Char('r') | KeyCode::Char('R') => {
            if app.undo_stack.redo(&mut app.text_buffer, &mut app.cursor_row, &mut app.cursor_col) {
                app.mark_dirty();
                app.status_message = "Redo".to_string();
            } else {
                app.status_message = "Nothing to redo".to_string();
            }
            app.needs_redraw = true;
        }

        // Navigate notes
        KeyCode::Up => {
            if app.selected_note_index > 0 {
                app.save_current_note()?;
                app.selected_note_index -= 1;
                let note = &app.notes_list[app.selected_note_index];
                app.text_buffer = crate::text_buffer::TextBuffer::from_string(&note.content);
                app.cursor_row = 0;
                app.cursor_col = 0;
                app.current_note = Some(note.clone());
                app.needs_redraw = true;
            }
        }
        KeyCode::Down => {
            if app.selected_note_index < app.notes_list.len().saturating_sub(1) {
                app.save_current_note()?;
                app.selected_note_index += 1;
                let note = &app.notes_list[app.selected_note_index];
                app.text_buffer = crate::text_buffer::TextBuffer::from_string(&note.content);
                app.cursor_row = 0;
                app.cursor_col = 0;
                app.current_note = Some(note.clone());
                app.needs_redraw = true;
            }
        }

        // Copy
        KeyCode::Char('c') | KeyCode::Char('C') => {
            if let Some(text) = app.text_buffer.get_selected_text() {
                if let Ok(mut clipboard) = arboard::Clipboard::new() {
                    let _ = clipboard.set_text(text);
                    app.status_message = "Copied to clipboard".to_string();
                } else {
                    app.status_message = "Failed to access clipboard".to_string();
                }
            } else {
                app.status_message = "No text selected to copy".to_string();
            }
            app.needs_redraw = true;
        }

        // Cut
        KeyCode::Char('x') | KeyCode::Char('X') => {
            if let Some(text) = app.text_buffer.get_selected_text() {
                // Save undo state before modification
                app.undo_stack.push_state(&app.text_buffer, app.cursor_row, app.cursor_col);

                if let Ok(mut clipboard) = arboard::Clipboard::new() {
                    let _ = clipboard.set_text(text);
                    app.text_buffer.delete_selection();
                    app.mark_dirty();
                    app.status_message = "Cut to clipboard".to_string();
                } else {
                    app.status_message = "Failed to access clipboard".to_string();
                }
            } else {
                app.status_message = "No text selected to cut".to_string();
            }
            app.needs_redraw = true;
        }

        // Paste
        KeyCode::Char('v') | KeyCode::Char('V') => {
            match arboard::Clipboard::new() {
                Ok(mut clipboard) => {
                    match clipboard.get_text() {
                        Ok(text) => {
                            if text.is_empty() {
                                app.status_message = "Clipboard is empty".to_string();
                                app.needs_redraw = true;
                            } else {
                                // Save undo state before modification
                                app.undo_stack.push_state(&app.text_buffer, app.cursor_row, app.cursor_col);

                                // If there's a selection, delete it first
                                if app.text_buffer.selection.is_some() {
                                    let sel = app.text_buffer.selection.as_ref().unwrap();
                                    let (start_row, start_col, _, _) = sel.normalized();
                                    app.text_buffer.delete_selection();
                                    app.cursor_row = start_row;
                                    app.cursor_col = start_col;
                                }

                                // Insert the text into the buffer
                                let (new_row, new_col) = app.text_buffer.insert_text(app.cursor_row, app.cursor_col, &text);
                                app.cursor_row = new_row;
                                app.cursor_col = new_col;

                                app.mark_dirty();
                                app.status_message = format!("Pasted {} chars", text.len());
                                app.needs_redraw = true;
                                let _ = std::io::Write::flush(&mut std::io::stdout());
                            }
                        }
                        Err(e) => {
                            app.status_message = format!("Clipboard read error: {:?}", e);
                            app.needs_redraw = true;
                        }
                    }
                }
                Err(e) => {
                    app.status_message = format!("Clipboard access error: {:?}", e);
                    app.needs_redraw = true;
                }
            }
        }

        // Select all (skip title line - line 0)
        KeyCode::Char('a') | KeyCode::Char('A') => {
            let max_row = app.text_buffer.line_count().saturating_sub(1);
            let max_col = app.text_buffer.get_line_length(max_row);
            // Start selection from line 1 (after title) instead of line 0
            app.text_buffer.start_selection(1, 0);
            app.text_buffer.update_selection(max_row, max_col);
            // Position cursor at end of selection
            app.cursor_row = max_row;
            app.cursor_col = max_col;
            app.status_message = "Selected all".to_string();
            app.needs_redraw = true;
        }

        // Redo (Cmd+Y for compatibility)
        KeyCode::Char('y') | KeyCode::Char('Y') => {
            if app.undo_stack.redo(&mut app.text_buffer, &mut app.cursor_row, &mut app.cursor_col) {
                app.mark_dirty();
                app.status_message = "Redo".to_string();
            } else {
                app.status_message = "Nothing to redo".to_string();
            }
            app.needs_redraw = true;
        }

        _ => {}
    }

    Ok(true)
}
