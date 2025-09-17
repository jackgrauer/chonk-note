// MINIMAL KEYBOARD HANDLING
use crate::{App, MOD_KEY};
use anyhow::Result;
use crate::kitty_native::{KeyCode, KeyEvent, KeyModifiers};

// HELIX-CORE INTEGRATION! Professional text editing
use helix_core::{movement, Transaction, Selection, Range, textobject, history::State};

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

        (KeyCode::Char('z'), mods) if mods.contains(KeyModifiers::SUPER) && !mods.contains(KeyModifiers::SHIFT) => {
            // CORRECT HELIX: Undo with proper API!
            if let Some(transaction) = app.history.undo() {
                // Apply undo transaction (in-place)
                let success = transaction.apply(&mut app.rope);

                if success {
                    // Map selection through changes
                    app.selection = app.selection.clone().map(transaction.changes());
                    app.status_message = "Undo".to_string();
                }
            }
        }

        (KeyCode::Char('z'), mods) if mods.contains(KeyModifiers::SUPER) && mods.contains(KeyModifiers::SHIFT) => {
            // CORRECT HELIX: Redo with proper API!
            if let Some(transaction) = app.history.redo() {
                // Apply redo transaction (in-place)
                let success = transaction.apply(&mut app.rope);

                if success {
                    // Map selection through changes
                    app.selection = app.selection.clone().map(transaction.changes());
                    app.status_message = "Redo".to_string();
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

        // BASIC MOVEMENT - Arrow keys (simple implementation)
        (KeyCode::Up, mods) => {
            let pos = app.selection.primary().head;
            let line = app.rope.byte_to_line(pos);
            if line > 0 {
                let new_line = line - 1;
                let line_start = app.rope.line_to_byte(new_line);
                let line_len = app.rope.line(new_line).len_bytes().saturating_sub(1);
                let col = pos - app.rope.line_to_byte(line);
                let new_pos = line_start + col.min(line_len);
                app.selection = Selection::point(new_pos);
            }
        }

        (KeyCode::Down, mods) => {
            let pos = app.selection.primary().head;
            let line = app.rope.byte_to_line(pos);
            if line < app.rope.len_lines() - 1 {
                let new_line = line + 1;
                let line_start = app.rope.line_to_byte(new_line);
                let line_len = app.rope.line(new_line).len_bytes().saturating_sub(1);
                let col = pos - app.rope.line_to_byte(line);
                let new_pos = line_start + col.min(line_len);
                app.selection = Selection::point(new_pos);
            }
        }

        (KeyCode::Left, mods) if !mods.contains(KeyModifiers::SUPER) && !mods.contains(KeyModifiers::ALT) => {
            let pos = app.selection.primary().head;
            if pos > 0 {
                app.selection = Selection::point(pos - 1);
            }
        }

        (KeyCode::Right, mods) if !mods.contains(KeyModifiers::SUPER) && !mods.contains(KeyModifiers::ALT) => {
            let pos = app.selection.primary().head;
            if pos < app.rope.len_chars() {
                app.selection = Selection::point(pos + 1);
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