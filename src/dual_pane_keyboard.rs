// Simplified dual-pane keyboard handling for Notes mode
use crate::{App, AppMode, ActivePane};
use anyhow::Result;
use crate::kitty_native::{KeyCode, KeyEvent, KeyModifiers};
use helix_core::{Transaction, Selection, history::State};
use crate::text_filter;

// Handle keyboard input for dual-pane Notes mode
// Returns true if the key was handled, false otherwise
pub fn handle_dual_pane_input(app: &mut App, key: &KeyEvent) -> Result<bool> {
    // Only handle if we're in Notes mode
    if app.app_mode != AppMode::NotesEditor {
        return Ok(false);
    }

    // Get the active rope, selection, grid cursor, and history
    let (rope, selection, grid_cursor, history) = match app.active_pane {
        ActivePane::Left => (&mut app.notes_rope, &mut app.notes_selection, &mut app.notes_cursor, &mut app.notes_history),
        ActivePane::Right => (&mut app.extraction_rope, &mut app.extraction_selection, &mut app.extraction_cursor, &mut app.extraction_history),
    };

    match (key.code, key.modifiers) {
        // Basic character input
        (KeyCode::Char(c), mods) if !mods.contains(KeyModifiers::CONTROL) && !mods.contains(KeyModifiers::SUPER) => {
            // Filter out any control characters or ANSI codes
            if !text_filter::is_allowed_char(c) {
                return Ok(true); // Consume the character but don't insert it
            }

            // Use grid cursor position for insertion
            let cursor_row = grid_cursor.row;
            let cursor_col = grid_cursor.col;

            // Ensure the line exists
            while rope.len_lines() <= cursor_row {
                let newline_pos = rope.len_chars();
                rope.insert(newline_pos, "\n");
            }

            // Get the line boundaries
            let line_start = rope.line_to_char(cursor_row);
            let line_end = if cursor_row + 1 < rope.len_lines() {
                rope.line_to_char(cursor_row + 1).saturating_sub(1) // Exclude newline
            } else {
                rope.len_chars()
            };

            let line_len = line_end.saturating_sub(line_start);

            // If cursor is beyond line end, pad with spaces
            if cursor_col > line_len {
                let padding = " ".repeat(cursor_col - line_len);
                rope.insert(line_end, &padding);
            }

            // Calculate the actual position
            let char_pos = line_start + cursor_col;

            // Create a transaction for history tracking
            let old_rope = rope.clone();

            // Overwrite mode: replace the character at this position instead of inserting
            if char_pos < rope.len_chars() {
                // Check if we're not replacing a newline
                let ch_at_pos = rope.char(char_pos);
                if ch_at_pos == '\n' {
                    // Don't overwrite newlines, insert before them
                    rope.insert(char_pos, &c.to_string());
                } else {
                    // Replace the character
                    rope.remove(char_pos..char_pos + 1);
                    rope.insert(char_pos, &c.to_string());
                }
            } else {
                // Position is beyond rope length, just insert
                rope.insert(char_pos, &c.to_string());
            }

            // Move cursor right
            grid_cursor.col += 1;
            grid_cursor.desired_col = Some(grid_cursor.col);

            // Update selection to match cursor
            let new_pos = line_start + grid_cursor.col;
            *selection = Selection::point(new_pos);

            // Create and commit transaction to history for undo/redo
            // We need to calculate the changes between old and new rope
            let transaction = Transaction::change(
                &old_rope,
                std::iter::once((0, old_rope.len_chars(), Some(rope.to_string().into())))
            );
            let state = State { doc: old_rope.clone(), selection: selection.clone() };
            history.commit_revision(&transaction, &state);

            app.needs_redraw = true;

            // Auto-save for notes pane
            if app.active_pane == ActivePane::Left {
                auto_save_note(app)?;
            }
            Ok(true)
        }

        // Backspace
        (KeyCode::Backspace, mods) if !mods.contains(KeyModifiers::ALT) && !mods.contains(KeyModifiers::SUPER) => {
            let old_rope = rope.clone();
            let mut changed = false;

            if grid_cursor.col > 0 {
                // Move cursor left
                grid_cursor.col -= 1;
                grid_cursor.desired_col = Some(grid_cursor.col);

                // Calculate position to delete
                if grid_cursor.row < rope.len_lines() {
                    let line_start = rope.line_to_char(grid_cursor.row);
                    let delete_pos = line_start + grid_cursor.col;

                    // Only delete if there's actually a character there
                    if delete_pos < rope.len_chars() {
                        let next_pos = delete_pos + 1;
                        rope.remove(delete_pos..next_pos.min(rope.len_chars()));

                        // Update selection to match cursor
                        *selection = Selection::point(delete_pos);
                        changed = true;
                    }
                }
            } else if grid_cursor.row > 0 {
                // At beginning of line, move to end of previous line
                grid_cursor.row -= 1;

                if grid_cursor.row < rope.len_lines() {
                    let line = rope.line(grid_cursor.row);
                    let line_len = line.len_chars().saturating_sub(1); // Exclude newline
                    grid_cursor.col = line_len;
                    grid_cursor.desired_col = Some(grid_cursor.col);

                    // Delete the newline character
                    let line_end = rope.line_to_char(grid_cursor.row + 1).saturating_sub(1);
                    if line_end < rope.len_chars() {
                        rope.remove(line_end..line_end + 1);
                        *selection = Selection::point(line_end);
                        changed = true;
                    }
                }
            }

            // Commit to history if we made changes
            if changed {
                let transaction = Transaction::change(
                    &old_rope,
                    std::iter::once((0, old_rope.len_chars(), Some(rope.to_string().into())))
                );
                let state = State { doc: old_rope.clone(), selection: selection.clone() };
                history.commit_revision(&transaction, &state);
            }

            app.needs_redraw = true;

            // Auto-save for notes pane
            if app.active_pane == ActivePane::Left {
                auto_save_note(app)?;
            }
            Ok(true)
        }

        // Enter
        (KeyCode::Enter, _) => {
            // Create a transaction for history tracking
            let old_rope = rope.clone();

            // Insert newline at cursor position
            let cursor_row = grid_cursor.row;
            let cursor_col = grid_cursor.col;

            // Ensure the line exists
            while rope.len_lines() <= cursor_row {
                let newline_pos = rope.len_chars();
                rope.insert(newline_pos, "\n");
            }

            // Get the line boundaries
            let line_start = rope.line_to_char(cursor_row);
            let line_end = if cursor_row + 1 < rope.len_lines() {
                rope.line_to_char(cursor_row + 1).saturating_sub(1) // Exclude newline
            } else {
                rope.len_chars()
            };

            let line_len = line_end.saturating_sub(line_start);

            // If cursor is beyond line end, pad with spaces up to cursor
            if cursor_col > line_len {
                let padding = " ".repeat(cursor_col - line_len);
                rope.insert(line_end, &padding);
            }

            // Calculate the actual insertion position
            let insert_pos = line_start + cursor_col;

            // Insert newline
            rope.insert(insert_pos, "\n");

            // Move cursor to beginning of next line
            grid_cursor.row += 1;
            grid_cursor.col = 0;
            grid_cursor.desired_col = Some(0);

            // Update selection to match cursor
            *selection = Selection::point(insert_pos + 1);

            // Commit transaction to history for undo/redo
            let transaction = Transaction::change(
                &old_rope,
                std::iter::once((0, old_rope.len_chars(), Some(rope.to_string().into())))
            );
            let state = State { doc: old_rope.clone(), selection: selection.clone() };
            history.commit_revision(&transaction, &state);

            app.needs_redraw = true;

            // Auto-save for notes pane
            if app.active_pane == ActivePane::Left {
                auto_save_note(app)?;
            }
            Ok(true)
        }

        // Arrow keys - handled in main keyboard.rs for grid-aware movement
        (KeyCode::Left, mods) if !mods.contains(KeyModifiers::SUPER) && !mods.contains(KeyModifiers::ALT) => {
            Ok(false) // Fall through to main handler
        }

        (KeyCode::Right, mods) if !mods.contains(KeyModifiers::SUPER) && !mods.contains(KeyModifiers::ALT) => {
            Ok(false) // Fall through to main handler
        }

        (KeyCode::Up, mods) => {
            Ok(false) // Fall through to main handler
        }

        (KeyCode::Down, mods) => {
            Ok(false) // Fall through to main handler
        }

        // Tab and Delete should fall through to main handler for proper implementation
        (KeyCode::Tab, _) => Ok(false),
        (KeyCode::Delete, _) => Ok(false),

        // Let other keys fall through
        _ => Ok(false),
    }
}

// Auto-save function for notes
fn auto_save_note(app: &mut App) -> Result<()> {
    if let Some(ref mut notes_mode) = app.notes_mode {
        // Extract content from the notes rope
        let content = app.notes_rope.to_string();

        // Extract title from first line
        let title = content.lines()
            .next()
            .unwrap_or("Untitled")
            .trim_start_matches('#')
            .trim()
            .to_string();

        // Extract tags if present (lines starting with "Tags:")
        let mut tags = Vec::new();
        for line in content.lines() {
            if line.starts_with("Tags:") {
                let tags_str = line.trim_start_matches("Tags:").trim();
                tags = tags_str.split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
                break;
            }
        }

        // Update or create note
        if let Some(ref note) = notes_mode.current_note {
            // Update existing note
            notes_mode.db.update_note(&note.id, title, content, tags)?;
        } else if !content.trim().is_empty() {
            // Create new note only if there's content
            let note = notes_mode.db.create_note(title, content, tags)?;
            notes_mode.current_note = Some(note.clone());

            // Add to the notes list if not already there
            if !app.notes_list.iter().any(|n| n.id == note.id) {
                app.notes_list.push(note);
            }
        }
    }
    Ok(())
}