// Mouse handling for chonk-note
use crate::App;
use crate::kitty_native::MouseEvent;
use anyhow::Result;

pub struct MouseState {
    pub last_click_pos: Option<(u16, u16)>,
    pub is_dragging: bool,
}

impl Default for MouseState {
    fn default() -> Self {
        Self {
            last_click_pos: None,
            is_dragging: false,
        }
    }
}

pub async fn handle_mouse(app: &mut App, event: MouseEvent, mouse_state: &mut MouseState) -> Result<()> {
    let (_term_width, term_height) = crate::kitty_native::KittyTerminal::size()?;
    let notes_list_width = if app.sidebar_expanded { crate::SIDEBAR_WIDTH_EXPANDED } else { crate::SIDEBAR_WIDTH_COLLAPSED };

    match event {
        // Left click - position cursor or select note
        MouseEvent { button: Some(crate::kitty_native::MouseButton::Left), is_press: true, is_drag: false, x, y, .. } => {
            // Click in notes list sidebar
            if x < notes_list_width {
                if !app.notes_list.is_empty() {
                    let clicked_row = y.saturating_sub(1) as usize;
                    let visible_start = app.notes_list_scroll;
                    let note_index = visible_start + clicked_row;

                    if note_index < app.notes_list.len() {
                        // Save current note
                        app.save_current_note()?;

                        // Reload notes list to get fresh data
                        if let Ok(notes) = app.notes_mode.db.list_notes(100) {
                            app.notes_list = notes;
                        }

                        // Load selected note
                        app.selected_note_index = note_index;
                        let note = &app.notes_list[note_index];

                        // Load note content into grid
                        let lines: Vec<String> = note.content.lines().map(|s| s.to_string()).collect();
                        app.grid = crate::chunked_grid::ChunkedGrid::from_lines(&lines);
                        app.cursor_row = 0;
                        app.cursor_col = 0;
                        app.viewport_row = 0;
                        app.viewport_col = 0;

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

            // Clear any existing selection on new click
            app.grid.clear_selection();

            // Mark position for potential drag
            mouse_state.last_click_pos = Some((x, y));
            mouse_state.is_dragging = false; // Will become true on drag

            app.needs_redraw = true;
        }

        // Mouse drag - update selection
        MouseEvent { is_drag: true, x, y, .. } => {
            let _ = (|| -> std::io::Result<()> {
                use std::io::Write;
                let mut f = std::fs::OpenOptions::new().create(true).append(true).open("/tmp/chonk-debug.log")?;
                writeln!(f, "DRAG EVENT: x={} y={} is_dragging={}", x, y, mouse_state.is_dragging)?;
                f.flush()
            })();

            if x >= notes_list_width {
                let screen_x = x.saturating_sub(notes_list_width) as usize;
                let screen_y = y.saturating_sub(1) as usize;

                // Start selection if this is first drag event
                if !mouse_state.is_dragging {
                    if let Some((start_x, start_y)) = mouse_state.last_click_pos {
                        let start_screen_x = start_x.saturating_sub(notes_list_width) as usize;
                        let start_screen_y = start_y.saturating_sub(1) as usize;
                        app.grid.start_selection(start_screen_y, start_screen_x);
                        mouse_state.is_dragging = true;

                        let _ = (|| -> std::io::Result<()> {
                            use std::io::Write;
                            let mut f = std::fs::OpenOptions::new().create(true).append(true).open("/tmp/chonk-debug.log")?;
                            writeln!(f, "  -> Started selection at ({}, {})", start_screen_y, start_screen_x)?;
                            f.flush()
                        })();
                    }
                }

                // Update cursor position
                app.cursor_row = screen_y;
                app.cursor_col = screen_x;

                // Update block selection
                app.grid.update_selection(screen_y, screen_x);

                let _ = (|| -> std::io::Result<()> {
                    use std::io::Write;
                    let mut f = std::fs::OpenOptions::new().create(true).append(true).open("/tmp/chonk-debug.log")?;
                    writeln!(f, "  -> Updated selection to ({}, {})", screen_y, screen_x)?;
                    f.flush()
                })();

                app.needs_redraw = true;
            }
        }

        // Mouse release - keep selection but stop dragging
        MouseEvent { button: Some(crate::kitty_native::MouseButton::Left), is_press: false, .. } => {
            mouse_state.is_dragging = false;
            mouse_state.last_click_pos = None;
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
