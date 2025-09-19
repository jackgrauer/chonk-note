// MOUSE INTERACTION HANDLER
use crate::{App, kitty_native::MouseEvent};
use crate::block_selection::{BlockSelection, char_idx_to_visual_col};
use anyhow::Result;
use helix_core::{Selection, Range, movement};
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
            // Account for viewport scrolling - y is 1-based from terminal coordinates
            // No adjustment needed - y directly maps to the line
            let actual_y = (y as usize) + renderer.viewport_y;
            let actual_x = text_x as usize + renderer.viewport_x;

            if let Ok(mut file) = OpenOptions::new().create(true).append(true).open("/Users/jack/chonker7_debug.log") {
                writeln!(file, "[SCREEN_TO_TEXT] text_x={}, actual_x={}, actual_y={}, viewport_y={}",
                    text_x, actual_x, actual_y, renderer.viewport_y).ok();
            }

            // Convert to character position in rope
            let line = actual_y.min(self.rope.len_lines().saturating_sub(1));
            let line_start = self.rope.line_to_char(line);
            let line_str = self.rope.line(line);

            if let Ok(mut file) = OpenOptions::new().create(true).append(true).open("/Users/jack/chonker7_debug.log") {
                writeln!(file, "[SCREEN_TO_TEXT] line={}, line_start={}, line_len={}",
                    line, line_start, line_str.len_chars()).ok();
            }

            // Find character at x position (simple approach, assumes monospace)
            let mut char_pos = 0;
            let mut display_x = 0;
            for ch in line_str.chars() {
                if display_x >= actual_x {
                    break;
                }
                char_pos += 1; // Count chars, not bytes
                display_x += 1; // Simplified - assumes 1 char = 1 column
            }

            // For virtual space: don't clamp to line length
            // Store the desired column separately if it's past end of line
            let final_pos = if actual_x > display_x && display_x == line_str.len_chars() {
                // Clicked past end of line - position at line end
                line_start + line_str.len_chars().saturating_sub(1).max(0)
            } else {
                line_start + char_pos.min(line_str.len_chars())
            };

            if let Ok(mut file) = OpenOptions::new().create(true).append(true).open("/Users/jack/chonker7_debug.log") {
                writeln!(file, "[SCREEN_TO_TEXT] final_pos={} (line_start={} + char_pos={})",
                    final_pos, line_start, char_pos).ok();
            }

            Some(final_pos)
        } else {
            if let Ok(mut file) = OpenOptions::new().create(true).append(true).open("/Users/jack/chonker7_debug.log") {
                writeln!(file, "[SCREEN_TO_TEXT] No edit_display available!").ok();
            }
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
    // Debug log all mouse events with more detail
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open("/Users/jack/chonker7_debug.log") {
        writeln!(file, "[MOUSE] Event: button={:?}, x={}, y={}, press={}, drag={}, shift={}, alt={}, ctrl={}",
            event.button, event.x, event.y, event.is_press, event.is_drag,
            event.modifiers.shift, event.modifiers.alt, event.modifiers.ctrl).ok();
        writeln!(file, "[MOUSE] State: is_dragging={}, block_selection={:?}",
            mouse_state.is_dragging, app.block_selection.is_some()).ok();
    }

    match event {
        // Handle drag events FIRST before regular clicks
        MouseEvent { is_drag: true, x, y, .. } => {
            // Mouse drag - always block selection
            if let Ok(mut file) = OpenOptions::new().create(true).append(true).open("/Users/jack/chonker7_debug.log") {
                writeln!(file, "[MOUSE] DRAG EVENT MATCHED! x={}, y={}", x, y).ok();
            }
            if let Some(end_pos) = app.screen_to_text_pos(x, y) {
                let end_line = app.rope.char_to_line(end_pos);
                let end_line_start = app.rope.line_to_char(end_line);
                let end_col = end_pos - end_line_start;

                // Calculate visual column for proper handling of tabs/wide chars
                let rope_slice = app.rope.slice(..);
                let line_slice = rope_slice.line(end_line);
                let visual_col = char_idx_to_visual_col(line_slice, end_col);

                if let Some(block_sel) = &mut app.block_selection {
                    // Extend existing block selection
                    block_sel.extend_to(end_line, end_col, visual_col);

                    // Update helix selection to match block selection
                    app.selection = block_sel.to_selection(&app.rope);

                    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open("/Users/jack/chonker7_debug.log") {
                        let ((min_line, min_col), (max_line, max_col)) = block_sel.visual_bounds();
                        writeln!(file, "[MOUSE] Block selection: lines {}-{}, cols {}-{}",
                            min_line, max_line, min_col, max_col).ok();
                    }
                } else {
                    // This shouldn't happen if click properly starts a block selection
                    // but handle it gracefully - for drag we always create block selection
                    app.block_selection = Some(BlockSelection::new(end_line, end_col));
                }

                mouse_state.is_dragging = true;
                app.needs_redraw = true;
            }
        }

        MouseEvent { button: Some(crate::kitty_native::MouseButton::Left), is_press: true, is_drag: false, x, y, modifiers, .. } => {
            // Debug log click position
            if let Ok(mut file) = OpenOptions::new().create(true).append(true).open("/Users/jack/chonker7_debug.log") {
                writeln!(file, "[MOUSE] Left click at ({}, {}) with modifiers: alt={}", x, y, modifiers.alt).ok();
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
                    // Single click: move cursor and start block selection
                    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open("/Users/jack/chonker7_debug.log") {
                        writeln!(file, "[MOUSE] Single click - moving cursor to pos {}", pos).ok();
                        writeln!(file, "[MOUSE] Old selection: {:?}", app.selection).ok();
                    }

                    // Only start block selection if Alt is held
                    let line = app.rope.char_to_line(pos);
                    let line_start = app.rope.line_to_char(line);
                    let col = pos - line_start;

                    if modifiers.alt {
                        // Calculate visual column for proper handling of tabs/wide chars
                        let rope_slice = app.rope.slice(..);
                        let line_slice = rope_slice.line(line);
                        let visual_col = char_idx_to_visual_col(line_slice, col);

                        // Start a new block selection only with Alt modifier
                        app.block_selection = Some(BlockSelection::new(line, col));
                        if let Some(block_sel) = &mut app.block_selection {
                            block_sel.anchor_visual_col = visual_col;
                            block_sel.cursor_visual_col = visual_col;
                        }
                    } else {
                        // Normal click without Alt - clear any block selection
                        app.block_selection = None;
                    }

                    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open("/Users/jack/chonker7_debug.log") {
                        if modifiers.alt {
                            writeln!(file, "[MOUSE] Starting block selection at col={}, line={}", col, line).ok();
                        } else {
                            writeln!(file, "[MOUSE] Regular click at col={}, line={}", col, line).ok();
                        }
                    }

                    app.selection = Selection::point(pos);

                    // Check if we clicked past end of line to set virtual cursor column
                    let (term_width, _) = crate::kitty_native::KittyTerminal::size().unwrap_or((80, 24));
                    let split_x = term_width / 2;
                    let actual_col = (x as usize).saturating_sub(split_x as usize);
                    let line = app.rope.char_to_line(pos);
                    let line_str = app.rope.line(line);
                    let line_chars = line_str.len_chars();

                    // Always store the virtual cursor column to maintain consistency
                    app.virtual_cursor_col = Some(actual_col);
                    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open("/Users/jack/chonker7_debug.log") {
                        writeln!(file, "[MOUSE] Set virtual_cursor_col={} (line_chars={})", actual_col, line_chars).ok();
                    }

                    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open("/Users/jack/chonker7_debug.log") {
                        writeln!(file, "[MOUSE] New selection: {:?}", app.selection).ok();
                    }

                    mouse_state.last_click = Some(now);
                    mouse_state.last_click_pos = Some((x, y));

                    // Set dragging to prepare for potential drag
                    mouse_state.is_dragging = true;  // Set to true to catch motion events
                }

                app.needs_redraw = true;

                // Force update of the edit display
                if let Some(renderer) = &mut app.edit_display {
                    renderer.update_from_rope(&app.rope);
                    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open("/Users/jack/chonker7_debug.log") {
                        writeln!(file, "[MOUSE] Updated renderer after cursor move").ok();
                    }
                }
            } else {
                // Debug log when click is outside text area
                if let Ok(mut file) = OpenOptions::new().create(true).append(true).open("/Users/jack/chonker7_debug.log") {
                    writeln!(file, "[MOUSE] Click at ({}, {}) is outside text area or couldn't convert", x, y).ok();
                }
            }
        }

        MouseEvent { button: Some(crate::kitty_native::MouseButton::Left), is_press: false, .. } => {
            // Left button release - end drag but keep the block selection visible
            mouse_state.is_dragging = false;
            // Block selection remains in app.block_selection
        }

        // Remove the old motion handler - we handle drag with is_drag now

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

        // Ignore other mouse events
        _ => {}
    }

    Ok(())
}