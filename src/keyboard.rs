// MINIMAL KEYBOARD HANDLING
use crate::{App, MOD_KEY};
use anyhow::Result;
use crate::kitty_native::{KeyCode, KeyEvent, KeyModifiers};
use std::fs::OpenOptions;
use std::io::Write;
use std::time::{Duration, Instant};

// HELIX-CORE INTEGRATION! Professional text editing
use helix_core::{movement, Transaction, Selection, Range, textobject, history::State};

// Arrow key acceleration helper
fn update_arrow_acceleration(app: &mut App, key: KeyCode) -> usize {
    let now = Instant::now();

    // Check if it's the same arrow key being held
    if let Some(last_key) = app.last_arrow_key {
        if last_key == key {
            // Check if it's within the acceleration window (300ms)
            if let Some(last_time) = app.last_arrow_time {
                if now.duration_since(last_time) < Duration::from_millis(300) {
                    app.arrow_key_count += 1;
                } else {
                    // Reset if too much time has passed
                    app.arrow_key_count = 1;
                }
            }
        } else {
            // Different arrow key, reset counter
            app.arrow_key_count = 1;
        }
    } else {
        // First arrow press
        app.arrow_key_count = 1;
    }

    app.last_arrow_key = Some(key);
    app.last_arrow_time = Some(now);

    // Calculate acceleration based on key count
    match app.arrow_key_count {
        1..=3 => 1,           // Normal speed for first few presses
        4..=8 => 3,           // 3x speed after holding briefly
        9..=15 => 6,          // 6x speed for sustained holding
        _ => 10,              // Max 10x speed for long holds
    }
}

pub async fn handle_input(app: &mut App, key: KeyEvent) -> Result<bool> {
    let rope = app.rope.slice(..);

    // macOS-NATIVE KEYBOARD SHORTCUTS!
    match (key.code, key.modifiers) {
        // NAVIGATION - macOS style
        // Cmd+Left/Right = beginning/end of line
        (KeyCode::Left, mods) if mods.contains(KeyModifiers::SUPER) => {
            // TODO: Implement Cmd+Left = line start with proper helix API
            let pos = app.selection.primary().head;
            let line = app.rope.byte_to_line(pos);
            let line_start = app.rope.line_to_byte(line);
            app.selection = Selection::point(line_start);
        }

        (KeyCode::Right, mods) if mods.contains(KeyModifiers::SUPER) => {
            // Cmd+Right = move to line end
            let pos = app.selection.primary().head;
            let line = app.rope.byte_to_line(pos);
            let line_start = app.rope.line_to_byte(line);
            let line_end = line_start + app.rope.line(line).len_bytes().saturating_sub(1);
            app.selection = Selection::point(line_end);
        }

        // Option+Left/Right = word by word (simplified for now)
        (KeyCode::Left, mods) if mods.contains(KeyModifiers::ALT) => {
            // Option+Left = move to previous word (basic implementation)
            let pos = app.selection.primary().head;
            let text = rope.to_string();
            let mut new_pos = pos;

            // Find previous word boundary
            let chars: Vec<char> = text.chars().collect();
            if let Some(mut i) = pos.checked_sub(1) {
                // Skip current whitespace
                while i > 0 && chars.get(i).map_or(false, |c| c.is_whitespace()) {
                    i -= 1;
                }
                // Skip current word
                while i > 0 && chars.get(i).map_or(false, |c| !c.is_whitespace()) {
                    i -= 1;
                }
                new_pos = i;
            }
            app.selection = Selection::point(new_pos);
        }

        (KeyCode::Right, mods) if mods.contains(KeyModifiers::ALT) => {
            // Option+Right = move to next word (basic implementation)
            let pos = app.selection.primary().head;
            let text = rope.to_string();
            let mut new_pos = pos;

            // Find next word boundary
            let chars: Vec<char> = text.chars().collect();
            if pos < chars.len() {
                let mut i = pos;
                // Skip current word
                while i < chars.len() && chars.get(i).map_or(false, |c| !c.is_whitespace()) {
                    i += 1;
                }
                // Skip whitespace
                while i < chars.len() && chars.get(i).map_or(false, |c| c.is_whitespace()) {
                    i += 1;
                }
                new_pos = i;
            }
            app.selection = Selection::point(new_pos);
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

        // DELETION - macOS style
        // Option+Backspace = delete word (simplified)
        (KeyCode::Backspace, mods) if mods.contains(KeyModifiers::ALT) => {
            let pos = app.selection.primary().head;
            if pos > 0 {
                // Save state before transaction for history
                let state = State {
                    doc: app.rope.clone(),
                    selection: app.selection.clone(),
                };

                // For now, delete 5 characters (basic word approximation)
                let start = pos.saturating_sub(5);
                let transaction = Transaction::delete(&app.rope, std::iter::once((start, pos)));

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
            let line = app.rope.byte_to_line(pos);
            let line_start = app.rope.line_to_byte(line);
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

        (KeyCode::Char('n'), mods) if mods.contains(KeyModifiers::CONTROL) => {
            app.next_page();
            if app.current_page_image.is_none() {
                app.load_pdf_page().await?;
            }
        }

        (KeyCode::Char('p'), mods) if mods.contains(KeyModifiers::CONTROL) => {
            app.prev_page();
            if app.current_page_image.is_none() {
                app.load_pdf_page().await?;
            }
        }

        // BASIC MOVEMENT - Arrow keys with acceleration
        (KeyCode::Up, _mods) => {
            // Update acceleration state
            let accel = update_arrow_acceleration(app, KeyCode::Up);

            let pos = app.selection.primary().head;
            let line = app.rope.byte_to_line(pos);
            let lines_to_move = accel.min(line);  // Don't go past start

            if lines_to_move > 0 {
                let new_line = line - lines_to_move;
                let line_start = app.rope.line_to_byte(new_line);
                let line_len = app.rope.line(new_line).len_bytes().saturating_sub(1);
                let col = pos - app.rope.line_to_byte(line);
                let new_pos = line_start + col.min(line_len);
                app.selection = Selection::point(new_pos);
            }
        }

        (KeyCode::Down, _mods) => {
            // Update acceleration state
            let accel = update_arrow_acceleration(app, KeyCode::Down);

            let pos = app.selection.primary().head;
            let line = app.rope.byte_to_line(pos);
            let max_line = app.rope.len_lines() - 1;
            let lines_to_move = accel.min(max_line - line);  // Don't go past end

            if lines_to_move > 0 {
                let new_line = line + lines_to_move;
                let line_start = app.rope.line_to_byte(new_line);
                let line_len = app.rope.line(new_line).len_bytes().saturating_sub(1);
                let col = pos - app.rope.line_to_byte(line);
                let new_pos = line_start + col.min(line_len);
                app.selection = Selection::point(new_pos);
            }
        }

        (KeyCode::Left, mods) if !mods.contains(KeyModifiers::SUPER) && !mods.contains(KeyModifiers::ALT) => {
            // Update acceleration state
            let accel = update_arrow_acceleration(app, KeyCode::Left);

            let pos = app.selection.primary().head;
            let chars_to_move = accel.min(pos);  // Don't go past start
            if chars_to_move > 0 {
                app.selection = Selection::point(pos - chars_to_move);
            }
        }

        (KeyCode::Right, mods) if !mods.contains(KeyModifiers::SUPER) && !mods.contains(KeyModifiers::ALT) => {
            // Update acceleration state
            let accel = update_arrow_acceleration(app, KeyCode::Right);

            let pos = app.selection.primary().head;
            let max_pos = app.rope.len_chars();
            let chars_to_move = accel.min(max_pos - pos);  // Don't go past end
            if chars_to_move > 0 {
                app.selection = Selection::point(pos + chars_to_move);
            }
        }

        // TEXT OPERATIONS
        (KeyCode::Backspace, mods) if !mods.contains(KeyModifiers::ALT) && !mods.contains(KeyModifiers::SUPER) => {
            // Save state before transaction for history
            let state = State {
                doc: app.rope.clone(),
                selection: app.selection.clone(),
            };

            // CORRECT HELIX: Professional backspace with Ferrari engine!
            let transaction = if app.selection.len() > 1 {
                // Delete selection
                Transaction::delete(&app.rope, app.selection.ranges().into_iter().map(|r| (r.from(), r.to())))
            } else {
                // Delete character before cursor (delete_backward)
                Transaction::delete(&app.rope, std::iter::once((
                    app.selection.primary().head.saturating_sub(1),
                    app.selection.primary().head
                )))
            };

            // Apply transaction (modifies rope in-place)
            let success = transaction.apply(&mut app.rope);

            if success {
                // Map selection through changes
                app.selection = app.selection.clone().map(transaction.changes());

                // Commit to history for undo/redo
                app.history.commit_revision(&transaction, &state);
            }
        }

        (KeyCode::Enter, _) => {
            // Save state before transaction for history
            let state = State {
                doc: app.rope.clone(),
                selection: app.selection.clone(),
            };

            // CORRECT HELIX: Professional newline with Ferrari engine!
            let transaction = Transaction::insert(&app.rope, &app.selection, "\n".into());

            // Apply transaction (modifies rope in-place)
            let success = transaction.apply(&mut app.rope);

            if success {
                // Map selection through changes
                app.selection = app.selection.clone().map(transaction.changes());

                // Commit to history for undo/redo
                app.history.commit_revision(&transaction, &state);
            }
        }

        (KeyCode::Char(c), mods) if !mods.contains(KeyModifiers::CONTROL) && !mods.contains(KeyModifiers::SUPER) => {
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

// HELIX-CORE: Extract selection from rope (much simpler!)
fn extract_selection_from_rope(app: &App) -> String {
    if app.selection.len() > 1 {
        let range = app.selection.primary();
        app.rope.slice(range.from()..range.to()).to_string()
    } else {
        String::new()
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