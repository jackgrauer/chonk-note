// MOUSE INTERACTION HANDLER
use crate::{App, kitty_native::MouseEvent};
use anyhow::Result;
use helix_core::{Selection, Range, movement, Transaction, history::State};
use std::time::{Duration, Instant};
use std::fs::OpenOptions;
use std::io::Write;

// Smooth scrolling state for trackpad gestures
pub struct ScrollMomentum {
    pub velocity_y: f32,
    pub velocity_x: f32,
    pub last_update: Instant,
    pub friction: f32,
    pub min_velocity: f32,
}

impl Default for ScrollMomentum {
    fn default() -> Self {
        Self {
            velocity_y: 0.0,
            velocity_x: 0.0,
            last_update: Instant::now(),
            friction: 0.95,  // Momentum decay factor
            min_velocity: 0.1, // Minimum velocity before stopping
        }
    }
}

pub struct MouseState {
    pub last_click: Option<Instant>,
    pub last_click_pos: Option<(u16, u16)>,
    pub drag_start: Option<(usize, usize)>, // (char_pos, line)
    pub is_dragging: bool,
    pub double_click_threshold: Duration,
    pub scroll_momentum: ScrollMomentum,
    pub last_scroll_time: Option<Instant>,
    pub scroll_accumulator_y: f32,  // For sub-line precision
    pub scroll_accumulator_x: f32,
    pub pinch_scale: f32,  // For zoom gestures
}

impl Default for MouseState {
    fn default() -> Self {
        Self {
            last_click: None,
            last_click_pos: None,
            drag_start: None,
            is_dragging: false,
            double_click_threshold: Duration::from_millis(500),
            scroll_momentum: ScrollMomentum::default(),
            last_scroll_time: None,
            scroll_accumulator_y: 0.0,
            scroll_accumulator_x: 0.0,
            pinch_scale: 1.0,
        }
    }
}

impl MouseState {
    // Update momentum and apply friction
    pub fn update_momentum(&mut self) -> (f32, f32) {
        let now = Instant::now();
        let dt = now.duration_since(self.scroll_momentum.last_update).as_secs_f32();

        if dt > 0.0 {
            // Apply friction
            self.scroll_momentum.velocity_y *= self.scroll_momentum.friction.powf(dt * 60.0);
            self.scroll_momentum.velocity_x *= self.scroll_momentum.friction.powf(dt * 60.0);

            // Stop if velocity is too small
            if self.scroll_momentum.velocity_y.abs() < self.scroll_momentum.min_velocity {
                self.scroll_momentum.velocity_y = 0.0;
            }
            if self.scroll_momentum.velocity_x.abs() < self.scroll_momentum.min_velocity {
                self.scroll_momentum.velocity_x = 0.0;
            }

            self.scroll_momentum.last_update = now;
        }

        (self.scroll_momentum.velocity_x * dt, self.scroll_momentum.velocity_y * dt)
    }

    // Add velocity from a scroll event
    pub fn add_scroll_velocity(&mut self, dx: f32, dy: f32) {
        let now = Instant::now();

        // If this is a continuation of scrolling, add to momentum
        if let Some(last_time) = self.last_scroll_time {
            if now.duration_since(last_time) < Duration::from_millis(50) {
                // Smooth blending of velocities
                self.scroll_momentum.velocity_y = self.scroll_momentum.velocity_y * 0.7 + dy * 10.0 * 0.3;
                self.scroll_momentum.velocity_x = self.scroll_momentum.velocity_x * 0.7 + dx * 10.0 * 0.3;
            } else {
                // Reset momentum if scrolling stopped
                self.scroll_momentum.velocity_y = dy * 10.0;
                self.scroll_momentum.velocity_x = dx * 10.0;
            }
        } else {
            self.scroll_momentum.velocity_y = dy * 10.0;
            self.scroll_momentum.velocity_x = dx * 10.0;
        }

        self.last_scroll_time = Some(now);
        self.scroll_momentum.last_update = now;
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

// Apply smooth scrolling with sub-line precision
pub fn apply_smooth_scroll(app: &mut App, mouse_state: &mut MouseState) {
    if let Some(renderer) = &mut app.edit_display {
        let (dx, dy) = mouse_state.update_momentum();

        // Accumulate fractional scrolling for smooth motion
        mouse_state.scroll_accumulator_y += dy;
        mouse_state.scroll_accumulator_x += dx;

        // Convert accumulated scroll to lines
        let lines_to_scroll_y = mouse_state.scroll_accumulator_y as i32;
        let lines_to_scroll_x = mouse_state.scroll_accumulator_x as i32;

        // Keep fractional part for next frame
        mouse_state.scroll_accumulator_y -= lines_to_scroll_y as f32;
        mouse_state.scroll_accumulator_x -= lines_to_scroll_x as f32;

        // Apply vertical scrolling
        if lines_to_scroll_y != 0 {
            let max_y = app.rope.len_lines().saturating_sub(20);
            let new_y = if lines_to_scroll_y > 0 {
                (renderer.viewport_y + lines_to_scroll_y as usize).min(max_y)
            } else {
                renderer.viewport_y.saturating_sub((-lines_to_scroll_y) as usize)
            };

            if new_y != renderer.viewport_y {
                renderer.viewport_y = new_y;
                app.needs_redraw = true;
            }
        }

        // Apply horizontal scrolling if needed
        if lines_to_scroll_x != 0 {
            let new_x = if lines_to_scroll_x > 0 {
                renderer.viewport_x + lines_to_scroll_x as usize
            } else {
                renderer.viewport_x.saturating_sub((-lines_to_scroll_x) as usize)
            };

            if new_x != renderer.viewport_x {
                renderer.viewport_x = new_x.min(200); // Max horizontal scroll
                app.needs_redraw = true;
            }
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
            // Smooth scroll up with momentum
            mouse_state.add_scroll_velocity(0.0, -3.0);
            apply_smooth_scroll(app, mouse_state);
        }

        MouseEvent { button: Some(crate::kitty_native::MouseButton::ScrollDown), .. } => {
            // Smooth scroll down with momentum
            mouse_state.add_scroll_velocity(0.0, 3.0);
            apply_smooth_scroll(app, mouse_state);
        }

        _ => {}
    }

    Ok(())
}