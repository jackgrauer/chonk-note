// Mouse handling for chonk-note
use crate::App;
use crate::kitty_native::MouseEvent;
use anyhow::Result;
use helix_core::Selection;
use std::time::{Duration, Instant};

pub struct MouseState {
    pub last_click: Option<Instant>,
    pub last_click_pos: Option<(u16, u16)>,
    pub is_dragging: bool,
    pub drag_start_pos: Option<usize>,
    pub double_click_threshold: Duration,
}

impl Default for MouseState {
    fn default() -> Self {
        Self {
            last_click: None,
            last_click_pos: None,
            is_dragging: false,
            drag_start_pos: None,
            double_click_threshold: Duration::from_millis(500),
        }
    }
}

pub async fn handle_mouse(app: &mut App, event: MouseEvent, mouse_state: &mut MouseState) -> Result<()> {
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
                            let content = app.notes_rope.to_string();
                            let _ = app.notes_mode.db.update_note(&current_note.id, current_note.title.clone(), content, current_note.tags.clone());
                        }

                        // Load selected note
                        app.selected_note_index = note_index;
                        let note = &app.notes_list[note_index];
                        app.notes_rope = helix_core::Rope::from(note.content.as_str());
                        app.notes_selection = Selection::point(0);
                        app.notes_mode.current_note = Some(note.clone());
                        app.notes_grid = crate::virtual_grid::VirtualGrid::new(app.notes_rope.clone());
                        app.notes_cursor = crate::grid_cursor::GridCursor::new();
                        app.needs_redraw = true;
                    }
                }
                return Ok(());
            }

            // Click in editor area - position cursor
            let editor_x = x.saturating_sub(notes_list_width) as usize;
            let editor_y = y.saturating_sub(1) as usize;

            // Move virtual cursor to click position
            app.notes_cursor.move_to(editor_y, editor_x);

            // Update selection if we have a valid text position
            if let Some(char_pos) = app.notes_cursor.to_char_offset(&app.notes_grid) {
                let now = Instant::now();
                let is_double_click = if let (Some(last_time), Some((last_x, last_y))) =
                    (mouse_state.last_click, mouse_state.last_click_pos) {
                    now.duration_since(last_time) < mouse_state.double_click_threshold &&
                    (x, y) == (last_x, last_y)
                } else {
                    false
                };

                if is_double_click {
                    // Double-click: select word
                    let line = app.notes_rope.char_to_line(char_pos);
                    let line_start = app.notes_rope.line_to_char(line);
                    let line_slice = app.notes_rope.line(line);

                    // Find word boundaries
                    let col = char_pos - line_start;
                    let mut word_start = col;
                    let mut word_end = col;

                    let line_str = line_slice.to_string();
                    let chars: Vec<char> = line_str.chars().collect();

                    // Find word start
                    while word_start > 0 && chars.get(word_start - 1).map_or(false, |c| c.is_alphanumeric() || *c == '_') {
                        word_start -= 1;
                    }

                    // Find word end
                    while word_end < chars.len() && chars.get(word_end).map_or(false, |c| c.is_alphanumeric() || *c == '_') {
                        word_end += 1;
                    }

                    app.notes_selection = Selection::single(line_start + word_start, line_start + word_end);
                    mouse_state.last_click = None;
                } else {
                    // Single click: move cursor
                    app.notes_selection = Selection::point(char_pos);
                    mouse_state.last_click = Some(now);
                    mouse_state.last_click_pos = Some((x, y));
                    mouse_state.is_dragging = true;
                    mouse_state.drag_start_pos = Some(char_pos);
                }
            }

            app.needs_redraw = true;
        }

        // Drag - extend selection
        MouseEvent { is_drag: true, x, y, .. } => {
            if mouse_state.is_dragging && x >= notes_list_width {
                let editor_x = x.saturating_sub(notes_list_width) as usize;
                let editor_y = y.saturating_sub(1) as usize;

                app.notes_cursor.move_to(editor_y, editor_x);

                if let (Some(start_pos), Some(end_pos)) = (mouse_state.drag_start_pos, app.notes_cursor.to_char_offset(&app.notes_grid)) {
                    app.notes_selection = Selection::single(start_pos, end_pos);
                    app.needs_redraw = true;
                }
            }
        }

        // Release - end drag
        MouseEvent { button: Some(crate::kitty_native::MouseButton::Left), is_press: false, .. } => {
            mouse_state.is_dragging = false;
            mouse_state.drag_start_pos = None;
        }

        // Scroll up
        MouseEvent { button: Some(crate::kitty_native::MouseButton::ScrollUp), x, .. } => {
            if x < notes_list_width {
                // Scroll notes list
                if app.notes_list_scroll > 0 {
                    app.notes_list_scroll -= 1;
                    app.needs_redraw = true;
                }
            } else {
                // Scroll editor
                if let Some(renderer) = &mut app.notes_display {
                    renderer.scroll_up(3);
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
            } else {
                // Scroll editor
                if let Some(renderer) = &mut app.notes_display {
                    renderer.scroll_down(3);
                    app.needs_redraw = true;
                }
            }
        }

        _ => {}
    }

    Ok(())
}
