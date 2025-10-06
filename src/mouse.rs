// Mouse handling for chonk-note
use crate::App;
use crate::kitty_native::MouseEvent;
use anyhow::Result;

pub struct MouseState {
    pub last_click_pos: Option<(u16, u16)>,
}

impl Default for MouseState {
    fn default() -> Self {
        Self {
            last_click_pos: None,
        }
    }
}

pub async fn handle_mouse(app: &mut App, event: MouseEvent, _mouse_state: &mut MouseState) -> Result<()> {
    let (_term_width, term_height) = crate::kitty_native::KittyTerminal::size()?;
    let notes_list_width = if app.sidebar_expanded { 30 } else { 4 };

    match event {
        // Left click - position cursor or select note
        MouseEvent { button: Some(crate::kitty_native::MouseButton::Left), is_press: true, x, y, .. } => {
            // Click in notes list sidebar
            if x < notes_list_width {
                if !app.notes_list.is_empty() {
                    let clicked_row = y.saturating_sub(1) as usize;
                    let visible_start = app.notes_list_scroll;
                    let note_index = visible_start + clicked_row;

                    if note_index < app.notes_list.len() {
                        // Save current note
                        if let Some(ref current_note) = app.notes_mode.current_note {
                            let lines = app.grid.to_lines();
                            let content = lines.join("\n");
                            let _ = app.notes_mode.db.update_note(&current_note.id, current_note.title.clone(), content, current_note.tags.clone());
                        }

                        // Load selected note
                        app.selected_note_index = note_index;
                        let note = &app.notes_list[note_index];

                        // Load note content into grid
                        let lines: Vec<String> = note.content.lines().map(|s| s.to_string()).collect();
                        app.grid = crate::chunked_grid::ChunkedGrid::from_lines(&lines);
                        app.cursor_row = 0;
                        app.cursor_col = 0;

                        app.notes_mode.current_note = Some(note.clone());

                        // Expand sidebar and enter title editing mode
                        app.sidebar_expanded = true;
                        app.editing_title = true;
                        app.title_buffer = note.title.clone();

                        app.needs_redraw = true;
                    }
                }
                return Ok(());
            }

            // Click in editor area - collapse sidebar and position cursor
            app.sidebar_expanded = false;
            app.editing_title = false;

            // Calculate cursor position from click
            let screen_x = x.saturating_sub(notes_list_width) as usize;
            let screen_y = y.saturating_sub(1) as usize;

            // Set cursor directly - chunked grid handles virtual space
            app.cursor_row = screen_y;
            app.cursor_col = screen_x;

            app.needs_redraw = true;
        }

        // Scroll up
        MouseEvent { button: Some(crate::kitty_native::MouseButton::ScrollUp), x, .. } => {
            if x < notes_list_width {
                // Scroll notes list
                if app.notes_list_scroll > 0 {
                    app.notes_list_scroll -= 1;
                    app.needs_redraw = true;
                }
            }
        }

        // Scroll down
        MouseEvent { button: Some(crate::kitty_native::MouseButton::ScrollDown), x, .. } => {
            if x < notes_list_width {
                // Scroll notes list
                let visible_count = term_height.saturating_sub(2) as usize;
                let max_scroll = app.notes_list.len().saturating_sub(visible_count);
                if app.notes_list_scroll < max_scroll {
                    app.notes_list_scroll += 1;
                    app.needs_redraw = true;
                }
            }
        }

        _ => {}
    }

    Ok(())
}
