// MINIMAL KEYBOARD HANDLING
use crate::App;
use anyhow::Result;
use crate::kitty_native::{KeyCode, KeyEvent, KeyModifiers};
use std::fs::OpenOptions;
use std::io::Write;
use std::time::{Duration, Instant};

// HELIX-CORE INTEGRATION! Professional text editing
use helix_core::{Transaction, Selection, history::State, movement};


pub async fn handle_input(app: &mut App, key: KeyEvent) -> Result<bool> {
    // Try dual-pane handler first for Notes mode
    if app.app_mode == crate::AppMode::NotesEditor {
        // Check if dual-pane handler can handle this key
        if let Ok(handled) = crate::dual_pane_keyboard::handle_dual_pane_input(app, &key) {
            if handled {
                return Ok(true);
            }
        }
    }

    let rope = app.rope.slice(..);

    // Handle notes mode specific commands
    if app.app_mode == crate::AppMode::NotesEditor {
        if let Some(ref mut notes) = app.notes_mode {
            match (key.code, key.modifiers) {
                // Ctrl+N - Create new note (in notes mode)
                // Always works with the notes pane (left pane) regardless of which is active
                (KeyCode::Char('n'), mods) if mods.contains(KeyModifiers::CONTROL) && !mods.contains(KeyModifiers::SHIFT) => {
                    // Debug log
                    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open("/Users/jack/chonker7_debug.log") {
                        writeln!(file, "[KEYBOARD] Ctrl+N detected in Notes mode").ok();
                        writeln!(file, "  Before: notes_rope len = {}, selection = {:?}",
                            app.notes_rope.len_chars(), app.notes_selection.primary()).ok();
                    }

                    if let Some(msg) = notes.handle_command(&mut app.notes_rope, &mut app.notes_selection, "new")? {
                        app.status_message = msg;
                    } else {
                        app.status_message = "New note created".to_string();
                    }

                    // Debug log after
                    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open("/Users/jack/chonker7_debug.log") {
                        writeln!(file, "  After: notes_rope len = {}, selection = {:?}",
                            app.notes_rope.len_chars(), app.notes_selection.primary()).ok();
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
                // Ctrl+S - Save note
                (KeyCode::Char('s'), mods) if mods.contains(KeyModifiers::CONTROL) => {
                    if let Some(msg) = notes.handle_command(&mut app.notes_rope, &mut app.notes_selection, "save")? {
                        app.status_message = msg;
                    }
                    return Ok(true);
                }
                // Ctrl+L - List notes
                (KeyCode::Char('l'), mods) if mods.contains(KeyModifiers::CONTROL) => {
                    if let Some(msg) = notes.handle_command(&mut app.notes_rope, &mut app.notes_selection, "list")? {
                        app.status_message = msg;
                    }
                    // Switch to notes pane to see the list
                    app.switch_active_pane(crate::ActivePane::Left);
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
            let pos = app.selection.primary().head;
            let line = rope.char_to_line(pos);
            let line_start = rope.line_to_char(line);
            app.selection = Selection::point(line_start);
        }

        (KeyCode::Right, mods) if mods.contains(KeyModifiers::SUPER) => {
            // Cmd+Right = move to line end using Helix Rope API
            let pos = app.selection.primary().head;
            let line = rope.char_to_line(pos);
            let line_start = rope.line_to_char(line);
            let line_len = rope.line(line).len_chars();
            let line_end = line_start + line_len.saturating_sub(1);
            app.selection = Selection::point(line_end);
        }

        // Option+Left/Right = word by word using proper Helix movement
        (KeyCode::Left, mods) if mods.contains(KeyModifiers::ALT) => {
            // Option+Left = move to previous word using Helix movement
            let range = app.selection.primary();
            let new_pos = movement::move_prev_word_start(rope.slice(..), range, 1);
            app.selection = Selection::single(new_pos.anchor, new_pos.head);
        }

        (KeyCode::Right, mods) if mods.contains(KeyModifiers::ALT) => {
            // Option+Right = move to next word using Helix movement
            let range = app.selection.primary();
            let new_pos = movement::move_next_word_end(rope.slice(..), range, 1);
            app.selection = Selection::single(new_pos.anchor, new_pos.head);
        }

        // Cmd+Up/Down = document start/end
        (KeyCode::Up, mods) if mods.contains(KeyModifiers::SUPER) => {
            // Cmd+Up = move to document start
            app.selection = Selection::point(0);
        }

        (KeyCode::Down, mods) if mods.contains(KeyModifiers::SUPER) => {
            // Cmd+Down = move to document end
            app.selection = Selection::point(rope.len_chars());
        }

        // DELETION - macOS style with proper Helix word boundaries
        // Option+Backspace = delete word
        (KeyCode::Backspace, mods) if mods.contains(KeyModifiers::ALT) => {
            let range = app.selection.primary();
            if range.head > 0 {
                // Save state before transaction for history
                let state = State {
                    doc: app.rope.clone(),
                    selection: app.selection.clone(),
                };

                // Use Helix movement to find the previous word boundary
                let word_start_range = movement::move_prev_word_start(rope, range, 1);
                let start = word_start_range.head;
                let end = range.head;

                // Create transaction to delete the word
                let transaction = Transaction::delete(&app.rope, std::iter::once((start, end)));

                // Apply transaction
                let success = transaction.apply(&mut app.rope);

                if success {
                    app.selection = Selection::point(start);

                    // Commit to history for undo/redo
                    app.history.commit_revision(&transaction, &state);
                }
            }
        }

        // Cmd+Backspace = delete to line start
        (KeyCode::Backspace, mods) if mods.contains(KeyModifiers::SUPER) => {
            let pos = app.selection.primary().head;
            let line = app.rope.char_to_line(pos);
            let line_start = app.rope.line_to_char(line);
            if pos > line_start {
                // Save state before transaction for history
                let state = State {
                    doc: app.rope.clone(),
                    selection: app.selection.clone(),
                };

                let transaction = Transaction::delete(&app.rope, std::iter::once((line_start, pos)));

                // Apply transaction
                let success = transaction.apply(&mut app.rope);

                if success {
                    app.selection = Selection::point(line_start);

                    // Commit to history for undo/redo
                    app.history.commit_revision(&transaction, &state);
                }
            }
        }

        // TEXT EDITING - macOS standard
        (KeyCode::Char('a'), mods) if mods.contains(KeyModifiers::SUPER) => {
            // Select All
            app.selection = Selection::single(0, rope.len_chars());
        }

        (KeyCode::Char('x'), mods) if mods.contains(KeyModifiers::SUPER) => {
            // Cut - copy to clipboard then delete selection
            let text = extract_selection_from_rope(app);
            if !text.is_empty() {
                copy_to_clipboard(&text)?;

                // Save state before deletion for history
                let state = State {
                    doc: app.rope.clone(),
                    selection: app.selection.clone(),
                };

                // Delete the selected text
                let transaction = Transaction::delete(&app.rope, app.selection.ranges().into_iter().map(|r| (r.from(), r.to())));

                // Apply transaction
                let success = transaction.apply(&mut app.rope);

                if success {
                    // Map selection through changes
                    app.selection = app.selection.clone().map(transaction.changes());

                    // Commit to history for undo/redo
                    app.history.commit_revision(&transaction, &state);
                    app.status_message = "Cut".to_string();
                }
            }
        }

        (KeyCode::Char('c'), mods) if mods.contains(KeyModifiers::SUPER) => {
            // Copy
            let text = extract_selection_from_rope(app);
            if !text.is_empty() {
                copy_to_clipboard(&text)?;
                app.status_message = "Copied".to_string();
            }
        }

        // On macOS, Cmd key is being reported as CONTROL by Kitty
        (KeyCode::Char('z'), mods) if mods.contains(KeyModifiers::CONTROL) && !mods.contains(KeyModifiers::SHIFT) => {
            // Debug to file
            if let Ok(mut file) = OpenOptions::new().create(true).append(true).open("/Users/jack/chonker7_debug.log") {
                writeln!(file, "[UNDO] History rev: {}, at_root: {}",
                    app.history.current_revision(),
                    app.history.at_root()).ok();
            }

            // CORRECT HELIX: Undo with proper API!
            if let Some(transaction) = app.history.undo() {
                // Clone the transaction since we get a reference from history
                let transaction = transaction.clone();
                // Apply undo transaction (in-place)
                let success = transaction.apply(&mut app.rope);

                if success {
                    // Map selection through changes
                    app.selection = app.selection.clone().map(transaction.changes());
                    app.status_message = "Undo".to_string();

                    // CRITICAL: Trigger redraw after undo!
                    app.needs_redraw = true;

                    // Update the edit display renderer
                    if let Some(renderer) = &mut app.edit_display {
                        renderer.update_from_rope(&app.rope);
                    }

                    // Debug to file
                    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open("/Users/jack/chonker7_debug.log") {
                        writeln!(file, "[UNDO] Success! New rev: {}", app.history.current_revision()).ok();
                    }
                } else {
                    app.status_message = "Undo failed".to_string();
                    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open("/Users/jack/chonker7_debug.log") {
                        writeln!(file, "[UNDO] Failed to apply transaction").ok();
                    }
                }
            } else {
                app.status_message = "Nothing to undo".to_string();
                if let Ok(mut file) = OpenOptions::new().create(true).append(true).open("/Users/jack/chonker7_debug.log") {
                    writeln!(file, "[UNDO] No transaction available (at root)").ok();
                }
            }
        }

        // On macOS, Cmd key is being reported as CONTROL by Kitty
        (KeyCode::Char('z'), mods) if mods.contains(KeyModifiers::CONTROL) && mods.contains(KeyModifiers::SHIFT) => {
            // Debug to file
            if let Ok(mut file) = OpenOptions::new().create(true).append(true).open("/Users/jack/chonker7_debug.log") {
                writeln!(file, "[REDO] History rev: {}", app.history.current_revision()).ok();
            }

            // CORRECT HELIX: Redo with proper API!
            if let Some(transaction) = app.history.redo() {
                // Clone the transaction since we get a reference from history
                let transaction = transaction.clone();
                // Apply redo transaction (in-place)
                let success = transaction.apply(&mut app.rope);

                if success {
                    // Map selection through changes
                    app.selection = app.selection.clone().map(transaction.changes());
                    app.status_message = "Redo".to_string();

                    // CRITICAL: Trigger redraw after redo!
                    app.needs_redraw = true;

                    // Update the edit display renderer
                    if let Some(renderer) = &mut app.edit_display {
                        renderer.update_from_rope(&app.rope);
                    }

                    // Debug to file
                    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open("/Users/jack/chonker7_debug.log") {
                        writeln!(file, "[REDO] Success! New rev: {}", app.history.current_revision()).ok();
                    }
                } else {
                    app.status_message = "Redo failed".to_string();
                    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open("/Users/jack/chonker7_debug.log") {
                        writeln!(file, "[REDO] Failed to apply transaction").ok();
                    }
                }
            } else {
                app.status_message = "Nothing to redo".to_string();
                if let Ok(mut file) = OpenOptions::new().create(true).append(true).open("/Users/jack/chonker7_debug.log") {
                    writeln!(file, "[REDO] No transaction available").ok();
                }
            }
        }

        (KeyCode::Char('v'), mods) if mods.contains(KeyModifiers::SUPER) => {
            // FULL HELIX: Professional paste with transactions
            if let Ok(text) = paste_from_clipboard() {
                // Save state before transaction for history
                let state = State {
                    doc: app.rope.clone(),
                    selection: app.selection.clone(),
                };

                // CORRECT HELIX: Paste with Ferrari engine!
                let transaction = Transaction::insert(&app.rope, &app.selection, text.into());

                // Apply and get new rope
                let success = transaction.apply(&mut app.rope);

                if success {
                    // Map selection through changes
                    app.selection = app.selection.clone().map(transaction.changes());

                    // Commit to history for undo/redo
                    app.history.commit_revision(&transaction, &state);
                    app.status_message = "Pasted".to_string();
                }
            }
        }

        // Select All - Ctrl+A
        (KeyCode::Char('a'), mods) if mods.contains(KeyModifiers::CONTROL) => {
            // Debug to file
            if let Ok(mut file) = OpenOptions::new().create(true).append(true).open("/Users/jack/chonker7_debug.log") {
                writeln!(file, "[KEYBOARD] Ctrl+A - Select All triggered").ok();
            }

            // Select entire document
            let doc_len = app.rope.len_chars();
            if doc_len > 0 {
                // Create a selection from start to end of document
                app.selection = Selection::single(0, doc_len);

                // Clear block selection since we're doing regular selection
                app.block_selection = None;

                app.needs_redraw = true;
                app.status_message = "Selected all".to_string();

                if let Ok(mut file) = OpenOptions::new().create(true).append(true).open("/Users/jack/chonker7_debug.log") {
                    writeln!(file, "[KEYBOARD] Selected entire document: 0 to {} chars", doc_len).ok();
                }
            }
        }

        // Cut - Ctrl+X (in addition to Cmd+X)
        (KeyCode::Char('x'), mods) if mods.contains(KeyModifiers::CONTROL) => {
            // Debug to file
            if let Ok(mut file) = OpenOptions::new().create(true).append(true).open("/Users/jack/chonker7_debug.log") {
                writeln!(file, "[KEYBOARD] Ctrl+X - Cut triggered").ok();
            }

            // Extract selected text
            let text = extract_selection_from_rope(app);
            if !text.is_empty() {
                // Copy to clipboard
                copy_to_clipboard(&text)?;

                // In Notes mode, work with the appropriate rope and selection based on active pane
                if app.app_mode == crate::AppMode::NotesEditor {
                    let (rope, selection) = match app.active_pane {
                        crate::ActivePane::Left => {
                            (&mut app.notes_rope, &mut app.notes_selection)
                        }
                        crate::ActivePane::Right => {
                            (&mut app.extraction_rope, &mut app.extraction_selection)
                        }
                    };

                    // Save state before deletion for history
                    let state = State {
                        doc: rope.clone(),
                        selection: selection.clone(),
                    };

                    // Get the selection to use for deletion
                    let sel_for_deletion = selection.clone();

                    // Delete the selected text using Transaction
                    let transaction = Transaction::change_by_selection(&rope, &selection, |range| {
                        (range.from(), range.to(), None)
                    });

                    // Apply transaction
                    transaction.apply(rope);

                    // Update selection to collapsed position at deletion point
                    let new_pos = sel_for_deletion.primary().from();
                    *selection = Selection::point(new_pos);

                    // Block selection not supported in dual-pane mode yet

                    // Note: History would need separate tracking per rope
                    // For now, skip history in Notes mode

                } else {
                    // Original PDF mode logic
                    // Save state before deletion for history
                    let state = State {
                        doc: app.rope.clone(),
                        selection: app.selection.clone(),
                    };

                    // Get the selection to use for deletion (block or regular)
                    let selection = if let Some(block_sel) = &app.block_selection {
                        block_sel.to_selection(&app.rope)
                    } else {
                        app.selection.clone()
                    };

                    // Delete the selected text using Transaction
                    let transaction = if let Some(block_sel) = &app.block_selection {
                        let selection = block_sel.to_selection(&app.rope);
                        Transaction::change_by_selection(&app.rope, &selection, |range| {
                            let start = range.from();
                            let end = range.to();
                            let text = app.rope.slice(start..end);
                            let mut replacement = String::new();
                            for ch in text.chars() {
                                if ch == '\n' || ch == '\r' {
                                    replacement.push(ch);
                                } else {
                                    replacement.push(' ');
                                }
                            }
                            (start, end, Some(replacement.into()))
                        })
                    } else {
                        Transaction::change_by_selection(&app.rope, &selection, |range| {
                            (range.from(), range.to(), None)
                        })
                    };

                    // Apply transaction
                    transaction.apply(&mut app.rope);

                    // Update selection to collapsed position at deletion point
                    let new_pos = selection.primary().from();
                    app.selection = Selection::point(new_pos);

                    // Clear any block selection
                    app.block_selection = None;

                    // Add to history for undo support
                    app.history.commit_revision(&transaction, &state);
                }

                // Force re-render
                app.needs_redraw = true;
                app.status_message = format!("Cut {} characters", text.len());

                if let Ok(mut file) = OpenOptions::new().create(true).append(true).open("/Users/jack/chonker7_debug.log") {
                    writeln!(file, "[KEYBOARD] Cut {} characters to clipboard: {:?}", text.len(), &text[..text.len().min(50)]).ok();
                }
            } else {
                app.status_message = "Nothing to cut".to_string();
                app.needs_redraw = true;
            }
        }

        // Copy - Ctrl+C
        (KeyCode::Char('c'), mods) if mods.contains(KeyModifiers::CONTROL) => {
            // Debug to file
            if let Ok(mut file) = OpenOptions::new().create(true).append(true).open("/Users/jack/chonker7_debug.log") {
                writeln!(file, "[KEYBOARD] Ctrl+C - Copy triggered").ok();
            }

            // Copy selected text to clipboard
            let text = extract_selection_from_rope(app);
            if !text.is_empty() {
                copy_to_clipboard(&text)?;
                app.status_message = format!("Copied {} characters", text.len());
                app.needs_redraw = true;  // Force redraw to show status

                if let Ok(mut file) = OpenOptions::new().create(true).append(true).open("/Users/jack/chonker7_debug.log") {
                    writeln!(file, "[KEYBOARD] Copied {} characters to clipboard: {:?}", text.len(), &text[..text.len().min(50)]).ok();
                }
            } else {
                app.status_message = "Nothing to copy".to_string();
                app.needs_redraw = true;
            }
        }

        // Paste - Ctrl+V
        (KeyCode::Char('v'), mods) if mods.contains(KeyModifiers::CONTROL) => {
            // Debug to file
            if let Ok(mut file) = OpenOptions::new().create(true).append(true).open("/Users/jack/chonker7_debug.log") {
                writeln!(file, "[KEYBOARD] Ctrl+V - Paste triggered").ok();
            }

            if let Ok(text) = paste_from_clipboard() {
                // In Notes mode, work with the appropriate rope and selection based on active pane
                if app.app_mode == crate::AppMode::NotesEditor {
                    let (rope, selection) = match app.active_pane {
                        crate::ActivePane::Left => {
                            (&mut app.notes_rope, &mut app.notes_selection)
                        }
                        crate::ActivePane::Right => {
                            (&mut app.extraction_rope, &mut app.extraction_selection)
                        }
                    };

                    // Save state before transaction for history
                    let state = State {
                        doc: rope.clone(),
                        selection: selection.clone(),
                    };

                    // Get the selection to use for paste
                    let sel_for_paste = selection.clone();

                    // Create paste transaction
                    let transaction = Transaction::change_by_selection(&rope, &sel_for_paste, |range| {
                        (range.from(), range.to(), Some(text.clone().into()))
                    });

                    // Apply and get new rope
                    transaction.apply(rope);

                    // Move cursor to end of pasted text
                    let paste_end = sel_for_paste.primary().from() + text.len();
                    *selection = Selection::point(paste_end);

                    // Block selection not supported in dual-pane mode yet

                    // Note: History would need separate tracking per rope
                    // For now, skip history in Notes mode

                } else {
                    // Original PDF mode logic
                    // Save state before transaction for history
                    let state = State {
                        doc: app.rope.clone(),
                        selection: app.selection.clone(),
                    };

                    // Get the selection to use for paste (block or regular)
                    let selection = if let Some(block_sel) = &app.block_selection {
                        block_sel.to_selection(&app.rope)
                    } else {
                        app.selection.clone()
                    };

                    // Create paste transaction
                    let transaction = Transaction::change_by_selection(&app.rope, &selection, |range| {
                        (range.from(), range.to(), Some(text.clone().into()))
                    });

                    // Apply and get new rope
                    transaction.apply(&mut app.rope);

                    // Move cursor to end of pasted text
                    let paste_end = selection.primary().from() + text.len();
                    app.selection = Selection::point(paste_end);

                    // Clear any block selection
                    app.block_selection = None;

                    // Add to history
                    app.history.commit_revision(&transaction, &state);
                }

                // Force update
                app.needs_redraw = true;
                app.status_message = "Pasted".to_string();

                if let Ok(mut file) = OpenOptions::new().create(true).append(true).open("/Users/jack/chonker7_debug.log") {
                    writeln!(file, "[KEYBOARD] Pasted {} characters from clipboard", text.len()).ok();
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

        // BASIC MOVEMENT - Arrow keys (no acceleration)
        (KeyCode::Up, mods) => {
            // Get the correct rope and selection based on mode and active pane
            let (rope, selection) = if app.app_mode == crate::AppMode::NotesEditor {
                match app.active_pane {
                    crate::ActivePane::Left => (&mut app.notes_rope, &mut app.notes_selection),
                    crate::ActivePane::Right => (&mut app.extraction_rope, &mut app.extraction_selection),
                }
            } else {
                (&mut app.rope, &mut app.selection)
            };

            // Clear any block selection when using arrow keys
            app.block_selection = None;

            let pos = selection.primary().head;
            let line = rope.char_to_line(pos);
            let lines_to_move = 1.min(line);  // Don't go past start

            if lines_to_move > 0 {
                // Preserve virtual column if set, otherwise calculate from current position
                let virtual_col = if let Some(vc) = app.virtual_cursor_col {
                    vc
                } else {
                    let line_start = rope.line_to_char(line);
                    pos - line_start
                };

                let new_line = line - lines_to_move;
                let line_start = rope.line_to_char(new_line);
                let line_len = rope.line(new_line).len_chars().saturating_sub(1);

                // Position cursor at virtual column, allowing it to go past line end
                // The selection will be at the actual line position, but we remember the virtual column
                let new_pos = line_start + virtual_col.min(line_len);

                if mods.contains(KeyModifiers::SHIFT) {
                    // Extend selection - keep anchor, move head
                    let anchor = selection.primary().anchor;
                    *selection = Selection::single(anchor, new_pos);
                } else {
                    // Just move cursor
                    *selection = Selection::point(new_pos);
                }

                // Clear virtual column when moving up/down - let Right arrow recalculate it
                app.virtual_cursor_col = None;
            }
        }

        (KeyCode::Down, mods) => {
            // Clear any block selection when using arrow keys
            app.block_selection = None;

            let pos = app.selection.primary().head;
            let line = app.rope.char_to_line(pos);
            let max_line = app.rope.len_lines() - 1;
            let lines_to_move = 1.min(max_line - line);  // Don't go past end

            if lines_to_move > 0 {
                // Preserve virtual column if set, otherwise calculate from current position
                let virtual_col = if let Some(vc) = app.virtual_cursor_col {
                    vc
                } else {
                    let line_start = app.rope.line_to_char(line);
                    pos - line_start
                };

                let new_line = line + lines_to_move;
                let line_start = app.rope.line_to_char(new_line);
                let line_len = app.rope.line(new_line).len_chars().saturating_sub(1);

                // Position cursor at virtual column, allowing it to go past line end
                // The selection will be at the actual line position, but we remember the virtual column
                let new_pos = line_start + virtual_col.min(line_len);

                if mods.contains(KeyModifiers::SHIFT) {
                    // Extend selection - keep anchor, move head
                    let anchor = app.selection.primary().anchor;
                    app.selection = Selection::single(anchor, new_pos);
                } else {
                    // Just move cursor
                    app.selection = Selection::point(new_pos);
                }

                // Clear virtual column when moving up/down - let Right arrow recalculate it
                app.virtual_cursor_col = None;
            }
        }

        (KeyCode::Left, mods) if !mods.contains(KeyModifiers::SUPER) && !mods.contains(KeyModifiers::ALT) => {
            // Clear any block selection when using arrow keys
            app.block_selection = None;

            let pos = app.selection.primary().head;
            let line = app.rope.char_to_line(pos);
            let line_start = app.rope.line_to_char(line);
            let col = pos - line_start;

            // BOUNDARY CHECK: Don't go past the start of the current line
            // This enforces the left boundary on every row, not just row 0
            let max_movement = col.min(1);

            if max_movement > 0 {
                let new_pos = pos - max_movement;

                // Update virtual cursor column based on new position
                let new_col = col - max_movement;
                app.virtual_cursor_col = Some(new_col);

                if mods.contains(KeyModifiers::SHIFT) {
                    // Extend selection - keep anchor, move head
                    let anchor = app.selection.primary().anchor;
                    app.selection = Selection::single(anchor, new_pos);
                } else {
                    // Just move cursor
                    app.selection = Selection::point(new_pos);
                }
            }
        }

        (KeyCode::Right, mods) if !mods.contains(KeyModifiers::SUPER) && !mods.contains(KeyModifiers::ALT) => {
            // Clear any block selection when using arrow keys
            app.block_selection = None;

            let pos = app.selection.primary().head;
            let line = app.rope.char_to_line(pos);
            let line_start = app.rope.line_to_char(line);
            let current_col = pos - line_start;

            // Get the maximum valid position in the document
            let max_pos = app.rope.len_chars().saturating_sub(1);

            // For Right arrow, always use virtual column if set, otherwise current
            // This maintains virtual space position across lines
            let virtual_col = app.virtual_cursor_col.unwrap_or(current_col);

            // Now increment from the virtual position with boundary check
            let new_virtual_col = virtual_col.saturating_add(1);
            app.virtual_cursor_col = Some(new_virtual_col);

            // Get the line length to stay within document bounds
            let line_slice = app.rope.line(line);
            let line_len = line_slice.len_chars().saturating_sub(1); // Exclude newline

            // Document position tracks virtual position when possible, but caps at line end
            // BOUNDARY CHECK: Also ensure we never exceed the document's maximum position
            let new_pos = (line_start + new_virtual_col.min(line_len)).min(max_pos);

            if mods.contains(KeyModifiers::SHIFT) {
                // Extend selection - keep anchor, move head
                let anchor = app.selection.primary().anchor;
                app.selection = Selection::single(anchor, new_pos);
            } else {
                // Just move cursor
                app.selection = Selection::point(new_pos);
            }

            // Force redraw to show virtual cursor movement
            app.needs_redraw = true;
        }

        // TEXT OPERATIONS
        (KeyCode::Backspace, mods) if !mods.contains(KeyModifiers::ALT) && !mods.contains(KeyModifiers::SUPER) => {
            // Special handling: if cursor is on a space/empty and no selection, just move left
            let pos = app.selection.primary().head;
            let is_on_space = if pos < app.rope.len_chars() {
                let ch = app.rope.char(pos);
                ch == ' ' || ch == '\t'
            } else {
                true  // End of document counts as empty
            };

            // If on empty space with no selection, just move left instead of deleting
            if is_on_space && app.selection.primary().len() == 0 && app.block_selection.is_none() {
                // Just move cursor left like pressing left arrow
                if pos > 0 {
                    let new_pos = pos - 1;
                    app.selection = Selection::point(new_pos);

                    // Update virtual cursor column
                    let line = app.rope.char_to_line(new_pos);
                    let line_start = app.rope.line_to_char(line);
                    app.virtual_cursor_col = Some(new_pos - line_start);
                }
                // No deletion happens - just cursor movement
                app.needs_redraw = true;
                return Ok(true);  // Continue running
            }

            // Save state before transaction for history
            let state = State {
                doc: app.rope.clone(),
                selection: app.selection.clone(),
            };

            // Get the selection to use (block or regular)
            let (transaction, clear_block) = if let Some(block_sel) = &app.block_selection {
                // Block selection - replace with spaces to preserve layout
                let selection = block_sel.to_selection(&app.rope);

                // Replace each selected character with a space
                let transaction = Transaction::change_by_selection(&app.rope, &selection, |range| {
                    let start = range.from();
                    let end = range.to();

                    // Get the actual text being replaced
                    let text = app.rope.slice(start..end);
                    let mut replacement = String::new();

                    // Replace each character with a space, preserving line breaks
                    for ch in text.chars() {
                        if ch == '\n' || ch == '\r' {
                            replacement.push(ch);
                        } else {
                            replacement.push(' ');
                        }
                    }

                    (start, end, Some(replacement.into()))
                });
                (transaction, true)
            } else if app.selection.primary().len() > 0 {
                // Regular selection - delete normally
                let transaction = Transaction::delete(&app.rope, app.selection.ranges().into_iter().map(|r| (r.from(), r.to())));
                (transaction, false)
            } else {
                // Delete character before cursor (delete_backward)
                let transaction = Transaction::delete(&app.rope, std::iter::once((
                    app.selection.primary().head.saturating_sub(1),
                    app.selection.primary().head
                )));
                (transaction, false)
            };

            // Apply transaction (modifies rope in-place)
            let success = transaction.apply(&mut app.rope);

            if success {
                // Map selection through changes
                app.selection = app.selection.clone().map(transaction.changes());

                // Clear block selection if we just deleted it
                if clear_block {
                    app.block_selection = None;
                }

                // Commit to history for undo/redo
                app.history.commit_revision(&transaction, &state);
            }
        }

        (KeyCode::Enter, _) => {
            // Save the current virtual column before creating new line
            let pos = app.selection.primary().head;
            let line = app.rope.char_to_line(pos);
            let line_start = app.rope.line_to_char(line);
            let current_col = pos - line_start;

            // Preserve virtual column if set, otherwise use current column
            let virtual_col = app.virtual_cursor_col.unwrap_or(current_col);

            // Save state before transaction for history
            let state = State {
                doc: app.rope.clone(),
                selection: app.selection.clone(),
            };

            // Insert newline plus spaces to reach the virtual column position
            let padding = " ".repeat(virtual_col);
            let new_line_content = format!("\n{}", padding);

            // CORRECT HELIX: Professional newline with Ferrari engine!
            let transaction = Transaction::insert(&app.rope, &app.selection, new_line_content.into());

            // Apply transaction (modifies rope in-place)
            let success = transaction.apply(&mut app.rope);

            if success {
                // Map selection through changes
                app.selection = app.selection.clone().map(transaction.changes());

                // After Enter, we're at the END of the spaces on the new line
                // We need to explicitly set the virtual column to be at that position
                let new_pos = app.selection.primary().head;
                let new_line = app.rope.char_to_line(new_pos);
                let new_line_start = app.rope.line_to_char(new_line);
                let new_col = new_pos - new_line_start;

                // Set virtual column to where we actually are (at the end of spaces)
                app.virtual_cursor_col = Some(new_col);

                // Commit to history for undo/redo
                app.history.commit_revision(&transaction, &state);
            }
        }

        (KeyCode::Char(c), mods) if !mods.contains(KeyModifiers::CONTROL) && !mods.contains(KeyModifiers::SUPER) => {
            // In Notes mode, work with the appropriate rope and selection based on active pane
            if app.app_mode == crate::AppMode::NotesEditor {
                let (rope, selection) = match app.active_pane {
                    crate::ActivePane::Left => {
                        (&mut app.notes_rope, &mut app.notes_selection)
                    }
                    crate::ActivePane::Right => {
                        (&mut app.extraction_rope, &mut app.extraction_selection)
                    }
                };

                // Save state before transaction for history
                let state = State {
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

                    // Note: History would need separate tracking per rope
                    // For now, skip history in Notes mode

                    // Clear virtual column when typing
                    app.virtual_cursor_col = None;
                }
            } else {
                // Original PDF mode logic
                // Save state before transaction for history
                let state = State {
                    doc: app.rope.clone(),
                    selection: app.selection.clone(),
                };

                // CORRECT HELIX: The real Ferrari engine!
                let transaction = Transaction::insert(&app.rope, &app.selection, c.to_string().into());

                // Apply transaction (modifies rope in-place)
                let success = transaction.apply(&mut app.rope);

                if success {
                    // Map selection through changes (CRITICAL!)
                    app.selection = app.selection.clone().map(transaction.changes());

                    // Commit to history for undo/redo
                    app.history.commit_revision(&transaction, &state);

                    // Clear virtual column when typing
                    app.virtual_cursor_col = None;
                }
            }
        }

        _ => {
            // Unknown key - do nothing
        }
    }

    // Update renderer after any changes
    if let Some(renderer) = &mut app.edit_display {
        renderer.update_from_rope(&app.rope);
    }

    Ok(true)
}

// HELIX-CORE: Extract selection from rope (handles both regular and block selection)
fn extract_selection_from_rope(app: &App) -> String {
    // In Notes mode, check which pane is active and use the appropriate rope/selection
    let (rope, selection, block_selection) = if app.app_mode == crate::AppMode::NotesEditor {
        match app.active_pane {
            crate::ActivePane::Left => (&app.notes_rope, &app.notes_selection, &None),
            crate::ActivePane::Right => (&app.extraction_rope, &app.extraction_selection, &None),
        }
    } else {
        // In PDF mode, always use the main rope/selection (extraction text)
        (&app.rope, &app.selection, &app.block_selection)
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
    use std::io::Write;

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

    String::from_utf8(output.stdout)
        .map_err(|e| anyhow::anyhow!("Invalid UTF-8 from pbpaste: {}", e))
}