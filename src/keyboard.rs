// MINIMAL KEYBOARD HANDLING
use crate::{App, AppMode, ActivePane};
use crate::grid_cursor::GridCursor;
use crate::virtual_grid::VirtualGrid;
use anyhow::Result;
use crate::kitty_native::{KeyCode, KeyEvent, KeyModifiers};
use std::io::Write;

// HELIX-CORE INTEGRATION! Professional text editing
use helix_core::{Transaction, Selection, history::State, movement};

// Helper function to update notes rope and sync the grid
fn update_notes_with_content(app: &mut App, content: &str) {
    app.notes_rope = helix_core::Rope::from(content);
    sync_notes_state(app);
}

// Helper to ensure notes grid/cursor/selection are in sync with rope
fn sync_notes_state(app: &mut App) {
    let rope_len = app.notes_rope.len_chars();

    // Clamp selection to rope bounds
    let sel_pos = app.notes_selection.primary().from().min(rope_len);
    app.notes_selection = helix_core::Selection::point(sel_pos);

    // Clear block selection to prevent stale references
    app.notes_block_selection = None;

    // Sync grid and cursor
    app.notes_grid = crate::virtual_grid::VirtualGrid::new(app.notes_rope.clone());
    app.notes_cursor = crate::grid_cursor::GridCursor::new();
}

// Helper to ensure extraction grid/cursor/selection are in sync with rope
fn sync_extraction_state(app: &mut App) {
    let rope_len = app.extraction_rope.len_chars();

    // Clamp selection to rope bounds
    let sel_pos = app.extraction_selection.primary().from().min(rope_len);
    app.extraction_selection = helix_core::Selection::point(sel_pos);

    // Clear block selection to prevent stale references
    app.extraction_block_selection = None;

    // Sync grid and cursor
    app.extraction_grid = crate::virtual_grid::VirtualGrid::new(app.extraction_rope.clone());
    app.extraction_cursor = crate::grid_cursor::GridCursor::new();
}

// Helper function to auto-save notes after edits
fn auto_save_notes_if_needed(app: &mut App) -> Result<()> {
    if app.app_mode != crate::AppMode::NotesEditor || app.active_pane != crate::ActivePane::Left {
        return Ok(());
    }

    save_current_note_changes(app);

    if let Some(ref notes_mode) = app.notes_mode {
        if let Some(ref current_note) = notes_mode.current_note {
            let content = app.notes_rope.to_string();
            if let Some(ref notes_mode) = app.notes_mode {
                let _ = notes_mode.db.update_note(&current_note.id, current_note.title.clone(), content, current_note.tags.clone());
            }
        }
    }

    Ok(())
}

pub async fn handle_input(app: &mut App, key: KeyEvent) -> Result<bool> {
    let rope = app.extraction_rope.slice(..);

    // Handle notes mode specific commands
    if app.app_mode == crate::AppMode::NotesEditor {
        if let Some(ref mut notes) = app.notes_mode {
            match (key.code, key.modifiers) {
                // Ctrl+N - Create new note (in notes mode)
                // Always works with the notes pane (left pane) regardless of which is active
                (KeyCode::Char('n'), mods) if mods.contains(KeyModifiers::CONTROL) && !mods.contains(KeyModifiers::SHIFT) => {
                    // First, save the current note's changes back to the list
                    if let Some(ref current_note) = notes.current_note {
                        // Find the current note in the list and update it
                        for note in app.notes_list.iter_mut() {
                            if note.id == current_note.id {
                                // Update the note's content with the current editor content
                                note.content = app.notes_rope.to_string();
                                break;
                            }
                        }
                    }

                    if let Some(msg) = notes.handle_command(&mut app.notes_rope, &mut app.notes_selection, "new")? {
                        app.status_message = msg;
                    } else {
                        app.status_message = "New note created".to_string();
                    }


                    // Update the notes display renderer
                    if let Some(renderer) = &mut app.notes_display {
                        renderer.update_from_rope(&app.notes_rope);
                    }

                    // Also switch focus to the notes pane
                    app.switch_active_pane(crate::ActivePane::Left);
                    app.needs_redraw = true;
                    return Ok(true);
                }
                // Ctrl+L - List notes
                (KeyCode::Char('l'), mods) if mods.contains(KeyModifiers::CONTROL) => {
                    // Load all notes from database
                    if let Ok(notes_vec) = notes.db.list_notes(100) {
                        app.notes_list = notes_vec;
                        if !app.notes_list.is_empty() {
                            app.selected_note_index = 0;
                        }
                        app.status_message = format!("Loaded {} notes", app.notes_list.len());
                    }
                    // Switch to notes pane to see the list
                    app.switch_active_pane(crate::ActivePane::Left);
                    return Ok(true);
                }
                // Ctrl+Up - Navigate up in notes list
                (KeyCode::Up, mods) if mods.contains(KeyModifiers::CONTROL) => {
                    if !app.notes_list.is_empty() && app.selected_note_index > 0 {
                        app.selected_note_index -= 1;
                        app.needs_redraw = true;
                    }
                    return Ok(true);
                }
                // Ctrl+Down - Navigate down in notes list
                (KeyCode::Down, mods) if mods.contains(KeyModifiers::CONTROL) => {
                    if !app.notes_list.is_empty() && app.selected_note_index < app.notes_list.len() - 1 {
                        app.selected_note_index += 1;
                        app.needs_redraw = true;
                    }
                    return Ok(true);
                }
                // Ctrl+O - Open selected note
                (KeyCode::Char('o'), mods) if mods.contains(KeyModifiers::CONTROL) => {
                    if !app.notes_list.is_empty() && app.selected_note_index < app.notes_list.len() {
                        // First, save the current note's changes back to the list
                        if let Some(ref current_note) = notes.current_note {
                            // Find the current note in the list and update it
                            for note in app.notes_list.iter_mut() {
                                if note.id == current_note.id {
                                    // Update the note's content with the current editor content
                                    note.content = app.notes_rope.to_string();
                                    break;
                                }
                            }
                        }

                        // Now load the selected note
                        let selected_note = app.notes_list[app.selected_note_index].clone();

                        // Update the notes mode with the current note
                        notes.current_note = Some(selected_note.clone());

                        // Store note for later use outside the borrow
                        app.needs_redraw = true;
                    }

                    // Load outside the notes_mode borrow
                    if app.selected_note_index < app.notes_list.len() {
                        let selected_note = app.notes_list[app.selected_note_index].clone();
                        update_notes_with_content(app, &selected_note.content);

                        // Update the display
                        if let Some(renderer) = &mut app.notes_display {
                            renderer.update_from_rope(&app.notes_rope);
                        }

                        // Switch focus to the notes editor pane
                        app.switch_active_pane(crate::ActivePane::Left);
                        app.status_message = format!("Opened: {}", selected_note.title);
                    }
                    return Ok(true);
                }
                // Ctrl+F - Search notes
                (KeyCode::Char('f'), mods) if mods.contains(KeyModifiers::CONTROL) => {
                    if let Some(msg) = notes.handle_command(&mut app.notes_rope, &mut app.notes_selection, "search")? {
                        app.status_message = msg;
                    }
                    return Ok(true);
                }
                // Ctrl+E - Return to PDF mode (E for Editor toggle)
                (KeyCode::Char('e'), mods) if mods.contains(KeyModifiers::CONTROL) && !mods.contains(KeyModifiers::SHIFT) => {
                    app.toggle_notes_mode()?;
                    return Ok(true);
                }
                _ => {} // Fall through to regular text editing
            }
        }
    }

    // macOS-NATIVE KEYBOARD SHORTCUTS!
    match (key.code, key.modifiers) {
        // NAVIGATION - macOS style with proper Helix Rope API (no String conversion!)
        // Cmd+Left/Right = beginning/end of line
        (KeyCode::Left, mods) if mods.contains(KeyModifiers::SUPER) => {
            // Cmd+Left = move to line start using Helix Rope API
            let pos = app.extraction_selection.primary().head;
            let line = rope.char_to_line(pos);
            let line_start = rope.line_to_char(line);
            app.extraction_selection = Selection::point(line_start);
        }

        (KeyCode::Right, mods) if mods.contains(KeyModifiers::SUPER) => {
            // Cmd+Right = move to line end using Helix Rope API
            let pos = app.extraction_selection.primary().head;
            let line = rope.char_to_line(pos);
            let line_start = rope.line_to_char(line);
            let line_len = rope.line(line).len_chars();
            let line_end = line_start + line_len.saturating_sub(1);
            app.extraction_selection = Selection::point(line_end);
        }

        // Option+Left/Right = word by word using proper Helix movement
        (KeyCode::Left, mods) if mods.contains(KeyModifiers::ALT) => {
            // Option+Left = move to previous word using Helix movement
            let range = app.extraction_selection.primary();
            let new_pos = movement::move_prev_word_start(rope.slice(..), range, 1);
            app.extraction_selection = Selection::single(new_pos.anchor, new_pos.head);
        }

        (KeyCode::Right, mods) if mods.contains(KeyModifiers::ALT) => {
            // Option+Right = move to next word using Helix movement
            let range = app.extraction_selection.primary();
            let new_pos = movement::move_next_word_end(rope.slice(..), range, 1);
            app.extraction_selection = Selection::single(new_pos.anchor, new_pos.head);
        }

        // Cmd+Up/Down = document start/end
        (KeyCode::Up, mods) if mods.contains(KeyModifiers::SUPER) => {
            // Cmd+Up = move to document start
            app.extraction_selection = Selection::point(0);
        }

        (KeyCode::Down, mods) if mods.contains(KeyModifiers::SUPER) => {
            // Cmd+Down = move to document end
            app.extraction_selection = Selection::point(rope.len_chars());
        }

        // DELETION - macOS style with proper Helix word boundaries
        // Option+Backspace = delete word
        (KeyCode::Backspace, mods) if mods.contains(KeyModifiers::ALT) => {
            let range = app.extraction_selection.primary();
            if range.head > 0 {
                // Save state before transaction for history
                let state = State {
                    doc: app.extraction_rope.clone(),
                    selection: app.extraction_selection.clone(),
                };

                // Use Helix movement to find the previous word boundary
                let word_start_range = movement::move_prev_word_start(rope, range, 1);
                let start = word_start_range.head;
                let end = range.head;

                // Create transaction to delete the word
                let transaction = Transaction::delete(&app.extraction_rope, std::iter::once((start, end)));

                // Apply transaction
                if transaction.apply(&mut app.extraction_rope) {
                    // Commit to history for undo/redo
                    app.extraction_history.commit_revision(&transaction, &state);
                    sync_extraction_state(app);
                }
            }
        }

        // Cmd+Backspace = delete to line start
        (KeyCode::Backspace, mods) if mods.contains(KeyModifiers::SUPER) => {
            let pos = app.extraction_selection.primary().head;
            let line = app.extraction_rope.char_to_line(pos);
            let line_start = app.extraction_rope.line_to_char(line);
            if pos > line_start {
                // Save state before transaction for history
                let state = State {
                    doc: app.extraction_rope.clone(),
                    selection: app.extraction_selection.clone(),
                };

                let transaction = Transaction::delete(&app.extraction_rope, std::iter::once((line_start, pos)));

                // Apply transaction
                if transaction.apply(&mut app.extraction_rope) {
                    // Commit to history for undo/redo
                    app.extraction_history.commit_revision(&transaction, &state);
                    sync_extraction_state(app);
                }
            }
        }

        // TEXT EDITING - macOS standard
        (KeyCode::Char('a'), mods) if mods.contains(KeyModifiers::SUPER) => {
            // Select All
            app.extraction_selection = Selection::single(0, rope.len_chars());
        }

        (KeyCode::Char('x'), mods) if mods.contains(KeyModifiers::SUPER) => {
            // Check for block selection first
            if let Some(block_sel) = &app.extraction_block_selection {
                // Save state before cut for history
                let old_rope = app.extraction_rope.clone();
                let old_selection = app.extraction_selection.clone();

                // Use non-collapsing block cut
                let cut_data = app.extraction_grid.cut_block(block_sel);
                app.block_clipboard = Some(cut_data.clone());

                // Also copy to system clipboard as plain text
                let text = cut_data.join("\n");
                copy_to_clipboard(&text)?;

                // Update rope from grid
                app.extraction_rope = app.extraction_grid.rope.clone();

                // Create transaction for history
                let transaction = Transaction::change(
                    &old_rope,
                    std::iter::once((0, old_rope.len_chars(), Some(app.extraction_rope.to_string().into())))
                );
                let state = State {
                    doc: old_rope,
                    selection: old_selection,
                };
                app.extraction_history.commit_revision(&transaction, &state);

                // Clear block selection after cut
                app.extraction_block_selection = None;

                app.status_message = format!("Cut block: {} lines", cut_data.len());
            } else {
                // No block selection - just clear any text in clipboard
                app.status_message = "No selection to cut".to_string();
            }
        }

        (KeyCode::Char('c'), mods) if mods.contains(KeyModifiers::SUPER) => {
            // Check for block selection first
            if let Some(block_sel) = &app.extraction_block_selection {
                // Copy block data
                let ((start_line, start_col), (end_line, end_col)) = block_sel.normalized();
                let block_data = app.extraction_grid.get_block(
                    start_col, start_line, end_col, end_line
                );
                app.block_clipboard = Some(block_data.clone());

                // Also copy to system clipboard as plain text
                let text = block_data.join("\n");
                copy_to_clipboard(&text)?;

                app.status_message = format!("Copied block: {} lines", block_data.len());
            } else {
                // Regular copy
                let text = extract_selection_from_rope(app);
                if !text.is_empty() {
                    copy_to_clipboard(&text)?;
                    app.block_clipboard = None; // Clear block clipboard on regular copy
                    app.status_message = "Copied".to_string();
                }
            }
        }

        // Ctrl+Z: Undo
        (KeyCode::Char('z'), mods) if mods.contains(KeyModifiers::CONTROL) => {
            // Get the appropriate history and rope based on active pane
            let (history, rope, selection, renderer) = if app.app_mode == AppMode::NotesEditor {
                match app.active_pane {
                    ActivePane::Left => (&mut app.notes_history, &mut app.notes_rope, &mut app.notes_selection, &mut app.notes_display),
                    ActivePane::Right => (&mut app.extraction_history, &mut app.extraction_rope, &mut app.extraction_selection, &mut app.edit_display),
                }
            } else {
                // PDF mode - always use extraction
                (&mut app.extraction_history, &mut app.extraction_rope, &mut app.extraction_selection, &mut app.edit_display)
            };

            // Perform undo
            if let Some(transaction) = history.undo() {
                let transaction = transaction.clone();
                let success = transaction.apply(rope);

                if success {
                    // Map selection through changes
                    *selection = selection.clone().map(transaction.changes());
                    app.status_message = "Undo".to_string();
                    app.needs_redraw = true;

                    // Update the renderer
                    if let Some(r) = renderer {
                        r.update_from_rope(rope);
                    }
                } else {
                    app.status_message = "Undo failed".to_string();
                }
            } else {
                app.status_message = "Nothing to undo".to_string();
            }
        }

        // Ctrl+U: Redo
        (KeyCode::Char('u'), mods) if mods.contains(KeyModifiers::CONTROL) => {
            // Get the appropriate history and rope based on active pane
            let (history, rope, selection, renderer) = if app.app_mode == AppMode::NotesEditor {
                match app.active_pane {
                    ActivePane::Left => (&mut app.notes_history, &mut app.notes_rope, &mut app.notes_selection, &mut app.notes_display),
                    ActivePane::Right => (&mut app.extraction_history, &mut app.extraction_rope, &mut app.extraction_selection, &mut app.edit_display),
                }
            } else {
                // PDF mode - always use extraction
                (&mut app.extraction_history, &mut app.extraction_rope, &mut app.extraction_selection, &mut app.edit_display)
            };

            // Perform redo
            if let Some(transaction) = history.redo() {
                let transaction = transaction.clone();
                let success = transaction.apply(rope);

                if success {
                    // Map selection through changes
                    *selection = selection.clone().map(transaction.changes());
                    app.status_message = "Redo".to_string();
                    app.needs_redraw = true;

                    // Update the renderer
                    if let Some(r) = renderer {
                        r.update_from_rope(rope);
                    }
                } else {
                    app.status_message = "Redo failed".to_string();
                }
            } else {
                app.status_message = "Nothing to redo".to_string();
            }
        }

        (KeyCode::Char('v'), mods) if mods.contains(KeyModifiers::SUPER) => {
            // Get text from clipboard
            if let Ok(text) = paste_from_clipboard() {
                // Save state before paste for history
                let old_rope = app.extraction_rope.clone();
                let old_selection = app.extraction_selection.clone();

                // Convert text to block data (split by lines)
                let block_data: Vec<String> = text.lines().map(|s| s.to_string()).collect();

                // Paste at cursor position using grid
                let (row, col) = (app.extraction_cursor.row, app.extraction_cursor.col);
                app.extraction_grid.paste_block(row, col, &block_data);

                // Update rope from grid
                app.extraction_rope = app.extraction_grid.rope.clone();

                // Create transaction for history
                let transaction = Transaction::change(
                    &old_rope,
                    std::iter::once((0, old_rope.len_chars(), Some(app.extraction_rope.to_string().into())))
                );
                let state = State {
                    doc: old_rope,
                    selection: old_selection,
                };
                app.extraction_history.commit_revision(&transaction, &state);

                app.status_message = format!("Pasted {} lines", block_data.len());
                app.needs_redraw = true;
            }
        }

        // Select All - Ctrl+A
        (KeyCode::Char('a'), mods) if mods.contains(KeyModifiers::CONTROL) => {
            // Create pink block selection covering entire document
            if app.app_mode == crate::AppMode::NotesEditor {
                match app.active_pane {
                    crate::ActivePane::Left => {
                        let num_lines = app.notes_rope.len_lines();
                        if num_lines > 0 {
                            let last_line_len = app.notes_rope.line(num_lines - 1).len_chars();
                            let mut block_sel = crate::block_selection::BlockSelection::new(0, 0);
                            block_sel.extend_to(num_lines - 1, last_line_len, last_line_len);
                            app.notes_block_selection = Some(block_sel);
                        }
                    }
                    crate::ActivePane::Right => {
                        let num_lines = app.extraction_rope.len_lines();
                        if num_lines > 0 {
                            let last_line_len = app.extraction_rope.line(num_lines - 1).len_chars();
                            let mut block_sel = crate::block_selection::BlockSelection::new(0, 0);
                            block_sel.extend_to(num_lines - 1, last_line_len, last_line_len);
                            app.extraction_block_selection = Some(block_sel);
                        }
                    }
                }
            } else {
                // PDF mode - always extraction pane
                let num_lines = app.extraction_rope.len_lines();
                if num_lines > 0 {
                    let last_line_len = app.extraction_rope.line(num_lines - 1).len_chars();
                    let mut block_sel = crate::block_selection::BlockSelection::new(0, 0);
                    block_sel.extend_to(num_lines - 1, last_line_len, last_line_len);
                    app.extraction_block_selection = Some(block_sel);
                }
            }

            app.needs_redraw = true;
            app.status_message = "Selected all".to_string();
        }

        // Wrap text - Ctrl+W
        (KeyCode::Char('w'), mods) if mods.contains(KeyModifiers::CONTROL) => {
            app.wrap_text = !app.wrap_text;
            app.needs_redraw = true;
            app.status_message = if app.wrap_text {
                "Text wrapping enabled".to_string()
            } else {
                "Text wrapping disabled".to_string()
            };
        }

        // Cut - Ctrl+X (in addition to Cmd+X)
        (KeyCode::Char('x'), mods) if mods.contains(KeyModifiers::CONTROL) => {
            // In Notes mode, work with the appropriate grid and block selection
            if app.app_mode == crate::AppMode::NotesEditor {
                // Ensure grids are synchronized with their ropes before operations
                if app.active_pane == crate::ActivePane::Left {
                    // Make sure notes_grid is in sync with notes_rope
                    if app.notes_grid.rope.len_chars() != app.notes_rope.len_chars() {
                        app.notes_grid = crate::virtual_grid::VirtualGrid::new(app.notes_rope.clone());
                    }
                }

                let (grid, block_selection) = match app.active_pane {
                    crate::ActivePane::Left => (&mut app.notes_grid, &mut app.notes_block_selection),
                    crate::ActivePane::Right => (&mut app.extraction_grid, &mut app.extraction_block_selection),
                };

                if let Some(block_sel) = block_selection.take() {  // Use take() to get ownership
                    // Use non-collapsing block cut
                    let cut_data = grid.cut_block(&block_sel);
                    app.block_clipboard = Some(cut_data.clone());

                    // Also copy to system clipboard as plain text
                    let text = cut_data.join("\n");
                    copy_to_clipboard(&text)?;

                    // Get the start position of the cut area
                    let ((start_line, start_col), _) = block_sel.normalized();

                    // Update the appropriate rope and cursor from grid
                    match app.active_pane {
                        crate::ActivePane::Left => {
                            app.notes_rope = grid.rope.clone();
                            app.notes_cursor.move_to(start_line, start_col);

                            // Update helix selection to match cursor
                            if let Some(char_offset) = app.notes_cursor.to_char_offset(&app.notes_grid) {
                                app.notes_selection = helix_core::Selection::point(char_offset);
                            } else {
                                let line_start = if start_line < app.notes_rope.len_lines() {
                                    app.notes_rope.line_to_char(start_line)
                                } else {
                                    app.notes_rope.len_chars()
                                };
                                app.notes_selection = helix_core::Selection::point(line_start);
                            }
                        },
                        crate::ActivePane::Right => {
                            app.extraction_rope = grid.rope.clone();
                            app.extraction_cursor.move_to(start_line, start_col);

                            // Update helix selection to match cursor
                            if let Some(char_offset) = app.extraction_cursor.to_char_offset(&app.extraction_grid) {
                                app.extraction_selection = helix_core::Selection::point(char_offset);
                            } else {
                                let line_start = if start_line < app.extraction_rope.len_lines() {
                                    app.extraction_rope.line_to_char(start_line)
                                } else {
                                    app.extraction_rope.len_chars()
                                };
                                app.extraction_selection = helix_core::Selection::point(line_start);
                            }
                        },
                    }

                    app.status_message = format!("Cut block: {} lines", cut_data.len());
                    app.needs_redraw = true;
                    return Ok(true);
                }
            } else if let Some(block_sel) = app.extraction_block_selection.take() {
                // PDF mode with block selection
                let cut_data = app.extraction_grid.cut_block(&block_sel);
                app.block_clipboard = Some(cut_data.clone());

                // Also copy to system clipboard as plain text
                let text = cut_data.join("\n");
                copy_to_clipboard(&text)?;

                // Get the start position before updating rope
                let ((start_line, start_col), _) = block_sel.normalized();

                // Update rope from grid
                app.extraction_rope = app.extraction_grid.rope.clone();

                // Update the cursor and selection to the start of the cut area
                app.extraction_cursor.move_to(start_line, start_col);

                // Set the helix selection to a point at the cursor position
                if let Some(char_offset) = app.extraction_cursor.to_char_offset(&app.extraction_grid) {
                    app.extraction_selection = helix_core::Selection::point(char_offset);
                } else {
                    // If we're in virtual space, just set to nearest valid position
                    let line_start = if start_line < app.extraction_rope.len_lines() {
                        app.extraction_rope.line_to_char(start_line)
                    } else {
                        app.extraction_rope.len_chars()
                    };
                    app.extraction_selection = helix_core::Selection::point(line_start);
                }

                app.status_message = format!("Cut block: {} lines", cut_data.len());
                app.needs_redraw = true;
                return Ok(true);
            }

            // No block selection - show message
            app.status_message = "No block selection to cut".to_string();
            app.needs_redraw = true;
            return Ok(true);  // Important: return to prevent falling through to other handlers
        }

        // Copy - Ctrl+C
        (KeyCode::Char('c'), mods) if mods.contains(KeyModifiers::CONTROL) => {
            // Check for block selection in the active pane
            let block_sel = if app.app_mode == crate::AppMode::NotesEditor {
                match app.active_pane {
                    crate::ActivePane::Left => &app.notes_block_selection,
                    crate::ActivePane::Right => &app.extraction_block_selection,
                }
            } else {
                &app.extraction_block_selection
            };

            if let Some(block_sel) = block_sel {
                // Get the appropriate grid
                let grid = if app.app_mode == crate::AppMode::NotesEditor {
                    match app.active_pane {
                        crate::ActivePane::Left => &app.notes_grid,
                        crate::ActivePane::Right => &app.extraction_grid,
                    }
                } else {
                    &app.extraction_grid
                };

                // Copy block data
                let ((start_line, start_col), (end_line, end_col)) = block_sel.normalized();
                let block_data = grid.get_block(start_col, start_line, end_col, end_line);
                app.block_clipboard = Some(block_data.clone());

                // Also copy to system clipboard as plain text
                let text = block_data.join("\n");
                copy_to_clipboard(&text)?;

                app.status_message = format!("Copied block: {} lines", block_data.len());
                app.needs_redraw = true;
            } else {
                // Regular copy
                let text = extract_selection_from_rope(app);
                if !text.is_empty() {
                    copy_to_clipboard(&text)?;
                    app.block_clipboard = None; // Clear block clipboard on regular copy
                    app.status_message = format!("Copied {} characters", text.len());
                    app.needs_redraw = true;
                } else {
                    app.status_message = "Nothing to copy".to_string();
                    app.needs_redraw = true;
                }
            }
        }

        // Paste - Ctrl+V
        (KeyCode::Char('v'), mods) if mods.contains(KeyModifiers::CONTROL) => {
            // Check if we have block clipboard data to paste
            if let Some(block_data) = &app.block_clipboard {
                // In Notes mode, work with the appropriate grid and cursor
                if app.app_mode == crate::AppMode::NotesEditor {
                    let (grid, cursor, rope) = match app.active_pane {
                        crate::ActivePane::Left => {
                            (&mut app.notes_grid, &app.notes_cursor, &mut app.notes_rope)
                        }
                        crate::ActivePane::Right => {
                            (&mut app.extraction_grid, &app.extraction_cursor, &mut app.extraction_rope)
                        }
                    };

                    // Block paste at cursor position
                    grid.paste_block(cursor.row, cursor.col, block_data);

                    // Update rope from grid
                    *rope = grid.rope.clone();
                } else {
                    // PDF mode - paste to extraction grid
                    app.extraction_grid.paste_block(app.extraction_cursor.row, app.extraction_cursor.col, block_data);
                    app.extraction_rope = app.extraction_grid.rope.clone();
                }

                app.status_message = format!("Pasted block: {} lines", block_data.len());
                app.needs_redraw = true;
            } else if let Ok(text) = paste_from_clipboard() {
                // Convert clipboard text to block data for grid-based paste
                let block_data: Vec<String> = text.lines().map(|s| s.to_string()).collect();

                // In Notes mode, work with the appropriate grid, cursor, rope and history
                if app.app_mode == crate::AppMode::NotesEditor {
                    let (grid, cursor, rope, history, selection) = match app.active_pane {
                        crate::ActivePane::Left => {
                            (&mut app.notes_grid, &app.notes_cursor, &mut app.notes_rope,
                             &mut app.notes_history, &mut app.notes_selection)
                        }
                        crate::ActivePane::Right => {
                            (&mut app.extraction_grid, &app.extraction_cursor, &mut app.extraction_rope,
                             &mut app.extraction_history, &mut app.extraction_selection)
                        }
                    };

                    // Save state before paste for history
                    let old_rope = rope.clone();
                    let old_selection = selection.clone();

                    // Paste using grid at cursor position
                    grid.paste_block(cursor.row, cursor.col, &block_data);

                    // Update rope from grid
                    *rope = grid.rope.clone();

                    // Create transaction for history
                    let transaction = Transaction::change(
                        &old_rope,
                        std::iter::once((0, old_rope.len_chars(), Some(rope.to_string().into())))
                    );
                    let state = State {
                        doc: old_rope,
                        selection: old_selection,
                    };
                    history.commit_revision(&transaction, &state);

                } else {
                    // PDF mode - save state before paste for history
                    let old_rope = app.extraction_rope.clone();
                    let old_selection = app.extraction_selection.clone();

                    // Paste to extraction grid
                    app.extraction_grid.paste_block(app.extraction_cursor.row, app.extraction_cursor.col, &block_data);
                    app.extraction_rope = app.extraction_grid.rope.clone();

                    // Create transaction for history
                    let transaction = Transaction::change(
                        &old_rope,
                        std::iter::once((0, old_rope.len_chars(), Some(app.extraction_rope.to_string().into())))
                    );
                    let state = State {
                        doc: old_rope,
                        selection: old_selection,
                    };
                    app.extraction_history.commit_revision(&transaction, &state);
                }

                app.status_message = format!("Pasted {} lines", block_data.len());
                app.needs_redraw = true;
            }
        }

        // Note selection shortcuts - Ctrl+1/2/3/4
        (KeyCode::Char('1'), mods) if mods.contains(KeyModifiers::CONTROL) => {
            if app.app_mode == crate::AppMode::NotesEditor && app.notes_list.len() > 0 {
                // Save current note before switching
                save_current_note_changes(app);

                app.selected_note_index = 0;
                let selected_note = app.notes_list[0].clone();

                // Load the note content
                update_notes_with_content(app, &selected_note.content);

                if let Some(ref mut notes_mode) = app.notes_mode {
                    notes_mode.current_note = Some(selected_note.clone());
                }

                // Update the display
                if let Some(renderer) = &mut app.notes_display {
                    renderer.update_from_rope(&app.notes_rope);
                }

                app.status_message = format!("Selected note 1: {}", selected_note.title);
                app.needs_redraw = true;
            }
        }

        (KeyCode::Char('2'), mods) if mods.contains(KeyModifiers::CONTROL) => {
            if app.app_mode == crate::AppMode::NotesEditor && app.notes_list.len() > 1 {
                // Save current note before switching
                save_current_note_changes(app);

                app.selected_note_index = 1;
                let selected_note = app.notes_list[1].clone();

                // Load the note content
                update_notes_with_content(app, &selected_note.content);

                if let Some(ref mut notes_mode) = app.notes_mode {
                    notes_mode.current_note = Some(selected_note.clone());
                }

                // Update the display
                if let Some(renderer) = &mut app.notes_display {
                    renderer.update_from_rope(&app.notes_rope);
                }

                app.status_message = format!("Selected note 2: {}", selected_note.title);
                app.needs_redraw = true;
            }
        }

        (KeyCode::Char('3'), mods) if mods.contains(KeyModifiers::CONTROL) => {
            if app.app_mode == crate::AppMode::NotesEditor && app.notes_list.len() > 2 {
                // Save current note before switching
                save_current_note_changes(app);

                app.selected_note_index = 2;
                let selected_note = app.notes_list[2].clone();

                // Load the note content
                update_notes_with_content(app, &selected_note.content);

                if let Some(ref mut notes_mode) = app.notes_mode {
                    notes_mode.current_note = Some(selected_note.clone());
                }

                // Update the display
                if let Some(renderer) = &mut app.notes_display {
                    renderer.update_from_rope(&app.notes_rope);
                }

                app.status_message = format!("Selected note 3: {}", selected_note.title);
                app.needs_redraw = true;
            }
        }

        (KeyCode::Char('4'), mods) if mods.contains(KeyModifiers::CONTROL) => {
            if app.app_mode == crate::AppMode::NotesEditor && app.notes_list.len() > 3 {
                // Save current note before switching
                save_current_note_changes(app);

                app.selected_note_index = 3;
                let selected_note = app.notes_list[3].clone();

                // Load the note content
                update_notes_with_content(app, &selected_note.content);

                if let Some(ref mut notes_mode) = app.notes_mode {
                    notes_mode.current_note = Some(selected_note.clone());
                }

                // Update the display
                if let Some(renderer) = &mut app.notes_display {
                    renderer.update_from_rope(&app.notes_rope);
                }

                app.status_message = format!("Selected note 4: {}", selected_note.title);
                app.needs_redraw = true;
            }
        }

        // Ctrl+N - New note
        (KeyCode::Char('n'), mods) if mods.contains(KeyModifiers::CONTROL) => {
            if app.app_mode == crate::AppMode::NotesEditor {
                // Save current note before creating new
                save_current_note_changes(app);

                // Create a new note in the database
                if let Some(ref mut notes_mode) = app.notes_mode {
                    match notes_mode.db.create_note(
                        format!("New Note {}", chrono::Utc::now().format("%Y-%m-%d %H:%M")),
                        String::new(),
                        vec![]
                    ) {
                        Ok(new_note) => {
                            // Add to list and select it
                            app.notes_list.insert(0, new_note.clone());
                            app.selected_note_index = 0;

                            // Store current note before dropping the borrow
                            notes_mode.current_note = Some(new_note.clone());

                            app.status_message = format!("Created: {}", new_note.title);
                            app.needs_redraw = true;
                        }
                        Err(e) => {
                            app.status_message = format!("Failed to create note: {}", e);
                            app.needs_redraw = true;
                        }
                    }
                }

                // Load the new empty note content outside the borrow
                if app.selected_note_index == 0 && !app.notes_list.is_empty() {
                    update_notes_with_content(app, "");

                    // Update display
                    if let Some(renderer) = &mut app.notes_display {
                        renderer.update_from_rope(&app.notes_rope);
                    }
                }
            }
        }

        // Ctrl+D - Delete current note
        (KeyCode::Char('d'), mods) if mods.contains(KeyModifiers::CONTROL) => {
            if app.app_mode == crate::AppMode::NotesEditor {
                // Store note info before mutable borrow
                let note_to_delete = if let Some(ref notes_mode) = app.notes_mode {
                    notes_mode.current_note.clone()
                } else {
                    None
                };

                if let Some(current_note) = note_to_delete {
                    // Delete from database
                    let delete_result = if let Some(ref notes_mode) = app.notes_mode {
                        notes_mode.db.delete_note(&current_note.id)
                    } else {
                        Ok(())
                    };

                    match delete_result {
                        Ok(_) => {
                            // Remove from list
                            app.notes_list.retain(|n| n.id != current_note.id);

                            // Select next note or clear if none
                            if !app.notes_list.is_empty() {
                                app.selected_note_index = app.selected_note_index.min(app.notes_list.len() - 1);
                                let next_note = app.notes_list[app.selected_note_index].clone();

                                // Load the next note
                                update_notes_with_content(app, &next_note.content);

                                if let Some(ref mut notes_mode) = app.notes_mode {
                                    notes_mode.current_note = Some(next_note);
                                }

                                if let Some(renderer) = &mut app.notes_display {
                                    renderer.update_from_rope(&app.notes_rope);
                                }
                            } else {
                                // No notes left
                                update_notes_with_content(app, "");

                                if let Some(ref mut notes_mode) = app.notes_mode {
                                    notes_mode.current_note = None;
                                }

                                if let Some(renderer) = &mut app.notes_display {
                                    renderer.update_from_rope(&app.notes_rope);
                                }
                            }

                            app.status_message = format!("Deleted: {}", current_note.title);
                            app.needs_redraw = true;
                        }
                        Err(e) => {
                            app.status_message = format!("Failed to delete note: {}", e);
                            app.needs_redraw = true;
                        }
                    }
                }
            }
        }

        // Removed Ctrl+S - notes now auto-save on every change

        // Ctrl+R - Rename current note
        (KeyCode::Char('r'), mods) if mods.contains(KeyModifiers::CONTROL) => {
            if app.app_mode == crate::AppMode::NotesEditor {
                if let Some(ref notes_mode) = app.notes_mode {
                    if let Some(ref current_note) = notes_mode.current_note {
                        // For now, generate a new name with timestamp
                        // In a full implementation, you'd prompt for a new name
                        let new_title = format!("Renamed Note {}", chrono::Utc::now().format("%H:%M:%S"));

                        // Update in the list
                        for note in app.notes_list.iter_mut() {
                            if note.id == current_note.id {
                                note.title = new_title.clone();
                                break;
                            }
                        }

                        // Update in database
                        if let Some(ref mut notes_mode) = app.notes_mode {
                            if let Some(ref mut current) = notes_mode.current_note {
                                current.title = new_title.clone();
                                // The actual database update happens on save
                            }
                        }

                        app.status_message = format!("Renamed to: {}", new_title);
                        app.needs_redraw = true;
                    }
                }
            }
        }

        // PDF-specific shortcuts (keep unchanged)
        (KeyCode::Char('q'), mods) if mods.contains(KeyModifiers::CONTROL) => {
            app.exit_requested = true;
        }

        (KeyCode::Char('o'), mods) if mods.contains(KeyModifiers::CONTROL) => {
            app.open_file_picker = true;
        }

        (KeyCode::Char('t'), mods) if mods.contains(KeyModifiers::CONTROL) => {
            app.toggle_extraction_method().await?;
        }


        // Next page - Using PageDown
        (KeyCode::PageDown, _) => {
            app.next_page();
            app.load_pdf_page().await?;
            app.extract_current_page().await?;  // Re-extract text for new page
        }

        // Next page - Ctrl+Right
        (KeyCode::Right, mods) if mods.contains(KeyModifiers::CONTROL) => {
            app.next_page();
            app.load_pdf_page().await?;
            app.extract_current_page().await?;  // Re-extract text for new page
        }

        // Toggle Notes Mode with Ctrl+E (E for Editor mode)
        (KeyCode::Char('e'), mods) if mods.contains(KeyModifiers::CONTROL) && !mods.contains(KeyModifiers::SHIFT) => {
            app.toggle_notes_mode()?;
            return Ok(true);
        }

        // Shift+Up/Down - Navigate notes list
        (KeyCode::Up, mods) if mods.contains(KeyModifiers::SHIFT) && app.app_mode == crate::AppMode::NotesEditor => {
            if app.selected_note_index > 0 {
                // Save current note before switching
                save_current_note_changes(app);

                app.selected_note_index -= 1;

                // Adjust scroll if needed
                if app.selected_note_index < app.notes_list_scroll {
                    app.notes_list_scroll = app.selected_note_index;
                }

                // Load the selected note
                if app.selected_note_index < app.notes_list.len() {
                    let selected_note = app.notes_list[app.selected_note_index].clone();
                    update_notes_with_content(app, &selected_note.content);

                    if let Some(ref mut notes_mode) = app.notes_mode {
                        notes_mode.current_note = Some(selected_note.clone());
                    }

                    if let Some(renderer) = &mut app.notes_display {
                        renderer.update_from_rope(&app.notes_rope);
                    }

                    app.status_message = format!("Selected: {}", selected_note.title);
                    app.unsaved_changes = false;
                }

                app.needs_redraw = true;
            }
        }

        (KeyCode::Down, mods) if mods.contains(KeyModifiers::SHIFT) && app.app_mode == crate::AppMode::NotesEditor => {
            if app.selected_note_index < app.notes_list.len() - 1 {
                // Save current note before switching
                save_current_note_changes(app);

                app.selected_note_index += 1;

                // Adjust scroll if needed (assuming we show 20 notes at a time)
                let visible_count = 20;
                if app.selected_note_index >= app.notes_list_scroll + visible_count {
                    app.notes_list_scroll = app.selected_note_index - visible_count + 1;
                }

                // Load the selected note
                let selected_note = app.notes_list[app.selected_note_index].clone();
                update_notes_with_content(app, &selected_note.content);

                if let Some(ref mut notes_mode) = app.notes_mode {
                    notes_mode.current_note = Some(selected_note.clone());
                }

                if let Some(renderer) = &mut app.notes_display {
                    renderer.update_from_rope(&app.notes_rope);
                }

                app.status_message = format!("Selected: {}", selected_note.title);
                app.unsaved_changes = false;
                app.needs_redraw = true;
            }
        }

        // Previous page - Using PageUp
        (KeyCode::PageUp, _) => {
            app.prev_page();
            app.load_pdf_page().await?;
            app.extract_current_page().await?;  // Re-extract text for new page
        }

        // Previous page - Ctrl+Left
        (KeyCode::Left, mods) if mods.contains(KeyModifiers::CONTROL) => {
            app.prev_page();
            app.load_pdf_page().await?;
            app.extract_current_page().await?;  // Re-extract text for new page
        }

        // Ctrl+J/K navigation for pages (keeps j/k free for typing)
        (KeyCode::Char('j'), mods) if mods.contains(KeyModifiers::CONTROL) => {
            app.next_page();
            app.load_pdf_page().await?;
            app.extract_current_page().await?;
        }

        (KeyCode::Char('k'), mods) if mods.contains(KeyModifiers::CONTROL) => {
            app.prev_page();
            app.load_pdf_page().await?;
            app.extract_current_page().await?;
        }

        // Text zoom disabled - terminal text can't be resized properly
        // These shortcuts now just show a message explaining the limitation
        (KeyCode::Char('+'), mods) if mods.contains(KeyModifiers::CONTROL) => {
            app.status_message = "Text zoom not available (terminal limitation)".to_string();
            app.needs_redraw = true;
        }

        (KeyCode::Char('-'), mods) if mods.contains(KeyModifiers::CONTROL) => {
            app.status_message = "Text zoom not available (terminal limitation)".to_string();
            app.needs_redraw = true;
        }

        (KeyCode::Char('0'), mods) if mods.contains(KeyModifiers::CONTROL) => {
            app.status_message = "Text zoom not available (terminal limitation)".to_string();
            app.needs_redraw = true;
        }

        // BASIC MOVEMENT - Arrow keys (grid-based!)
        (KeyCode::Up, mods) => {
            eprintln!("DEBUG: Up arrow pressed, mods: {:?}", mods);
            // Clear any block selection when using arrow keys
            if app.app_mode == crate::AppMode::NotesEditor {
                match app.active_pane {
                    crate::ActivePane::Left => app.notes_block_selection = None,
                    crate::ActivePane::Right => app.extraction_block_selection = None,
                }
            } else {
                app.extraction_block_selection = None;
            }

            // Get the correct grid cursor based on mode and active pane
            let (grid, cursor, selection) = if app.app_mode == crate::AppMode::NotesEditor {
                match app.active_pane {
                    crate::ActivePane::Left => (&mut app.notes_grid, &mut app.notes_cursor, &mut app.notes_selection),
                    crate::ActivePane::Right => (&mut app.extraction_grid, &mut app.extraction_cursor, &mut app.extraction_selection),
                }
            } else {
                (&mut app.extraction_grid, &mut app.extraction_cursor, &mut app.extraction_selection)
            };

            // Move cursor up in the grid (no text boundary constraints!)
            cursor.move_up();

            // If we can map to a real text position, update the Helix selection too
            // (for text operations like copy/paste)
            if let Some(char_pos) = cursor.to_char_offset(grid) {
                if mods.contains(KeyModifiers::SHIFT) {
                    // Extend selection - keep anchor, move head
                    let anchor = selection.primary().anchor;
                    *selection = Selection::single(anchor, char_pos);
                } else {
                    // Just move cursor
                    *selection = Selection::point(char_pos);
                }
            } else {
                // Cursor is in virtual space - clear any text selection
                if !mods.contains(KeyModifiers::SHIFT) {
                    let pos = grid.rope.line_to_char(cursor.row.min(grid.rope.len_lines().saturating_sub(1)));
                    *selection = Selection::point(pos);
                }
            }
            app.needs_redraw = true;
        }

        (KeyCode::Down, mods) => {
            // Clear any block selection when using arrow keys
            if app.app_mode == crate::AppMode::NotesEditor {
                match app.active_pane {
                    crate::ActivePane::Left => app.notes_block_selection = None,
                    crate::ActivePane::Right => app.extraction_block_selection = None,
                }
            } else {
                app.extraction_block_selection = None;
            }

            // Get the correct grid cursor based on mode and active pane
            let (grid, cursor, selection) = if app.app_mode == crate::AppMode::NotesEditor {
                match app.active_pane {
                    crate::ActivePane::Left => (&mut app.notes_grid, &mut app.notes_cursor, &mut app.notes_selection),
                    crate::ActivePane::Right => (&mut app.extraction_grid, &mut app.extraction_cursor, &mut app.extraction_selection),
                }
            } else {
                (&mut app.extraction_grid, &mut app.extraction_cursor, &mut app.extraction_selection)
            };

            // Allow moving down even past the last line of text!
            let max_rows = 200;  // Allow scrolling down to row 200 even if there's no text
            cursor.move_down(max_rows);

            // If we can map to a real text position, update the Helix selection too
            if let Some(char_pos) = cursor.to_char_offset(grid) {
                if mods.contains(KeyModifiers::SHIFT) {
                    let anchor = selection.primary().anchor;
                    *selection = Selection::single(anchor, char_pos);
                } else {
                    *selection = Selection::point(char_pos);
                }
            } else {
                // Cursor is in virtual space - clear any text selection
                if !mods.contains(KeyModifiers::SHIFT) {
                    let pos = grid.rope.line_to_char(cursor.row.min(grid.rope.len_lines().saturating_sub(1)));
                    *selection = Selection::point(pos);
                }
            }
            app.needs_redraw = true;
        }

        (KeyCode::Left, mods) if !mods.contains(KeyModifiers::SUPER) && !mods.contains(KeyModifiers::ALT) => {
            // Clear any block selection when using arrow keys
            if app.app_mode == crate::AppMode::NotesEditor {
                match app.active_pane {
                    crate::ActivePane::Left => app.notes_block_selection = None,
                    crate::ActivePane::Right => app.extraction_block_selection = None,
                }
            } else {
                app.extraction_block_selection = None;
            }

            // Get the correct grid cursor based on mode and active pane
            let (grid, cursor, selection) = if app.app_mode == crate::AppMode::NotesEditor {
                match app.active_pane {
                    crate::ActivePane::Left => (&mut app.notes_grid, &mut app.notes_cursor, &mut app.notes_selection),
                    crate::ActivePane::Right => (&mut app.extraction_grid, &mut app.extraction_cursor, &mut app.extraction_selection),
                }
            } else {
                (&mut app.extraction_grid, &mut app.extraction_cursor, &mut app.extraction_selection)
            };

            // Move left in grid
            cursor.move_left();

            // Update the Helix selection if we can map to a real text position
            if let Some(char_pos) = cursor.to_char_offset(grid) {
                if mods.contains(KeyModifiers::SHIFT) {
                    let anchor = selection.primary().anchor;
                    *selection = Selection::single(anchor, char_pos);
                } else {
                    *selection = Selection::point(char_pos);
                }
            } else {
                // In virtual space
                if !mods.contains(KeyModifiers::SHIFT) {
                    let pos = grid.rope.line_to_char(cursor.row.min(grid.rope.len_lines().saturating_sub(1)));
                    *selection = Selection::point(pos);
                }
            }
            app.needs_redraw = true;
        }

        (KeyCode::Right, mods) if !mods.contains(KeyModifiers::SUPER) && !mods.contains(KeyModifiers::ALT) => {
            // Clear any block selection when using arrow keys
            if app.app_mode == crate::AppMode::NotesEditor {
                match app.active_pane {
                    crate::ActivePane::Left => app.notes_block_selection = None,
                    crate::ActivePane::Right => app.extraction_block_selection = None,
                }
            } else {
                app.extraction_block_selection = None;
            }

            // Get the correct grid cursor based on mode and active pane
            let (grid, cursor, selection) = if app.app_mode == crate::AppMode::NotesEditor {
                match app.active_pane {
                    crate::ActivePane::Left => (&mut app.notes_grid, &mut app.notes_cursor, &mut app.notes_selection),
                    crate::ActivePane::Right => (&mut app.extraction_grid, &mut app.extraction_cursor, &mut app.extraction_selection),
                }
            } else {
                (&mut app.extraction_grid, &mut app.extraction_cursor, &mut app.extraction_selection)
            };

            // Move right in grid - no limit! Go as far right as you want!
            cursor.move_right(1000);  // Arbitrary large number, cursor will handle it

            // Update the Helix selection if we can map to a real text position
            if let Some(char_pos) = cursor.to_char_offset(grid) {
                if mods.contains(KeyModifiers::SHIFT) {
                    let anchor = selection.primary().anchor;
                    *selection = Selection::single(anchor, char_pos);
                } else {
                    *selection = Selection::point(char_pos);
                }
            } else {
                // In virtual space to the right of text
                if !mods.contains(KeyModifiers::SHIFT) {
                    // Position at end of current line
                    let line_end = if cursor.row < grid.rope.len_lines() {
                        let line_start = grid.rope.line_to_char(cursor.row);
                        let line = grid.rope.line(cursor.row);
                        line_start + line.len_chars().saturating_sub(1)
                    } else {
                        grid.rope.len_chars()
                    };
                    *selection = Selection::point(line_end);
                }
            }

            // Force redraw to show virtual cursor movement
            app.needs_redraw = true;
        }

        // TEXT OPERATIONS
        (KeyCode::Backspace, mods) if !mods.contains(KeyModifiers::ALT) && !mods.contains(KeyModifiers::SUPER) => {
            // Determine which rope/selection to use
            if app.app_mode == crate::AppMode::NotesEditor && app.active_pane == crate::ActivePane::Left {
                // Notes pane
                if let Some(block_sel) = &app.notes_block_selection {
                    // Delete block selection
                    let state = State {
                        doc: app.notes_rope.clone(),
                        selection: app.notes_selection.clone(),
                    };

                    let selection = block_sel.to_selection(&app.notes_rope);
                    let transaction = Transaction::change_by_selection(&app.notes_rope, &selection, |range| {
                        (range.from(), range.to(), None)
                    });

                    if transaction.apply(&mut app.notes_rope) {
                        app.notes_history.commit_revision(&transaction, &state);
                        sync_notes_state(app);
                    }
                } else if app.notes_selection.primary().len() > 0 {
                    // Delete regular selection
                    let state = State {
                        doc: app.notes_rope.clone(),
                        selection: app.notes_selection.clone(),
                    };

                    let transaction = Transaction::change_by_selection(&app.notes_rope, &app.notes_selection, |range| {
                        (range.from(), range.to(), None)
                    });

                    if transaction.apply(&mut app.notes_rope) {
                        app.notes_history.commit_revision(&transaction, &state);
                        sync_notes_state(app);
                    }
                } else if app.notes_selection.primary().head > 0 {
                    // Delete char before cursor
                    let head = app.notes_selection.primary().head;
                    let state = State {
                        doc: app.notes_rope.clone(),
                        selection: app.notes_selection.clone(),
                    };

                    let transaction = Transaction::change(&app.notes_rope, std::iter::once((head - 1, head, None)));

                    if transaction.apply(&mut app.notes_rope) {
                        app.notes_history.commit_revision(&transaction, &state);
                        sync_notes_state(app);
                    }
                }
            } else {
                // Extraction pane
                if let Some(block_sel) = &app.extraction_block_selection {
                    // Delete block selection
                    let state = State {
                        doc: app.extraction_rope.clone(),
                        selection: app.extraction_selection.clone(),
                    };

                    let selection = block_sel.to_selection(&app.extraction_rope);
                    let transaction = Transaction::change_by_selection(&app.extraction_rope, &selection, |range| {
                        (range.from(), range.to(), None)
                    });

                    if transaction.apply(&mut app.extraction_rope) {
                        app.extraction_history.commit_revision(&transaction, &state);
                        sync_extraction_state(app);
                    }
                } else if app.extraction_selection.primary().len() > 0 {
                    // Delete regular selection
                    let state = State {
                        doc: app.extraction_rope.clone(),
                        selection: app.extraction_selection.clone(),
                    };

                    let transaction = Transaction::change_by_selection(&app.extraction_rope, &app.extraction_selection, |range| {
                        (range.from(), range.to(), None)
                    });

                    if transaction.apply(&mut app.extraction_rope) {
                        app.extraction_history.commit_revision(&transaction, &state);
                        sync_extraction_state(app);
                    }
                } else if app.extraction_selection.primary().head > 0 {
                    // Delete char before cursor
                    let head = app.extraction_selection.primary().head;
                    let state = State {
                        doc: app.extraction_rope.clone(),
                        selection: app.extraction_selection.clone(),
                    };

                    let transaction = Transaction::change(&app.extraction_rope, std::iter::once((head - 1, head, None)));

                    if transaction.apply(&mut app.extraction_rope) {
                        app.extraction_history.commit_revision(&transaction, &state);
                        sync_extraction_state(app);
                    }
                }
            }

            auto_save_notes_if_needed(app)?;
            app.needs_redraw = true;
        }

        (KeyCode::Enter, _) => {
            // Save the current virtual column before creating new line
            let pos = app.extraction_selection.primary().head;
            let line = app.extraction_rope.char_to_line(pos);
            let line_start = app.extraction_rope.line_to_char(line);
            let current_col = pos - line_start;

            // Use current column
            let virtual_col = current_col;

            // Save state before transaction for history
            let state = State {
                doc: app.extraction_rope.clone(),
                selection: app.extraction_selection.clone(),
            };

            // Insert newline plus spaces to reach the virtual column position
            let padding = " ".repeat(virtual_col);
            let new_line_content = format!("\n{}", padding);

            // CORRECT HELIX: Professional newline with Ferrari engine!
            let transaction = Transaction::insert(&app.extraction_rope, &app.extraction_selection, new_line_content.into());

            // Apply transaction (modifies rope in-place)
            if transaction.apply(&mut app.extraction_rope) {
                // Commit to history for undo/redo
                app.extraction_history.commit_revision(&transaction, &state);
                sync_extraction_state(app);
            }

            auto_save_notes_if_needed(app)?;
        }

        // Tab key - insert 5 spaces
        (KeyCode::Tab, _mods) => {
            if app.app_mode == crate::AppMode::NotesEditor {
                let (rope, selection, grid, cursor) = match app.active_pane {
                    crate::ActivePane::Left => {
                        (&mut app.notes_rope, &mut app.notes_selection, &mut app.notes_grid, &mut app.notes_cursor)
                    }
                    crate::ActivePane::Right => {
                        (&mut app.extraction_rope, &mut app.extraction_selection, &mut app.extraction_grid, &mut app.extraction_cursor)
                    }
                };

                // Insert 5 spaces
                let transaction = Transaction::insert(rope, selection, "     ".into());
                let success = transaction.apply(rope);

                if success {
                    *selection = selection.clone().map(transaction.changes());
                    *cursor = GridCursor::from_char_offset(selection.primary().head, grid);
                    *grid = VirtualGrid::new(rope.clone());
                }
            } else {
                // PDF viewer mode - extraction pane only
                let transaction = Transaction::insert(&app.extraction_rope, &app.extraction_selection, "     ".into());
                let success = transaction.apply(&mut app.extraction_rope);

                if success {
                    sync_extraction_state(app);
                }
            }

            auto_save_notes_if_needed(app)?;
            app.needs_redraw = true;
        }

        (KeyCode::Char(c), mods) if !mods.contains(KeyModifiers::CONTROL) && !mods.contains(KeyModifiers::SUPER) => {
            // In Notes mode, work with the appropriate rope, selection, grid, and cursor based on active pane
            if app.app_mode == crate::AppMode::NotesEditor {
                let (rope, selection, grid, cursor, block_sel) = match app.active_pane {
                    crate::ActivePane::Left => {
                        (&mut app.notes_rope, &mut app.notes_selection, &mut app.notes_grid, &mut app.notes_cursor, &mut app.notes_block_selection)
                    }
                    crate::ActivePane::Right => {
                        (&mut app.extraction_rope, &mut app.extraction_selection, &mut app.extraction_grid, &mut app.extraction_cursor, &mut app.extraction_block_selection)
                    }
                };

                // If there's a block selection, delete it first before inserting the character
                if let Some(block_selection) = block_sel.take() {
                    // Delete the block selection
                    let sel = block_selection.to_selection(rope);
                    let delete_transaction = Transaction::change_by_selection(rope, &sel, |range| {
                        (range.from(), range.to(), None)
                    });
                    delete_transaction.apply(rope);

                    // Collapse selection to a single point at the start of where the block was
                    *selection = Selection::point(sel.primary().from());
                    *cursor = GridCursor::from_char_offset(sel.primary().from(), grid);
                    grid.rope = rope.clone();
                }

                // Check if cursor is in virtual space
                if cursor.to_char_offset(grid).is_none() {
                    // We're in virtual space - need to ensure the line exists and pad with spaces
                    grid.ensure_line_length(cursor.row, cursor.col + 1);
                    grid.set_char_at(cursor.col, cursor.row, c);

                    // Update the rope from the grid
                    *rope = grid.rope.clone();

                    // Move cursor right
                    cursor.col += 1;

                    // Update selection to match cursor if we can
                    if let Some(char_pos) = cursor.to_char_offset(grid) {
                        *selection = Selection::point(char_pos);
                    }
                } else {
                    // Normal character insertion - use existing Helix transaction
                    // Save state before transaction for history
                    let _state = State {
                        doc: rope.clone(),
                        selection: selection.clone(),
                    };

                    // CORRECT HELIX: The real Ferrari engine!
                    let transaction = Transaction::insert(rope, selection, c.to_string().into());

                    // Apply transaction (modifies rope in-place)
                    let success = transaction.apply(rope);

                    if success {
                        // Map selection through changes (CRITICAL!)
                        *selection = selection.clone().map(transaction.changes());

                        // Update grid cursor to match new selection position
                        *cursor = GridCursor::from_char_offset(selection.primary().head, grid);

                        // Update the grid with the new rope
                        grid.rope = rope.clone();
                    }
                }

                // Auto-save when editing notes (after borrows are released)
                if app.active_pane == crate::ActivePane::Left {
                    // Update in-memory note
                    save_current_note_changes(app);

                    // Save to database immediately
                    if let Some(ref notes_mode) = app.notes_mode {
                        if let Some(ref current_note) = notes_mode.current_note {
                            let content = app.notes_rope.to_string();
                            if let Some(ref notes_mode) = app.notes_mode {
                                let _ = notes_mode.db.update_note(&current_note.id, current_note.title.clone(), content, current_note.tags.clone());
                            }
                        }
                    }
                }

                app.needs_redraw = true;
            } else {
                // PDF mode - also use grid cursor

                // If there's a block selection, delete it first before inserting the character
                if let Some(block_selection) = app.extraction_block_selection.take() {
                    // Delete the block selection
                    let sel = block_selection.to_selection(&app.extraction_rope);
                    let delete_transaction = Transaction::change_by_selection(&app.extraction_rope, &sel, |range| {
                        (range.from(), range.to(), None)
                    });
                    delete_transaction.apply(&mut app.extraction_rope);

                    // Collapse selection to a single point at the start of where the block was
                    app.extraction_selection = Selection::point(sel.primary().from());
                    app.extraction_cursor = GridCursor::from_char_offset(sel.primary().from(), &app.extraction_grid);
                    app.extraction_grid.rope = app.extraction_rope.clone();
                }

                // Check if cursor is in virtual space
                if app.extraction_cursor.to_char_offset(&app.extraction_grid).is_none() {
                    // We're in virtual space - need to ensure the line exists and pad with spaces
                    app.extraction_grid.ensure_line_length(app.extraction_cursor.row, app.extraction_cursor.col + 1);
                    app.extraction_grid.set_char_at(app.extraction_cursor.col, app.extraction_cursor.row, c);

                    // Update the rope from the grid
                    app.extraction_rope = app.extraction_grid.rope.clone();

                    // Move cursor right
                    app.extraction_cursor.col += 1;

                    // Update selection to match cursor if we can
                    if let Some(char_pos) = app.extraction_cursor.to_char_offset(&app.extraction_grid) {
                        app.extraction_selection = Selection::point(char_pos);
                    }
                } else {
                    // Normal character insertion
                    // Save state before transaction for history
                    let state = State {
                        doc: app.extraction_rope.clone(),
                        selection: app.extraction_selection.clone(),
                    };

                    // CORRECT HELIX: The real Ferrari engine!
                    let transaction = Transaction::insert(&app.extraction_rope, &app.extraction_selection, c.to_string().into());

                    // Apply transaction (modifies rope in-place)
                    let success = transaction.apply(&mut app.extraction_rope);

                    if success {
                        // Map selection through changes (CRITICAL!)
                        app.extraction_selection = app.extraction_selection.clone().map(transaction.changes());

                        // Update grid cursor to match new selection position
                        app.extraction_cursor = GridCursor::from_char_offset(
                            app.extraction_selection.primary().head, &app.extraction_grid
                        );

                        // Update the grid with the new rope
                        app.extraction_grid.rope = app.extraction_rope.clone();

                        // Commit to history for undo/redo
                        app.extraction_history.commit_revision(&transaction, &state);
                    }
                }
            }
        }

        _ => {
            // Unknown key - do nothing
        }
    }

    // Update renderer after any changes
    if let Some(renderer) = &mut app.edit_display {
        renderer.update_from_rope(&app.extraction_rope);
    }

    Ok(true)
}

// Helper to save current note changes back to the list
fn save_current_note_changes(app: &mut App) {
    if let Some(ref notes) = app.notes_mode {
        if let Some(ref current_note) = notes.current_note {
            // Find the current note in the list and update it
            for note in app.notes_list.iter_mut() {
                if note.id == current_note.id {
                    // Update the note's content with the current editor content
                    note.content = app.notes_rope.to_string();
                    break;
                }
            }
        }
    }
}

// HELIX-CORE: Extract selection from rope (handles both regular and block selection)
fn extract_selection_from_rope(app: &App) -> String {
    // In Notes mode, check which pane is active and use the appropriate rope/selection
    let (rope, selection, block_selection) = if app.app_mode == crate::AppMode::NotesEditor {
        match app.active_pane {
            crate::ActivePane::Left => (&app.notes_rope, &app.notes_selection, &app.notes_block_selection),
            crate::ActivePane::Right => (&app.extraction_rope, &app.extraction_selection, &app.extraction_block_selection),
        }
    } else {
        // In PDF mode, always use extraction_rope (right pane shows extraction text)
        (&app.extraction_rope, &app.extraction_selection, &app.extraction_block_selection)
    };

    // First check if we have block selection
    if let Some(block_sel) = block_selection {
        // Convert block selection to regular selection and extract text
        let selection = block_sel.to_selection(rope);
        let mut result = String::new();
        for range in selection.ranges() {
            if range.len() > 0 {
                if !result.is_empty() {
                    result.push('\n');  // Separate lines in block selection
                }
                result.push_str(&rope.slice(range.from()..range.to()).to_string());
            }
        }
        return result;
    }

    // Regular selection
    let range = selection.primary();
    if range.len() > 0 {
        rope.slice(range.from()..range.to()).to_string()
    } else {
        String::new()
    }
}

fn copy_to_clipboard(text: &str) -> Result<()> {
    // Direct macOS pbcopy command for reliable system clipboard
    use std::process::{Command, Stdio};

    let mut child = Command::new("pbcopy")
        .stdin(Stdio::piped())
        .spawn()
        .map_err(|e| anyhow::anyhow!("Failed to spawn pbcopy: {}", e))?;

    if let Some(stdin) = child.stdin.as_mut() {
        stdin.write_all(text.as_bytes())
            .map_err(|e| anyhow::anyhow!("Failed to write to pbcopy: {}", e))?;
    }

    let output = child.wait_with_output()
        .map_err(|e| anyhow::anyhow!("Failed to wait for pbcopy: {}", e))?;

    if !output.status.success() {
        return Err(anyhow::anyhow!("pbcopy failed with status: {}", output.status));
    }

    Ok(())
}

fn paste_from_clipboard() -> Result<String> {
    // Direct macOS pbpaste command for reliable system clipboard
    use std::process::Command;

    let output = Command::new("pbpaste")
        .output()
        .map_err(|e| anyhow::anyhow!("Failed to run pbpaste: {}", e))?;

    if !output.status.success() {
        return Err(anyhow::anyhow!("pbpaste failed with status: {}", output.status));
    }

    let text = String::from_utf8(output.stdout)
        .map_err(|e| anyhow::anyhow!("Invalid UTF-8 from pbpaste: {}", e))?;

    // Filter out ANSI codes and control characters from pasted text
    Ok(crate::text_filter::clean_text_for_insertion(&text))
}