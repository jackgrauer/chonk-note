// Keyboard handling for chonk-note
use crate::App;
use crate::kitty_native::{KeyCode, KeyEvent, KeyModifiers};
use anyhow::Result;
use helix_core::{Transaction, Selection, history::State};

pub async fn handle_input(app: &mut App, key: KeyEvent) -> Result<bool> {
    // Ctrl+Q - Quit
    if key.code == KeyCode::Char('q') && key.modifiers.contains(KeyModifiers::CONTROL) {
        app.exit_requested = true;
        return Ok(false);
    }

    // Ctrl+N - New note
    if key.code == KeyCode::Char('n') && key.modifiers.contains(KeyModifiers::CONTROL) {
        app.notes_mode.handle_command(&mut app.notes_rope, &mut app.notes_selection, "new")?;
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
                let content = app.notes_rope.to_string();
                let _ = app.notes_mode.db.update_note(&current_note.id, current_note.title.clone(), content, current_note.tags.clone());
            }

            app.selected_note_index -= 1;
            if app.selected_note_index < app.notes_list_scroll {
                app.notes_list_scroll = app.selected_note_index;
            }

            // Load selected note
            if !app.notes_list.is_empty() {
                let note = &app.notes_list[app.selected_note_index];
                app.notes_rope = helix_core::Rope::from(note.content.as_str());
                app.notes_selection = Selection::point(0);
                app.notes_mode.current_note = Some(note.clone());
                app.notes_grid = crate::virtual_grid::VirtualGrid::new(app.notes_rope.clone());
                app.notes_cursor = crate::grid_cursor::GridCursor::new();
            }

            app.needs_redraw = true;
        }
        return Ok(true);
    }

    if key.code == KeyCode::Down && key.modifiers.contains(KeyModifiers::CONTROL) {
        if app.selected_note_index < app.notes_list.len().saturating_sub(1) {
            // Save current note
            if let Some(ref current_note) = app.notes_mode.current_note {
                let content = app.notes_rope.to_string();
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
                app.notes_rope = helix_core::Rope::from(note.content.as_str());
                app.notes_selection = Selection::point(0);
                app.notes_mode.current_note = Some(note.clone());
                app.notes_grid = crate::virtual_grid::VirtualGrid::new(app.notes_rope.clone());
                app.notes_cursor = crate::grid_cursor::GridCursor::new();
            }

            app.needs_redraw = true;
        }
        return Ok(true);
    }

    // Cmd+C - Copy
    if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::SUPER) {
        let text = if let Some(block_sel) = &app.notes_block_selection {
            block_sel.to_selection(&app.notes_rope)
                .ranges()
                .into_iter()
                .map(|r| app.notes_rope.slice(r.from()..r.to()).to_string())
                .collect::<Vec<_>>()
                .join("\n")
        } else {
            let range = app.notes_selection.primary();
            app.notes_rope.slice(range.from()..range.to()).to_string()
        };

        if !text.is_empty() {
            let _ = copy_to_clipboard(&text);
            app.status_message = format!("Copied {} chars", text.len());
        }
        app.needs_redraw = true;
        return Ok(true);
    }

    // Cmd+X - Cut
    if key.code == KeyCode::Char('x') && key.modifiers.contains(KeyModifiers::SUPER) {
        let range = app.notes_selection.primary();
        if range.from() != range.to() {
            let text = app.notes_rope.slice(range.from()..range.to()).to_string();
            let _ = copy_to_clipboard(&text);

            let state = State {
                doc: app.notes_rope.clone(),
                selection: app.notes_selection.clone(),
            };

            let transaction = Transaction::delete(&app.notes_rope, std::iter::once((range.from(), range.to())));
            if transaction.apply(&mut app.notes_rope) {
                app.notes_selection = app.notes_selection.clone().map(transaction.changes());
                app.notes_history.commit_revision(&transaction, &state);
                app.notes_grid = crate::virtual_grid::VirtualGrid::new(app.notes_rope.clone());
                app.status_message = format!("Cut {} chars", text.len());
            }
        }
        app.needs_redraw = true;
        return Ok(true);
    }

    // Cmd+V - Paste
    if key.code == KeyCode::Char('v') && key.modifiers.contains(KeyModifiers::SUPER) {
        if let Ok(text) = paste_from_clipboard() {
            let state = State {
                doc: app.notes_rope.clone(),
                selection: app.notes_selection.clone(),
            };

            let transaction = Transaction::insert(&app.notes_rope, &app.notes_selection, text.clone().into());
            if transaction.apply(&mut app.notes_rope) {
                app.notes_selection = app.notes_selection.clone().map(transaction.changes());
                app.notes_history.commit_revision(&transaction, &state);
                app.notes_grid = crate::virtual_grid::VirtualGrid::new(app.notes_rope.clone());
                app.status_message = format!("Pasted {} chars", text.len());
            }
        }
        app.needs_redraw = true;
        return Ok(true);
    }

    // Cmd+A - Select All
    if key.code == KeyCode::Char('a') && key.modifiers.contains(KeyModifiers::SUPER) {
        app.notes_selection = Selection::single(0, app.notes_rope.len_chars());
        app.notes_block_selection = None;
        app.needs_redraw = true;
        return Ok(true);
    }

    // Arrow keys - Move cursor
    match key.code {
        KeyCode::Up if !key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.notes_cursor.move_up();
            if let Some(char_pos) = app.notes_cursor.to_char_offset(&app.notes_grid) {
                app.notes_selection = Selection::point(char_pos);
            }
            app.needs_redraw = true;
        }
        KeyCode::Down if !key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.notes_cursor.move_down(200);
            if let Some(char_pos) = app.notes_cursor.to_char_offset(&app.notes_grid) {
                app.notes_selection = Selection::point(char_pos);
            }
            app.needs_redraw = true;
        }
        KeyCode::Left => {
            app.notes_cursor.move_left();
            if let Some(char_pos) = app.notes_cursor.to_char_offset(&app.notes_grid) {
                app.notes_selection = Selection::point(char_pos);
            }
            app.needs_redraw = true;
        }
        KeyCode::Right => {
            app.notes_cursor.move_right(1000);
            if let Some(char_pos) = app.notes_cursor.to_char_offset(&app.notes_grid) {
                app.notes_selection = Selection::point(char_pos);
            }
            app.needs_redraw = true;
        }
        KeyCode::Backspace => {
            let state = State {
                doc: app.notes_rope.clone(),
                selection: app.notes_selection.clone(),
            };

            let range = app.notes_selection.primary();
            let transaction = if range.from() != range.to() {
                Transaction::delete(&app.notes_rope, std::iter::once((range.from(), range.to())))
            } else {
                Transaction::delete(&app.notes_rope, std::iter::once((
                    range.head.saturating_sub(1),
                    range.head
                )))
            };

            if transaction.apply(&mut app.notes_rope) {
                app.notes_selection = app.notes_selection.clone().map(transaction.changes());
                app.notes_history.commit_revision(&transaction, &state);
                app.notes_grid = crate::virtual_grid::VirtualGrid::new(app.notes_rope.clone());
            }
            app.needs_redraw = true;
        }
        KeyCode::Enter => {
            let state = State {
                doc: app.notes_rope.clone(),
                selection: app.notes_selection.clone(),
            };

            let transaction = Transaction::insert(&app.notes_rope, &app.notes_selection, "\n".into());
            if transaction.apply(&mut app.notes_rope) {
                app.notes_selection = app.notes_selection.clone().map(transaction.changes());
                app.notes_history.commit_revision(&transaction, &state);
                app.notes_grid = crate::virtual_grid::VirtualGrid::new(app.notes_rope.clone());
            }
            app.needs_redraw = true;
        }
        KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) && !key.modifiers.contains(KeyModifiers::SUPER) => {
            let state = State {
                doc: app.notes_rope.clone(),
                selection: app.notes_selection.clone(),
            };

            let transaction = Transaction::insert(&app.notes_rope, &app.notes_selection, c.to_string().into());
            if transaction.apply(&mut app.notes_rope) {
                app.notes_selection = app.notes_selection.clone().map(transaction.changes());
                app.notes_history.commit_revision(&transaction, &state);
                app.notes_grid = crate::virtual_grid::VirtualGrid::new(app.notes_rope.clone());
                app.notes_cursor = crate::grid_cursor::GridCursor::from_char_offset(app.notes_selection.primary().head, &app.notes_grid);
            }
            app.needs_redraw = true;
        }
        _ => {}
    }

    // Auto-save after edits
    if let Some(ref current_note) = app.notes_mode.current_note {
        let content = app.notes_rope.to_string();
        let _ = app.notes_mode.db.update_note(&current_note.id, current_note.title.clone(), content, current_note.tags.clone());
    }

    Ok(true)
}

// macOS clipboard functions
fn copy_to_clipboard(text: &str) -> Result<()> {
    use std::process::Command;
    let mut child = Command::new("pbcopy")
        .stdin(std::process::Stdio::piped())
        .spawn()?;

    if let Some(mut stdin) = child.stdin.take() {
        use std::io::Write;
        stdin.write_all(text.as_bytes())?;
    }

    child.wait()?;
    Ok(())
}

fn paste_from_clipboard() -> Result<String> {
    use std::process::Command;
    let output = Command::new("pbpaste").output()?;
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}
