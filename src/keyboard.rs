// Keyboard handling for chonk-note
use crate::App;
use crate::kitty_native::{KeyCode, KeyEvent, KeyModifiers};
use crate::config::{layout, navigation};
use anyhow::Result;

pub async fn handle_input(app: &mut App, key: KeyEvent) -> Result<bool> {
    // If in search mode, handle search input
    if app.search_mode {
        match key.code {
            KeyCode::Enter => {
                // Perform search and jump to first result
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
                return Ok(true);
            }
            KeyCode::Esc => {
                // Cancel search
                app.search_mode = false;
                app.search_query.clear();
                app.search_results.clear();
                app.status_message = "Search cancelled".to_string();
                app.needs_redraw = true;
                return Ok(true);
            }
            KeyCode::Backspace => {
                app.search_query.pop();
                app.status_message = format!("Search: {}", app.search_query);
                app.needs_redraw = true;
                return Ok(true);
            }
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) && !key.modifiers.contains(KeyModifiers::SUPER) => {
                app.search_query.push(c);
                app.status_message = format!("Search: {}", app.search_query);
                app.needs_redraw = true;
                return Ok(true);
            }
            _ => return Ok(true),
        }
    }

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

    // Ctrl+S - Manual save
    if key.code == KeyCode::Char('s') && key.modifiers.contains(KeyModifiers::CONTROL) {
        app.save_current_note()?;
        app.status_message = "Note saved".to_string();
        app.needs_redraw = true;
        return Ok(true);
    }

    // Ctrl+F - Search
    if key.code == KeyCode::Char('f') && key.modifiers.contains(KeyModifiers::CONTROL) {
        app.search_mode = true;
        app.search_query.clear();
        app.search_results.clear();
        app.status_message = "Search: ".to_string();
        app.needs_redraw = true;
        return Ok(true);
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

    // Ctrl+Z - Undo
    if key.code == KeyCode::Char('z') && key.modifiers.contains(KeyModifiers::CONTROL) && !key.modifiers.shift {
        if app.undo_stack.undo(&mut app.grid) {
            app.status_message = "Undo".to_string();
            app.mark_dirty();
            app.needs_redraw = true;
        } else {
            app.status_message = "Nothing to undo".to_string();
            app.needs_redraw = true;
        }
        return Ok(true);
    }

    // Ctrl+Y or Ctrl+Shift+Z - Redo
    if (key.code == KeyCode::Char('y') && key.modifiers.contains(KeyModifiers::CONTROL)) ||
       (key.code == KeyCode::Char('z') && key.modifiers.contains(KeyModifiers::CONTROL) && key.modifiers.shift) {
        if app.undo_stack.redo(&mut app.grid) {
            app.status_message = "Redo".to_string();
            app.mark_dirty();
            app.needs_redraw = true;
        } else {
            app.status_message = "Nothing to redo".to_string();
            app.needs_redraw = true;
        }
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
        if let Some(cut) = app.grid.copy_block() {
            // Get selection bounds for undo
            if let Some(ref sel) = app.grid.selection {
                let (min_row, min_col, max_row, max_col) = sel.bounds();

                // Collect the content that will be replaced (spaces)
                let mut replaced_content = Vec::new();
                for row in min_row..=max_row {
                    let mut row_content = String::new();
                    for col in min_col..=max_col {
                        row_content.push(app.grid.get(row, col));
                    }
                    replaced_content.push(row_content);
                }

                // Create undo command for the cut operation (which replaces with spaces)
                let cmd = crate::undo::Command::PasteBlock {
                    row: min_row,
                    col: min_col,
                    lines: vec![" ".repeat(max_col - min_col + 1); max_row - min_row + 1],
                    replaced_content,
                };
                app.undo_stack.push(cmd);

                // Now actually cut the block
                app.grid.cut_block();
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

                app.mark_dirty();
                app.needs_redraw = true;
            }
        }
        return Ok(true);
    }

    // Ctrl+V - Paste from system clipboard
    if key.code == KeyCode::Char('v') && key.modifiers.contains(KeyModifiers::CONTROL) {
        // Try system clipboard first
        if let Ok(mut clipboard) = arboard::Clipboard::new() {
            if let Ok(text) = clipboard.get_text() {
                let raw_lines: Vec<String> = text.lines().map(|s| s.to_string()).collect();

                // Apply word wrapping if soft_wrap_paste is enabled
                let final_lines = if app.soft_wrap_paste {
                    // Word wrap long lines at 150 characters
                    const MAX_LINE_WIDTH: usize = 150;
                    let mut wrapped_lines = Vec::new();

                    for line in raw_lines {
                        if line.chars().count() <= MAX_LINE_WIDTH {
                            wrapped_lines.push(line);
                        } else {
                            // Word wrap this line
                            let words: Vec<&str> = line.split_whitespace().collect();
                            let mut current_line = String::new();

                            for word in words {
                                // If adding this word would exceed the limit, start a new line
                                let test_line = if current_line.is_empty() {
                                    word.to_string()
                                } else {
                                    format!("{} {}", current_line, word)
                                };

                                if test_line.chars().count() > MAX_LINE_WIDTH && !current_line.is_empty() {
                                    // Push current line and start a new one
                                    wrapped_lines.push(current_line.clone());
                                    current_line = word.to_string();
                                } else {
                                    current_line = test_line;
                                }
                            }

                            // Push the last line if not empty
                            if !current_line.is_empty() {
                                wrapped_lines.push(current_line);
                            }
                        }
                    }
                    wrapped_lines
                } else {
                    // No wrapping - use raw lines as-is
                    raw_lines
                };

                // Collect replaced content for undo
                if !final_lines.is_empty() {
                    let mut replaced_content = Vec::new();
                    for (line_idx, line) in final_lines.iter().enumerate() {
                        let target_row = app.cursor_row + line_idx;
                        let start_col = if line_idx == 0 { app.cursor_col } else { 0 };
                        let mut row_content = String::new();

                        for (col_offset, _) in line.chars().enumerate() {
                            row_content.push(app.grid.get(target_row, start_col + col_offset));
                        }
                        replaced_content.push(row_content);
                    }

                    // Create undo command for paste
                    let cmd = crate::undo::Command::PasteBlock {
                        row: app.cursor_row,
                        col: app.cursor_col,
                        lines: final_lines.clone(),
                        replaced_content,
                    };
                    cmd.execute(&mut app.grid);
                    app.undo_stack.push(cmd);

                    let status_suffix = if app.soft_wrap_paste { " (wrapped)" } else { "" };
                    app.status_message = format!("Pasted {} rows{}", final_lines.len(), status_suffix);
                    app.mark_dirty();
                    app.needs_redraw = true;
                    return Ok(true);
                }
            }
        }

        // Fall back to internal block clipboard
        if let Some(ref clipboard) = app.block_clipboard {
            // Collect replaced content for undo
            let mut replaced_content = Vec::new();
            for (line_idx, line) in clipboard.iter().enumerate() {
                let target_row = app.cursor_row + line_idx;
                let mut row_content = String::new();

                for (col_offset, _) in line.chars().enumerate() {
                    row_content.push(app.grid.get(target_row, app.cursor_col + col_offset));
                }
                replaced_content.push(row_content);
            }

            // Create undo command for paste
            let cmd = crate::undo::Command::PasteBlock {
                row: app.cursor_row,
                col: app.cursor_col,
                lines: clipboard.clone(),
                replaced_content,
            };
            cmd.execute(&mut app.grid);
            app.undo_stack.push(cmd);

            app.status_message = format!("Pasted {} rows", clipboard.len());
            app.mark_dirty();
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

    // Ctrl+N - Next search result (when search results exist) OR New note (when no search results)
    if key.code == KeyCode::Char('n') && key.modifiers.contains(KeyModifiers::CONTROL) {
        if !app.search_results.is_empty() {
            // Navigate to next search result
            app.next_search_result();
            app.status_message = format!("Match {}/{}", app.current_search_index + 1, app.search_results.len());
            app.needs_redraw = true;
        } else {
            // Create new note
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
        }
        return Ok(true);
    }

    // Ctrl+P - Previous search result (only when search results exist)
    if key.code == KeyCode::Char('p') && key.modifiers.contains(KeyModifiers::CONTROL) && !app.search_results.is_empty() {
        app.prev_search_result();
        app.status_message = format!("Match {}/{}", app.current_search_index + 1, app.search_results.len());
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
            if app.selected_note_index >= app.notes_list_scroll + layout::VISIBLE_NOTE_COUNT_APPROX {
                app.notes_list_scroll = app.selected_note_index - layout::VISIBLE_NOTE_COUNT_APPROX + 1;
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



    // Arrow keys and navigation - Move cursor
    match key.code {
        // Home/End keys
        KeyCode::Home => {
            app.cursor_col = 0;
            app.needs_redraw = true;
        }
        KeyCode::End => {
            app.cursor_col = app.grid.get_line_length(app.cursor_row);
            app.needs_redraw = true;
        }
        // Mac: Cmd+Left = Scroll viewport left
        KeyCode::Left if key.modifiers.cmd => {
            app.viewport_col = app.viewport_col.saturating_sub(navigation::PAGE_JUMP_COLS);
            app.needs_redraw = true;
        }
        // Mac: Cmd+Right = Scroll viewport right
        KeyCode::Right if key.modifiers.cmd => {
            app.viewport_col += navigation::PAGE_JUMP_COLS;
            app.needs_redraw = true;
        }
        // PageUp/PageDown keys (also Cmd+Up/Down on Mac)
        KeyCode::PageUp => {
            app.cursor_row = app.cursor_row.saturating_sub(navigation::PAGE_JUMP_ROWS);
            app.needs_redraw = true;
        }
        KeyCode::PageDown => {
            app.cursor_row += navigation::PAGE_JUMP_ROWS;
            app.needs_redraw = true;
        }
        // Mac: Cmd+Up = PageUp
        KeyCode::Up if key.modifiers.contains(KeyModifiers::SUPER) => {
            app.cursor_row = app.cursor_row.saturating_sub(navigation::PAGE_JUMP_ROWS);
            app.needs_redraw = true;
        }
        // Mac: Cmd+Down = PageDown
        KeyCode::Down if key.modifiers.contains(KeyModifiers::SUPER) => {
            app.cursor_row += navigation::PAGE_JUMP_ROWS;
            app.needs_redraw = true;
        }
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
            } else if app.cursor_row > 0 {
                // Move to end of previous line
                app.cursor_row -= 1;
                app.cursor_col = app.grid.get_line_length(app.cursor_row);
            }
            app.needs_redraw = true;
        }
        KeyCode::Right => {
            let line_len = app.grid.get_line_length(app.cursor_row);
            if app.cursor_col < line_len {
                app.cursor_col += 1;
            } else {
                // Move to start of next line
                app.cursor_row += 1;
                app.cursor_col = 0;
            }
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

            // Microsoft Word style backspace
            if app.cursor_col > 0 {
                // Delete character before cursor and shift left
                let deleted_char = app.grid.get(app.cursor_row, app.cursor_col - 1);
                let cmd = crate::undo::Command::DeleteChar {
                    row: app.cursor_row,
                    col: app.cursor_col - 1,
                    deleted_char,
                };
                cmd.execute(&mut app.grid);
                app.undo_stack.push(cmd);

                app.cursor_col -= 1;
                app.mark_dirty();
            } else if app.cursor_row > 0 {
                // Join with previous line (like Word)
                let lines = app.grid.to_lines();

                // Bounds check: make sure previous line exists
                if app.cursor_row - 1 >= lines.len() {
                    // Previous line doesn't exist, can't join
                    app.needs_redraw = true;
                    return Ok(true);
                }

                let current_line = if app.cursor_row < lines.len() {
                    lines[app.cursor_row].clone()
                } else {
                    String::new()
                };
                let prev_line = &lines[app.cursor_row - 1];
                let prev_line_len = prev_line.chars().count();

                // Create undo command for line deletion
                let cmd = crate::undo::Command::DeleteLine {
                    row: app.cursor_row,
                    deleted_line: current_line.clone(),
                    prev_line_length: prev_line_len,
                };
                cmd.execute(&mut app.grid);
                app.undo_stack.push(cmd);

                // Move cursor to end of previous line
                app.cursor_row -= 1;
                app.cursor_col = prev_line_len;
                app.mark_dirty();
            }
            app.needs_redraw = true;
        }
        KeyCode::Delete => {
            // Microsoft Word style delete - delete character at cursor
            let line_len = app.grid.get_line_length(app.cursor_row);

            if app.cursor_col < line_len {
                // Delete character at cursor position and shift left
                let deleted_char = app.grid.get(app.cursor_row, app.cursor_col);
                let cmd = crate::undo::Command::DeleteChar {
                    row: app.cursor_row,
                    col: app.cursor_col,
                    deleted_char,
                };
                cmd.execute(&mut app.grid);
                app.undo_stack.push(cmd);

                app.mark_dirty();
            }
            // Note: Joining lines with Delete at end requires to_lines() for line shifting
            // This is less common than character deletion and can stay as-is for now
            app.needs_redraw = true;
        }
        KeyCode::Enter => {
            // Microsoft Word style enter - insert new line and move content after cursor down
            let lines = app.grid.to_lines();
            let current_line = if app.cursor_row < lines.len() {
                lines[app.cursor_row].clone()
            } else {
                String::new()
            };

            let chars: Vec<char> = current_line.chars().collect();
            let text_after_cursor: String = chars.iter().skip(app.cursor_col).collect();

            // Create undo command for newline insertion
            let cmd = crate::undo::Command::InsertNewLine {
                row: app.cursor_row,
                col: app.cursor_col,
                text_after_cursor: text_after_cursor.clone(),
            };
            cmd.execute(&mut app.grid);
            app.undo_stack.push(cmd);

            // Move cursor to beginning of new line
            app.cursor_row += 1;
            app.cursor_col = 0;

            app.mark_dirty();
            app.needs_redraw = true;
        }
        KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) && !key.modifiers.contains(KeyModifiers::SUPER) => {
            // Create undo command for character insertion
            let cmd = crate::undo::Command::InsertChar {
                row: app.cursor_row,
                col: app.cursor_col,
                ch: c,
            };
            cmd.execute(&mut app.grid);
            app.undo_stack.push(cmd);

            app.cursor_col += 1;
            app.mark_dirty();
            app.needs_redraw = true;
        }
        _ => {}
    }

    Ok(true)
}
