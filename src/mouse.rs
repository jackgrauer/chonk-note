// Mouse handling for chonk-note
use crate::App;
use crate::kitty_native::MouseEvent;
use anyhow::Result;

pub struct MouseState {
    pub last_click_pos: Option<(u16, u16)>,
    pub is_dragging: bool,
    pub last_click_time: std::time::Instant,
    pub last_clicked_note: Option<usize>,
}

impl Default for MouseState {
    fn default() -> Self {
        Self {
            last_click_pos: None,
            is_dragging: false,
            last_click_time: std::time::Instant::now(),
            last_clicked_note: None,
        }
    }
}

pub async fn handle_mouse(app: &mut App, event: MouseEvent, mouse_state: &mut MouseState) -> Result<()> {
    let (term_width, term_height) = crate::kitty_native::KittyTerminal::size()?;
    let notes_list_width = if app.sidebar_expanded { crate::SIDEBAR_WIDTH_EXPANDED } else { crate::SIDEBAR_WIDTH_COLLAPSED };

    match event {
        // Left click - position cursor or select note
        MouseEvent { button: Some(crate::kitty_native::MouseButton::Left), is_press: true, is_drag: false, x, y, .. } => {
            // Click on title bar (row 0) - toggle sidebar if clicking on indicator
            if y == 0 {
                // Click on the left part of title bar (first 2 chars) toggles sidebar
                if x <= 1 {
                    app.sidebar_expanded = !app.sidebar_expanded;
                    app.needs_redraw = true;
                    return Ok(());
                }
            }

            // Click in notes list sidebar
            if x < notes_list_width {
                if !app.notes_list.is_empty() {
                    // First click: just expand sidebar if collapsed
                    if !app.sidebar_expanded {
                        app.sidebar_expanded = true;
                        app.needs_redraw = true;
                        return Ok(());
                    }

                    // Check for double-click
                    let clicked_row = y.saturating_sub(1) as usize;
                    let visible_start = app.notes_list_scroll;
                    let note_index = visible_start + clicked_row;

                    if note_index < app.notes_list.len() {
                        let now = std::time::Instant::now();
                        let time_since_last_click = now.duration_since(mouse_state.last_click_time).as_millis();
                        let is_double_click = time_since_last_click < 500 && mouse_state.last_clicked_note == Some(note_index);

                        mouse_state.last_click_time = now;
                        mouse_state.last_clicked_note = Some(note_index);

                        if is_double_click {
                            // Double-click: enter rename mode
                            app.selected_note_index = note_index;
                            let note = &app.notes_list[note_index];
                            app.editing_title = true;
                            app.title_buffer = note.title.clone();
                            app.needs_redraw = true;
                        } else {
                            // Single click: switch to the note
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

                            app.needs_redraw = true;
                        }
                    }
                }
                return Ok(());
            }

            // Click in editor area - collapse sidebar and position cursor
            app.sidebar_expanded = false;
            app.editing_title = false;

            // Calculate cursor position from click (editor now spans full width)
            let screen_x = x as usize;
            let screen_y = y.saturating_sub(1) as usize;

            // Set cursor position
            app.cursor_row = app.viewport_row + screen_y;
            app.cursor_col = app.viewport_col + screen_x;

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

            // Editor now spans full width, so use x directly
            let screen_x = x as usize;
            let screen_y = y.saturating_sub(1) as usize;

            // Only process if we're dragging
            if mouse_state.is_dragging || x >= notes_list_width {

                // Start selection if this is first drag event
                if !mouse_state.is_dragging {
                    if let Some((start_x, start_y)) = mouse_state.last_click_pos {
                        // Editor now spans full width, use coordinates directly
                        let start_screen_x = start_x as usize;
                        let start_screen_y = start_y.saturating_sub(1) as usize;
                        // Start selection at actual grid position
                        let start_grid_row = app.viewport_row + start_screen_y;
                        let start_grid_col = app.viewport_col + start_screen_x;
                        app.grid.start_selection(start_grid_row, start_grid_col);
                        mouse_state.is_dragging = true;

                        let _ = (|| -> std::io::Result<()> {
                            use std::io::Write;
                            let mut f = std::fs::OpenOptions::new().create(true).append(true).open("/tmp/chonk-debug.log")?;
                            writeln!(f, "  -> Started selection at ({}, {})", start_screen_y, start_screen_x)?;
                            f.flush()
                        })();
                    }
                }

                // Auto-scroll viewport when dragging near edges
                let scroll_margin = 2; // cells from edge to trigger scrolling

                // Scroll vertically - allow cursor to keep moving at edges
                if screen_y < scroll_margin {
                    if app.viewport_row > 0 {
                        app.viewport_row = app.viewport_row.saturating_sub(1);
                    }
                    // Even if we can't scroll, cursor can still move within visible area
                } else if screen_y >= (term_height.saturating_sub(2) as usize).saturating_sub(scroll_margin) {
                    app.viewport_row += 1;
                }

                // Scroll horizontally
                let editor_width = (term_width.saturating_sub(notes_list_width)) as usize;
                if screen_x < scroll_margin {
                    if app.viewport_col > 0 {
                        app.viewport_col = app.viewport_col.saturating_sub(1);
                    }
                    // Even if we can't scroll, cursor can still move within visible area
                } else if screen_x >= editor_width.saturating_sub(scroll_margin) {
                    app.viewport_col += 1;
                }

                // Update cursor position
                app.cursor_row = app.viewport_row + screen_y;
                app.cursor_col = app.viewport_col + screen_x;

                // Update block selection
                app.grid.update_selection(app.cursor_row, app.cursor_col);

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
