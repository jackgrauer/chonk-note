// MOUSE INTERACTION HANDLER
use crate::{App, kitty_native::MouseEvent};
use anyhow::Result;
use helix_core::{Selection, Range, movement, Transaction, history::State};
use std::time::{Duration, Instant};
use std::fs::OpenOptions;
use std::io::Write;

pub struct MouseState {
    pub last_click: Option<Instant>,
    pub last_click_pos: Option<(u16, u16)>,
    pub drag_start: Option<(usize, usize)>, // (char_pos, line)
    pub is_dragging: bool,
    pub double_click_threshold: Duration,
}

impl Default for MouseState {
    fn default() -> Self {
        Self {
            last_click: None,
            last_click_pos: None,
            drag_start: None,
            is_dragging: false,
            double_click_threshold: Duration::from_millis(500),
        }
    }
}

impl App {
    // Convert screen coordinates to text position
    pub fn screen_to_text_pos(&self, x: u16, y: u16) -> Option<usize> {
        // Only process if in right pane (text editor)
        let (term_width, _) = crate::kitty_native::KittyTerminal::size().ok()?;
        let split_x = term_width / 2;

        // Debug log conversion attempt
        if let Ok(mut file) = OpenOptions::new().create(true).append(true).open("/Users/jack/chonker7_debug.log") {
            writeln!(file, "[SCREEN_TO_TEXT] x={}, y={}, term_width={}, split_x={}",
                x, y, term_width, split_x).ok();
        }

        if x < split_x {
            if let Ok(mut file) = OpenOptions::new().create(true).append(true).open("/Users/jack/chonker7_debug.log") {
                writeln!(file, "[SCREEN_TO_TEXT] Click in PDF pane, ignoring").ok();
            }
            return None; // Click is in PDF pane
        }

        let text_x = x - split_x;

        if let Some(renderer) = &self.edit_display {
            // Account for viewport scrolling
            let actual_y = y as usize + renderer.viewport_y;
            let actual_x = text_x as usize + renderer.viewport_x;

            // Convert to character position in rope
            let line = actual_y.min(self.rope.len_lines().saturating_sub(1));
            let line_start = self.rope.line_to_byte(line);
            let line_str = self.rope.line(line);

            // Find character at x position (simple approach, assumes monospace)
            let mut char_pos = 0;
            let mut display_x = 0;
            for ch in line_str.chars() {
                if display_x >= actual_x {
                    break;
                }
                char_pos += ch.len_utf8();
                display_x += 1; // Simplified - assumes 1 char = 1 column
            }

            Some(line_start + char_pos.min(line_str.len_bytes()))
        } else {
            None
        }
    }
}

pub async fn handle_mouse(app: &mut App, event: MouseEvent, mouse_state: &mut MouseState) -> Result<()> {
    // Debug log all mouse events
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open("/Users/jack/chonker7_debug.log") {
        writeln!(file, "[MOUSE] Event: button={:?}, x={}, y={}, press={}, drag={}",
            event.button, event.x, event.y, event.is_press, event.is_drag).ok();
    }

    match event {
        MouseEvent { button: Some(crate::kitty_native::MouseButton::Left), is_press: true, x, y, .. } => {
            // Debug log click position
            if let Ok(mut file) = OpenOptions::new().create(true).append(true).open("/Users/jack/chonker7_debug.log") {
                writeln!(file, "[MOUSE] Left click at ({}, {})", x, y).ok();
            }

            // Left click - set cursor position
            if let Some(pos) = app.screen_to_text_pos(x, y) {
                // Debug log text position
                if let Ok(mut file) = OpenOptions::new().create(true).append(true).open("/Users/jack/chonker7_debug.log") {
                    writeln!(file, "[MOUSE] Converted to text position: {}", pos).ok();
                }
                let now = Instant::now();

                // Check for double-click
                let is_double_click = if let (Some(last_time), Some((last_x, last_y))) =
                    (mouse_state.last_click, mouse_state.last_click_pos) {
                    now.duration_since(last_time) < mouse_state.double_click_threshold &&
                    (x, y) == (last_x, last_y)
                } else {
                    false
                };

                if is_double_click {
                    // Double-click: select word
                    let rope_slice = app.rope.slice(..);
                    let range = movement::move_next_word_end(
                        rope_slice,
                        Range::point(pos),
                        1
                    );
                    let word_start = movement::move_prev_word_start(
                        rope_slice,
                        Range::point(pos),
                        1
                    ).head;

                    app.selection = Selection::single(word_start, range.head);
                    mouse_state.last_click = None; // Reset to avoid triple-click
                } else {
                    // Single click: move cursor
                    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open("/Users/jack/chonker7_debug.log") {
                        writeln!(file, "[MOUSE] Single click - moving cursor to pos {}", pos).ok();
                    }
                    app.selection = Selection::point(pos);
                    mouse_state.last_click = Some(now);
                    mouse_state.last_click_pos = Some((x, y));

                    // Start potential drag
                    let line = app.rope.byte_to_line(pos);
                    let line_start = app.rope.line_to_byte(line);
                    mouse_state.drag_start = Some((pos, line));
                    mouse_state.is_dragging = false;
                }

                app.needs_redraw = true;
            } else {
                // Debug log when click is outside text area
                if let Ok(mut file) = OpenOptions::new().create(true).append(true).open("/Users/jack/chonker7_debug.log") {
                    writeln!(file, "[MOUSE] Click at ({}, {}) is outside text area or couldn't convert", x, y).ok();
                }
            }
        }

        MouseEvent { button: Some(crate::kitty_native::MouseButton::Left), is_press: false, .. } => {
            // Left button release - end drag
            mouse_state.drag_start = None;
            mouse_state.is_dragging = false;
        }

        MouseEvent { is_drag: true, x, y, .. } if mouse_state.drag_start.is_some() => {
            // Mouse drag - extend selection
            if let Some(end_pos) = app.screen_to_text_pos(x, y) {
                if let Some((start_pos, _)) = mouse_state.drag_start {
                    mouse_state.is_dragging = true;
                    app.selection = Selection::single(start_pos, end_pos);
                    app.needs_redraw = true;
                }
            }
        }

        MouseEvent { button: Some(crate::kitty_native::MouseButton::ScrollUp), .. } => {
            // Scroll up
            if let Some(renderer) = &mut app.edit_display {
                if renderer.viewport_y > 0 {
                    renderer.viewport_y = renderer.viewport_y.saturating_sub(3);
                    app.needs_redraw = true;
                }
            }
        }

        MouseEvent { button: Some(crate::kitty_native::MouseButton::ScrollDown), .. } => {
            // Scroll down
            if let Some(renderer) = &mut app.edit_display {
                let max_y = app.rope.len_lines().saturating_sub(20); // Keep some lines visible
                if renderer.viewport_y < max_y {
                    renderer.viewport_y = (renderer.viewport_y + 3).min(max_y);
                    app.needs_redraw = true;
                }
            }
        }

        _ => {}
    }

    Ok(())
}