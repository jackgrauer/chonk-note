// Simplified dual-pane keyboard handling for Notes mode
use crate::{App, AppMode, ActivePane};
use anyhow::Result;
use crate::kitty_native::{KeyCode, KeyEvent, KeyModifiers};
use helix_core::{Transaction, Selection};

// Handle keyboard input for dual-pane Notes mode
// Returns true if the key was handled, false otherwise
pub fn handle_dual_pane_input(app: &mut App, key: &KeyEvent) -> Result<bool> {
    // Only handle if we're in Notes mode
    if app.app_mode != AppMode::NotesEditor {
        return Ok(false);
    }

    // Get the active rope and selection
    let (rope, selection) = match app.active_pane {
        ActivePane::Left => (&mut app.notes_rope, &mut app.notes_selection),
        ActivePane::Right => (&mut app.extraction_rope, &mut app.extraction_selection),
    };

    match (key.code, key.modifiers) {
        // Basic character input
        (KeyCode::Char(c), mods) if !mods.contains(KeyModifiers::CONTROL) && !mods.contains(KeyModifiers::SUPER) => {
            let transaction = Transaction::insert(rope, selection, c.to_string().into());
            transaction.apply(rope);
            *selection = selection.clone().map(transaction.changes());
            app.needs_redraw = true;

            // Auto-save for notes pane
            if app.active_pane == ActivePane::Left {
                auto_save_note(app)?;
            }
            Ok(true)
        }

        // Backspace
        (KeyCode::Backspace, mods) if !mods.contains(KeyModifiers::ALT) && !mods.contains(KeyModifiers::SUPER) => {
            if selection.primary().head > 0 {
                let transaction = Transaction::delete(rope, std::iter::once((
                    selection.primary().head.saturating_sub(1),
                    selection.primary().head
                )));
                transaction.apply(rope);
                *selection = selection.clone().map(transaction.changes());
                app.needs_redraw = true;

                // Auto-save for notes pane
                if app.active_pane == ActivePane::Left {
                    auto_save_note(app)?;
                }
            }
            Ok(true)
        }

        // Enter
        (KeyCode::Enter, _) => {
            let transaction = Transaction::insert(rope, selection, "\n".into());
            transaction.apply(rope);
            *selection = selection.clone().map(transaction.changes());
            app.needs_redraw = true;

            // Auto-save for notes pane
            if app.active_pane == ActivePane::Left {
                auto_save_note(app)?;
            }
            Ok(true)
        }

        // Arrow keys for navigation
        (KeyCode::Left, mods) if !mods.contains(KeyModifiers::SUPER) && !mods.contains(KeyModifiers::ALT) => {
            let pos = selection.primary().head;
            if pos > 0 {
                let new_pos = pos - 1;
                if mods.contains(KeyModifiers::SHIFT) {
                    let anchor = selection.primary().anchor;
                    *selection = Selection::single(anchor, new_pos);
                } else {
                    *selection = Selection::point(new_pos);
                }
                app.needs_redraw = true;
            }
            Ok(true)
        }

        (KeyCode::Right, mods) if !mods.contains(KeyModifiers::SUPER) && !mods.contains(KeyModifiers::ALT) => {
            let pos = selection.primary().head;
            if pos < rope.len_chars() {
                let new_pos = pos + 1;
                if mods.contains(KeyModifiers::SHIFT) {
                    let anchor = selection.primary().anchor;
                    *selection = Selection::single(anchor, new_pos);
                } else {
                    *selection = Selection::point(new_pos);
                }
                app.needs_redraw = true;
            }
            Ok(true)
        }

        (KeyCode::Up, mods) => {
            let pos = selection.primary().head;
            let line = rope.char_to_line(pos);
            if line > 0 {
                let line_start = rope.line_to_char(line);
                let col = pos - line_start;

                let new_line = line - 1;
                let new_line_start = rope.line_to_char(new_line);
                let new_line_len = rope.line(new_line).len_chars().saturating_sub(1);
                let new_pos = new_line_start + col.min(new_line_len);

                if mods.contains(KeyModifiers::SHIFT) {
                    let anchor = selection.primary().anchor;
                    *selection = Selection::single(anchor, new_pos);
                } else {
                    *selection = Selection::point(new_pos);
                }
                app.needs_redraw = true;
            }
            Ok(true)
        }

        (KeyCode::Down, mods) => {
            let pos = selection.primary().head;
            let line = rope.char_to_line(pos);
            let max_line = rope.len_lines() - 1;

            if line < max_line {
                let line_start = rope.line_to_char(line);
                let col = pos - line_start;

                let new_line = line + 1;
                let new_line_start = rope.line_to_char(new_line);
                let new_line_len = rope.line(new_line).len_chars().saturating_sub(1);
                let new_pos = new_line_start + col.min(new_line_len);

                if mods.contains(KeyModifiers::SHIFT) {
                    let anchor = selection.primary().anchor;
                    *selection = Selection::single(anchor, new_pos);
                } else {
                    *selection = Selection::point(new_pos);
                }
                app.needs_redraw = true;
            }
            Ok(true)
        }

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