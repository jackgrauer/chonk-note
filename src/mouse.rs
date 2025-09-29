// MOUSE INTERACTION HANDLER
use crate::{App, kitty_native::MouseEvent};
use crate::block_selection::{BlockSelection, char_idx_to_visual_col};
use anyhow::Result;
use helix_core::{Selection, Range, movement};
use std::time::{Duration, Instant};



// DISABLED: Smooth scrolling for trackpad gestures - not active for text pane
// Keeping structure for potential future use with PDF pane
#[allow(dead_code)]
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
    // DISABLED: Trackpad gesture fields - not active for text pane
    #[allow(dead_code)]
    pub scroll_momentum: ScrollMomentum,
    #[allow(dead_code)]
    pub last_scroll_time: Option<Instant>,
    #[allow(dead_code)]
    pub scroll_accumulator_y: f32,  // For sub-line precision
    #[allow(dead_code)]
    pub scroll_accumulator_x: f32,
    #[allow(dead_code)]
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
    // DISABLED: Update momentum and apply friction
    #[allow(dead_code)]
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

    // DISABLED: Add velocity from a scroll event
    #[allow(dead_code)]
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
        let (term_width, _) = crate::kitty_native::KittyTerminal::size().ok()?;

        // Debug log conversion attempt

        // Determine which pane we're in and get the appropriate rope and renderer
        let (rope, renderer, pane_start_x) = if self.app_mode == crate::AppMode::NotesEditor {
            // In Notes mode, we have three sections:
            // 1. Notes list (0-4)
            // 2. Notes editor (4 to half of remaining)
            // 3. Extraction text (half to end)
            let notes_list_width = 4;
            let remaining_width = term_width.saturating_sub(notes_list_width);
            let notes_editor_width = remaining_width / 2;
            let extraction_start_x = notes_list_width + notes_editor_width;

            if x <= notes_list_width {
                // Click is in notes list, not in a text pane
                return None;
            } else if x < extraction_start_x {
                // Click is in notes editor (left text pane)
                (&self.notes_rope, &self.notes_display, notes_list_width)
            } else {
                // Click is in extraction text (right pane)
                (&self.extraction_rope, &self.edit_display, extraction_start_x)
            }
        } else {
            // In PDF mode, we have two panes split down the middle
            let split_x = self.split_position.unwrap_or(term_width / 2);

            if x <= split_x {
                // Click is in PDF pane
                return None;
            }

            // Click is in extraction text pane
            (&self.extraction_rope, &self.edit_display, split_x)
        };

        // Calculate relative position within the pane
        let text_x = x.saturating_sub(pane_start_x);

        if let Some(renderer) = renderer {
            // Account for viewport scrolling - y is already 0-based from kitty_native
            let actual_y = (y as usize) + renderer.viewport_y;
            let actual_x = text_x as usize + renderer.viewport_x;


            // Convert to character position in rope
            let line = actual_y.min(rope.len_lines().saturating_sub(1));
            let line_start = rope.line_to_char(line);
            let line_str = rope.line(line);


            // Find character at x position (simple approach, assumes monospace)
            let mut char_pos = 0;
            let mut display_x = 0;
            for _ch in line_str.chars() {
                if display_x >= actual_x {
                    break;
                }
                char_pos += 1; // Count chars, not bytes
                display_x += 1; // Simplified - assumes 1 char = 1 column
            }

            // Handle clicking past end of line
            let final_pos = if actual_x > line_str.len_chars() {
                // Clicked past end of line - position at line end
                line_start + line_str.len_chars().saturating_sub(1).max(0)
            } else {
                line_start + char_pos.min(line_str.len_chars())
            };


            Some(final_pos)
        } else {
            None
        }
    }
}

// DISABLED: Apply smooth scrolling with sub-line precision
#[allow(dead_code)]
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
            let max_y = app.extraction_rope.len_lines().saturating_sub(20);
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

    // Get terminal width and height for split position and scrolling calculation
    let (term_width, term_height) = crate::kitty_native::KittyTerminal::size()?;
    let current_split = app.split_position.unwrap_or(term_width / 2);

    // Update debug info if enabled
    if app.debug_mode {
        app.last_mouse_pos = (event.x, event.y);

        // Create coordinate system for conversions
        let coord_sys = crate::coordinate_system::CoordinateSystem::new(app, term_width, term_height);

        // Get document coordinates
        let doc_coords = coord_sys.screen_to_document(event.x, event.y);

        // Get viewport info
        let (viewport_x, viewport_y) = if app.app_mode == crate::AppMode::NotesEditor {
            match app.active_pane {
                crate::ActivePane::Left => {
                    app.notes_display.as_ref()
                        .map(|r| (r.viewport_x, r.viewport_y))
                        .unwrap_or((0, 0))
                }
                crate::ActivePane::Right => {
                    app.edit_display.as_ref()
                        .map(|r| (r.viewport_x, r.viewport_y))
                        .unwrap_or((0, 0))
                }
            }
        } else {
            app.edit_display.as_ref()
                .map(|r| (r.viewport_x, r.viewport_y))
                .unwrap_or((0, 0))
        };

        // Get cursor position
        let (cursor_row, cursor_col) = match app.active_pane {
            crate::ActivePane::Left => (app.notes_cursor.row, app.notes_cursor.col),
            crate::ActivePane::Right => (app.extraction_cursor.row, app.extraction_cursor.col),
        };

        app.debug_info = Some(crate::debug_overlay::DebugInfo {
            mouse_screen: (event.x, event.y),
            mouse_document: doc_coords.unwrap_or((0, 0)),
            cursor_grid: (cursor_row, cursor_col),
            viewport: (viewport_x, viewport_y),
            active_pane: format!("{:?}", app.active_pane),
        });
    }

    match event {
        // Handle drag events FIRST before regular clicks
        MouseEvent { is_drag: true, x, y, .. } => {
            // Check if we're dragging the divider
            if app.is_dragging_divider {
                // Update split position based on mouse X, with constraints
                let min_split = term_width / 4;  // Minimum 25% for each pane
                let max_split = term_width * 3 / 4;  // Maximum 75% for each pane
                app.split_position = Some(x.max(min_split).min(max_split));
                app.needs_redraw = true;
                return Ok(());
            }
            // Mouse drag - handle selection based on active pane
            // Determine which grid, cursor, rope, selection, and block selection to use
            let (grid, cursor, rope, selection, block_selection) = if app.app_mode == crate::AppMode::NotesEditor {
                match app.active_pane {
                    crate::ActivePane::Left => (&mut app.notes_grid, &mut app.notes_cursor, &app.notes_rope, &mut app.notes_selection, &mut app.notes_block_selection),
                    crate::ActivePane::Right => (&mut app.extraction_grid, &mut app.extraction_cursor, &app.extraction_rope, &mut app.extraction_selection, &mut app.extraction_block_selection),
                }
            } else {
                // In PDF mode, always use extraction
                (&mut app.extraction_grid, &mut app.extraction_cursor, &app.extraction_rope, &mut app.extraction_selection, &mut app.extraction_block_selection)
            };

            // Calculate grid position for drag end
            let pane_start_x = if app.app_mode == crate::AppMode::NotesEditor {
                let notes_list_width = 4;
                let remaining_width = term_width.saturating_sub(notes_list_width);
                let notes_editor_width = remaining_width / 2;
                let extraction_start_x = notes_list_width + notes_editor_width;

                match app.active_pane {
                    crate::ActivePane::Left => notes_list_width,  // Notes editor (no divider)
                    crate::ActivePane::Right => extraction_start_x,  // Extraction text (no divider)
                }
            } else {
                current_split  // In PDF mode (no divider)
            };

            // Calculate the position relative to the pane
            let pane_col = x.saturating_sub(pane_start_x) as usize;
            let pane_row = y as usize;  // Terminal coordinates are already 0-based from kitty

            // Add viewport scroll offset to get the actual document position
            let (grid_col, grid_row) = if app.app_mode == crate::AppMode::NotesEditor {
                match app.active_pane {
                    crate::ActivePane::Left => {
                        // Notes pane - get scroll offset from notes renderer
                        if let Some(renderer) = &app.notes_display {
                            (pane_col + renderer.viewport_x, pane_row + renderer.viewport_y)
                        } else {
                            (pane_col, pane_row)
                        }
                    }
                    crate::ActivePane::Right => {
                        // Extraction pane - get scroll offset from extraction renderer
                        if let Some(renderer) = &app.edit_display {
                            (pane_col + renderer.viewport_x, pane_row + renderer.viewport_y)
                        } else {
                            (pane_col, pane_row)
                        }
                    }
                }
            } else {
                // PDF mode - extraction pane
                if let Some(renderer) = &app.edit_display {
                    (pane_col + renderer.viewport_x, pane_row + renderer.viewport_y)
                } else {
                    (pane_col, pane_row)
                }
            };

            // Move the grid cursor to the drag position
            cursor.move_to(grid_row, grid_col);

            let end_line = grid_row;
            let end_col = grid_col;

                // Handle block selection for both panes
                // Block selection is now supported in both notes and extraction panes
                {
                    // Calculate visual column - but handle case where line doesn't exist
                    let visual_col = if end_line < rope.len_lines() {
                        let rope_slice = rope.slice(..);
                        let line_slice = rope_slice.line(end_line);
                        char_idx_to_visual_col(line_slice, end_col)
                    } else {
                        // Line doesn't exist yet, just use the column directly
                        end_col
                    };

                    if let Some(block_sel) = block_selection {
                        // Extend existing block selection
                        block_sel.extend_to(end_line, end_col, visual_col);

                        // Update helix selection to match block selection
                        *selection = block_sel.to_selection(rope);

                    } else {
                        // No block selection active - use regular selection extension
                        // The selection anchor was set during the initial click
                        let anchor = selection.primary().anchor;
                        // Try to get char position from grid cursor
                        if let Some(end_pos) = cursor.to_char_offset(grid) {
                            *selection = Selection::single(anchor, end_pos);
                        }
                    }
                }

                mouse_state.is_dragging = true;
                app.needs_redraw = true;
        }

        MouseEvent { button: Some(crate::kitty_native::MouseButton::Left), is_press: true, is_drag: false, x, y, modifiers, .. } => {
            // Debug log click position

            // In Notes mode, check for clicks on notes list (far left, 4 chars wide)
            if app.app_mode == crate::AppMode::NotesEditor && x <= 4 {
                // Calculate which note was clicked based on display position
                // y is already 0-based from kitty_native
                let clicked_row = y as usize;
                if clicked_row < app.notes_list.len() {
                    // First, save the current note's changes back to the list
                    if let Some(ref mut notes_mode) = app.notes_mode {
                        if let Some(ref current_note) = notes_mode.current_note {
                            // Find the current note in the list and update it
                            for note in app.notes_list.iter_mut() {
                                if note.id == current_note.id {
                                    // Update the note's content with the current editor content
                                    note.content = app.notes_rope.to_string();
                                    break;
                                }
                            }
                        }
                    }

                    // Now select and load the clicked note
                    app.selected_note_index = clicked_row;

                    if let Some(ref mut notes_mode) = app.notes_mode {
                        let selected_note = app.notes_list[clicked_row].clone();

                        // Load the note content
                        app.notes_rope = helix_core::Rope::from(selected_note.content.as_str());
                        app.notes_selection = helix_core::Selection::point(0);

                        // Update the grid with the new rope!
                        app.notes_grid = crate::virtual_grid::VirtualGrid::new(app.notes_rope.clone());
                        app.notes_cursor = crate::grid_cursor::GridCursor::new();

                        notes_mode.current_note = Some(selected_note.clone());

                        // Update the display
                        if let Some(renderer) = &mut app.notes_display {
                            renderer.update_from_rope(&app.notes_rope);
                        }

                        // Switch focus to notes editor
                        app.switch_active_pane(crate::ActivePane::Left);
                        app.status_message = format!("Opened: {}", selected_note.title);
                    }

                    app.needs_redraw = true;
                    return Ok(());
                }
            }

            // Check for zoom button clicks
            // PDF zoom buttons are in top-right of left pane
            let pdf_pane_width = current_split - 1;
            if y == 0 {  // Top row
                // PDF zoom minus button
                if x >= pdf_pane_width - 12 && x <= pdf_pane_width - 10 {
                    app.pdf_zoom = (app.pdf_zoom - 0.1).max(0.5);  // Min 50%
                    // Reload PDF at new zoom level
                    let _ = app.load_pdf_page().await;
                    app.needs_redraw = true;
                    return Ok(());
                }
                // PDF zoom plus button
                if x >= pdf_pane_width - 3 && x <= pdf_pane_width - 1 {
                    app.pdf_zoom = (app.pdf_zoom + 0.1).min(3.0);  // Max 300%
                    // Reload PDF at new zoom level
                    let _ = app.load_pdf_page().await;
                    app.needs_redraw = true;
                    return Ok(());
                }

            }

            // Check if click is on the divider (within 2 columns of the split position)
            if x >= current_split.saturating_sub(2) && x <= current_split + 2 {
                app.is_dragging_divider = true;
                app.needs_redraw = true;
                return Ok(());
            }

            // Determine which pane was clicked and switch to it
            if app.app_mode == crate::AppMode::NotesEditor {
                // In Notes mode, we have three sections:
                // 1. Notes list (0-4)
                // 2. Notes editor (4 to half of remaining)
                // 3. Extraction text (half to end)
                let notes_list_width = 4;
                let remaining_width = term_width.saturating_sub(notes_list_width);
                let notes_editor_width = remaining_width / 2;
                let extraction_start_x = notes_list_width + notes_editor_width;

                if x > notes_list_width && x < extraction_start_x {
                    // Clicked in notes editor
                    app.switch_active_pane(crate::ActivePane::Left);
                } else if x >= extraction_start_x {
                    // Clicked in extraction text
                    app.switch_active_pane(crate::ActivePane::Right);
                }
                // If x <= notes_list_width, we already handled it above
            } else {
                // In PDF mode, use simple split
                if x < current_split {
                    // Clicked in PDF pane - no action needed
                } else {
                    // Clicked in extraction pane
                    app.switch_active_pane(crate::ActivePane::Right);
                }
            }

            // Left click - set cursor position using grid
            // First determine which pane was clicked
            let (grid, cursor) = if app.app_mode == crate::AppMode::NotesEditor {
                let notes_list_width = 4;
                let remaining_width = term_width.saturating_sub(notes_list_width);
                let notes_editor_width = remaining_width / 2;
                let extraction_start_x = notes_list_width + notes_editor_width;

                if x > notes_list_width && x < extraction_start_x {
                    // Clicked in notes editor
                    (&mut app.notes_grid, &mut app.notes_cursor)
                } else if x >= extraction_start_x {
                    // Clicked in extraction text
                    (&mut app.extraction_grid, &mut app.extraction_cursor)
                } else {
                    // Clicked in notes list (handled elsewhere)
                    return Ok(());
                }
            } else {
                // In PDF mode
                if x < current_split {
                    // Clicked in PDF pane
                    return Ok(());
                } else {
                    // Clicked in extraction pane
                    (&mut app.extraction_grid, &mut app.extraction_cursor)
                }
            };

            // Calculate grid position (allow clicking anywhere!)
            let pane_start_x = if app.app_mode == crate::AppMode::NotesEditor {
                let notes_list_width = 4;
                let remaining_width = term_width.saturating_sub(notes_list_width);
                let notes_editor_width = remaining_width / 2;
                let extraction_start_x = notes_list_width + notes_editor_width;

                if x > notes_list_width && x < extraction_start_x {
                    notes_list_width  // Notes editor starts right after notes list (no divider)
                } else {
                    extraction_start_x  // Extraction text starts right after notes editor (no divider)
                }
            } else {
                current_split  // In PDF mode (no divider)
            };

            // Calculate the position relative to the pane
            let pane_col = x.saturating_sub(pane_start_x) as usize;
            let pane_row = y as usize;  // Terminal coordinates are already 0-based from kitty

            // Add viewport scroll offset to get the actual document position
            let (grid_col, grid_row) = if app.app_mode == crate::AppMode::NotesEditor {
                match app.active_pane {
                    crate::ActivePane::Left => {
                        // Notes pane - get scroll offset from notes renderer
                        if let Some(renderer) = &app.notes_display {
                            (pane_col + renderer.viewport_x, pane_row + renderer.viewport_y)
                        } else {
                            (pane_col, pane_row)
                        }
                    }
                    crate::ActivePane::Right => {
                        // Extraction pane - get scroll offset from extraction renderer
                        if let Some(renderer) = &app.edit_display {
                            (pane_col + renderer.viewport_x, pane_row + renderer.viewport_y)
                        } else {
                            (pane_col, pane_row)
                        }
                    }
                }
            } else {
                // PDF mode - extraction pane
                if let Some(renderer) = &app.edit_display {
                    (pane_col + renderer.viewport_x, pane_row + renderer.viewport_y)
                } else {
                    (pane_col, pane_row)
                }
            };

            // Move the grid cursor to the clicked position (allows virtual space!)
            cursor.move_to(grid_row, grid_col);

            // Minimal logging - just one line, doesn't change behavior
            crate::minimal_debug::log_click(x, y, grid_row, grid_col);

            // Log coordinate transformation if debugging
            if app.debug_mode {
                crate::logger::log_coordinate_event(
                    "Click",
                    (x, y),
                    (pane_col, pane_row),
                    (grid_col, grid_row),
                    (if app.app_mode == crate::AppMode::NotesEditor {
                        match app.active_pane {
                            crate::ActivePane::Left => {
                                app.notes_display.as_ref()
                                    .map(|r| (r.viewport_x, r.viewport_y))
                                    .unwrap_or((0, 0))
                            }
                            crate::ActivePane::Right => {
                                app.edit_display.as_ref()
                                    .map(|r| (r.viewport_x, r.viewport_y))
                                    .unwrap_or((0, 0))
                            }
                        }
                    } else {
                        app.edit_display.as_ref()
                            .map(|r| (r.viewport_x, r.viewport_y))
                            .unwrap_or((0, 0))
                    })
                );
            }

            // Update the selection based on cursor position
            // Even if we're in virtual space, we need to handle clicks
            let now = Instant::now();

            if let Some(pos) = cursor.to_char_offset(grid) {
                // Cursor is at a valid text position
                // Debug log text position

                // Check for double-click
                let is_double_click = if let (Some(last_time), Some((last_x, last_y))) =
                    (mouse_state.last_click, mouse_state.last_click_pos) {
                    now.duration_since(last_time) < mouse_state.double_click_threshold &&
                    (x, y) == (last_x, last_y)
                } else {
                    false
                };

                // Determine which rope, selection, and block selection to use based on pane
                let (rope, selection, block_selection) = if app.app_mode == crate::AppMode::NotesEditor {
                    match app.active_pane {
                        crate::ActivePane::Left => (&app.notes_rope, &mut app.notes_selection, &mut app.notes_block_selection),
                        crate::ActivePane::Right => (&app.extraction_rope, &mut app.extraction_selection, &mut app.extraction_block_selection),
                    }
                } else {
                    // In PDF mode, always use extraction rope
                    (&app.extraction_rope, &mut app.extraction_selection, &mut app.extraction_block_selection)
                };

                if is_double_click {
                    // Double-click: select word
                    let rope_slice = rope.slice(..);
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

                    *selection = Selection::single(word_start, range.head);
                    mouse_state.last_click = None; // Reset to avoid triple-click
                } else {
                    // Single click: move cursor and start block selection

                    // Always start block selection on click (no Alt required!)
                    let line = rope.char_to_line(pos);
                    let line_start = rope.line_to_char(line);
                    let col = pos - line_start;

                    // Calculate visual column for proper handling of tabs/wide chars
                    let rope_slice = rope.slice(..);
                    let line_slice = rope_slice.line(line);
                    let visual_col = char_idx_to_visual_col(line_slice, col);

                    // Start a new block selection on every click
                    *block_selection = Some(BlockSelection::new(line, col));
                    if let Some(block_sel) = block_selection {
                        block_sel.anchor_visual_col = visual_col;
                        block_sel.cursor_visual_col = visual_col;
                    }


                    *selection = Selection::point(pos);

                    // Virtual cursor column is no longer used in dual-pane mode
                    // Each pane tracks its own cursor independently


                    mouse_state.last_click = Some(now);
                    mouse_state.last_click_pos = Some((x, y));

                    // Set dragging to prepare for potential drag
                    mouse_state.is_dragging = true;  // Set to true to catch motion events
                }

                app.needs_redraw = true;

                // Force update of the appropriate display
                if app.app_mode == crate::AppMode::NotesEditor {
                    match app.active_pane {
                        crate::ActivePane::Left => {
                            if let Some(renderer) = &mut app.notes_display {
                                renderer.update_from_rope(&app.notes_rope);
                            }
                        }
                        crate::ActivePane::Right => {
                            if let Some(renderer) = &mut app.edit_display {
                                renderer.update_from_rope(&app.extraction_rope);
                            }
                        }
                    }
                } else {
                    if let Some(renderer) = &mut app.edit_display {
                        renderer.update_from_rope(&app.extraction_rope);
                    }
                }
            } else {
                // Cursor is in virtual space - still handle the click!
                let now = Instant::now();

                // We can still handle double-click in virtual space
                let is_double_click = if let (Some(last_time), Some((last_x, last_y))) =
                    (mouse_state.last_click, mouse_state.last_click_pos) {
                    now.duration_since(last_time) < mouse_state.double_click_threshold &&
                    (x as i32 - last_x as i32).abs() <= 2 &&
                    (y as i32 - last_y as i32).abs() <= 2
                } else {
                    false
                };

                // Get the appropriate rope and selection based on which pane was clicked
                let (rope, selection, block_selection) = if app.app_mode == crate::AppMode::NotesEditor {
                    match app.active_pane {
                        crate::ActivePane::Left => (&mut app.notes_rope, &mut app.notes_selection, &mut app.notes_block_selection),
                        crate::ActivePane::Right => (&mut app.extraction_rope, &mut app.extraction_selection, &mut app.extraction_block_selection),
                    }
                } else {
                    (&mut app.extraction_rope, &mut app.extraction_selection, &mut app.extraction_block_selection)
                };

                if is_double_click {
                    // For double-click in virtual space, select the whole line
                    let line_idx = cursor.row.min(rope.len_lines().saturating_sub(1));
                    let line_start = rope.line_to_char(line_idx);
                    let line = rope.line(line_idx);
                    let line_end = line_start + line.len_chars();
                    *selection = Selection::single(line_start, line_end);
                    mouse_state.last_click = None; // Reset to avoid triple-click
                } else {
                    // Single click in virtual space
                    // Set the selection to the closest valid position (end of line)
                    let line_idx = cursor.row.min(rope.len_lines().saturating_sub(1));
                    let line_start = rope.line_to_char(line_idx);
                    let line = rope.line(line_idx);
                    let line_end = line_start + line.len_chars().saturating_sub(1);
                    *selection = Selection::point(line_end);

                    // Always start block selection (no Alt required!)
                    *block_selection = Some(BlockSelection::new(cursor.row, cursor.col));

                    mouse_state.last_click = Some(now);
                    mouse_state.last_click_pos = Some((x, y));
                    mouse_state.is_dragging = true;  // Set to true to catch motion events
                }

                app.needs_redraw = true;

                // Force update of the appropriate display
                if app.app_mode == crate::AppMode::NotesEditor {
                    match app.active_pane {
                        crate::ActivePane::Left => {
                            if let Some(renderer) = &mut app.notes_display {
                                renderer.update_from_rope(&app.notes_rope);
                            }
                        }
                        crate::ActivePane::Right => {
                            if let Some(renderer) = &mut app.edit_display {
                                renderer.update_from_rope(&app.extraction_rope);
                            }
                        }
                    }
                } else {
                    if let Some(renderer) = &mut app.edit_display {
                        renderer.update_from_rope(&app.extraction_rope);
                    }
                }
            }
        }

        MouseEvent { button: Some(crate::kitty_native::MouseButton::Left), is_press: false, .. } => {
            // Left button release - end drag but keep the block selection visible
            mouse_state.is_dragging = false;
            app.is_dragging_divider = false;
            // Block selection remains in app.block_selection
        }

        // Remove the old motion handler - we handle drag with is_drag now

        MouseEvent { button: Some(crate::kitty_native::MouseButton::ScrollUp), x, modifiers, .. } => {
            if app.app_mode == crate::AppMode::NotesEditor {
                // In Notes mode, determine which pane to scroll
                let notes_list_width = 4;
                let remaining_width = term_width.saturating_sub(notes_list_width);
                let notes_editor_width = remaining_width / 2;
                let extraction_start_x = notes_list_width + notes_editor_width;

                if x <= notes_list_width {
                    // Scrolling in notes list - scroll the list
                    if app.notes_list_scroll > 0 {
                        app.notes_list_scroll = app.notes_list_scroll.saturating_sub(1);
                        app.needs_redraw = true;
                    }
                } else if x < extraction_start_x {
                    // Scrolling in notes editor pane
                    if let Some(renderer) = &mut app.notes_display {
                        if modifiers.shift {
                            // Horizontal scroll with Shift+Scroll
                            renderer.scroll_left(5);
                        } else {
                            // Vertical scroll
                            renderer.scroll_up(3);
                        }
                        app.needs_redraw = true;
                    }
                } else {
                    // Scrolling in extraction text pane
                    if let Some(renderer) = &mut app.edit_display {
                        if modifiers.shift {
                            // Horizontal scroll with Shift+Scroll
                            renderer.scroll_left(5);
                        } else {
                            // Vertical scroll
                            renderer.scroll_up(3);
                        }
                        app.needs_redraw = true;
                    }
                }
            } else {
                // PDF mode - original behavior
                if x <= current_split {
                    // PDF pane scrolling
                    if modifiers.shift {
                        // Horizontal scroll with Shift+Scroll
                        app.pdf_scroll_x = app.pdf_scroll_x.saturating_sub(5);
                    } else {
                        // Vertical scroll
                        app.pdf_scroll_y = app.pdf_scroll_y.saturating_sub(3);
                    }
                    app.needs_redraw = true;
                } else {
                    // Text pane scrolling
                    if let Some(renderer) = &mut app.edit_display {
                        if modifiers.shift {
                            // Horizontal scroll with Shift+Scroll
                            renderer.scroll_left(5);
                        } else {
                            // Vertical scroll
                            renderer.scroll_up(3);
                        }
                        app.needs_redraw = true;
                    }
                }
            }
        }

        MouseEvent { button: Some(crate::kitty_native::MouseButton::ScrollDown), x, modifiers, .. } => {
            if app.app_mode == crate::AppMode::NotesEditor {
                // In Notes mode, determine which pane to scroll
                let notes_list_width = 4;
                let remaining_width = term_width.saturating_sub(notes_list_width);
                let notes_editor_width = remaining_width / 2;
                let extraction_start_x = notes_list_width + notes_editor_width;

                if x <= notes_list_width {
                    // Scrolling in notes list - scroll the list
                    let visible_count = (term_height - 2) as usize;
                    let max_scroll = app.notes_list.len().saturating_sub(visible_count);
                    if app.notes_list_scroll < max_scroll {
                        app.notes_list_scroll += 1;
                        app.needs_redraw = true;
                    }
                } else if x < extraction_start_x {
                    // Scrolling in notes editor pane
                    if let Some(renderer) = &mut app.notes_display {
                        if modifiers.shift {
                            // Horizontal scroll with Shift+Scroll
                            renderer.scroll_right(5);
                        } else {
                            // Vertical scroll
                            renderer.scroll_down(3);
                        }
                        app.needs_redraw = true;
                    }
                } else {
                    // Scrolling in extraction text pane
                    if let Some(renderer) = &mut app.edit_display {
                        if modifiers.shift {
                            // Horizontal scroll with Shift+Scroll
                            renderer.scroll_right(5);
                        } else {
                            // Vertical scroll
                            renderer.scroll_down(3);
                        }
                        app.needs_redraw = true;
                    }
                }
            } else {
                // PDF mode - original behavior
                if x <= current_split {
                    // PDF pane scrolling
                    let (_term_width, term_height) = crate::kitty_native::KittyTerminal::size()?;
                    let pdf_viewport_height = term_height.saturating_sub(3);
                    let pdf_viewport_width = current_split.saturating_sub(3);

                    if modifiers.shift {
                        // Horizontal scroll with Shift+Scroll
                        let max_scroll_x = app.pdf_full_width.saturating_sub(pdf_viewport_width);
                        app.pdf_scroll_x = (app.pdf_scroll_x + 5).min(max_scroll_x);
                    } else {
                        // Vertical scroll
                        let max_scroll_y = app.pdf_full_height.saturating_sub(pdf_viewport_height);
                        app.pdf_scroll_y = (app.pdf_scroll_y + 3).min(max_scroll_y);
                    }
                    app.needs_redraw = true;
                } else {
                    // Text pane scrolling
                    if let Some(renderer) = &mut app.edit_display {
                        if modifiers.shift {
                            // Horizontal scroll with Shift+Scroll
                            renderer.scroll_right(5);
                        } else {
                            // Vertical scroll
                            renderer.scroll_down(3);
                        }
                        app.needs_redraw = true;
                    }
                }
            }
        }

        // Horizontal swipe gestures
        MouseEvent { button: Some(crate::kitty_native::MouseButton::ScrollLeft), x, .. } => {
            if app.app_mode == crate::AppMode::NotesEditor {
                // In Notes mode, determine which pane to scroll
                let notes_list_width = 4;
                let remaining_width = term_width.saturating_sub(notes_list_width);
                let notes_editor_width = remaining_width / 2;
                let extraction_start_x = notes_list_width + notes_editor_width;

                if x <= notes_list_width {
                    // Notes list doesn't need horizontal scrolling
                } else if x < extraction_start_x {
                    // Scrolling in notes editor pane - scroll left
                    if let Some(renderer) = &mut app.notes_display {
                        renderer.scroll_left(5);
                        app.needs_redraw = true;
                    }
                } else {
                    // Scrolling in extraction text pane - scroll left
                    if let Some(renderer) = &mut app.edit_display {
                        renderer.scroll_left(5);
                        app.needs_redraw = true;
                    }
                }
            } else {
                // PDF mode - original behavior
                if x <= current_split {
                    // PDF pane - scroll left (decrease scroll_x)
                    app.pdf_scroll_x = app.pdf_scroll_x.saturating_sub(5);
                    app.needs_redraw = true;
                } else {
                    // Text pane - scroll left
                    if let Some(renderer) = &mut app.edit_display {
                        renderer.scroll_left(5);
                        app.needs_redraw = true;
                    }
                }
            }
        }

        MouseEvent { button: Some(crate::kitty_native::MouseButton::ScrollRight), x, .. } => {
            if app.app_mode == crate::AppMode::NotesEditor {
                // In Notes mode, determine which pane to scroll
                let notes_list_width = 4;
                let remaining_width = term_width.saturating_sub(notes_list_width);
                let notes_editor_width = remaining_width / 2;
                let extraction_start_x = notes_list_width + notes_editor_width;

                if x <= notes_list_width {
                    // Notes list doesn't need horizontal scrolling
                } else if x < extraction_start_x {
                    // Scrolling in notes editor pane - scroll right
                    if let Some(renderer) = &mut app.notes_display {
                        renderer.scroll_right(5);
                        app.needs_redraw = true;
                    }
                } else {
                    // Scrolling in extraction text pane - scroll right
                    if let Some(renderer) = &mut app.edit_display {
                        renderer.scroll_right(5);
                        app.needs_redraw = true;
                    }
                }
            } else {
                // PDF mode - original behavior
                if x <= current_split {
                    // PDF pane - scroll right (increase scroll_x)
                    let pdf_viewport_width = current_split.saturating_sub(3);
                    let max_scroll_x = app.pdf_full_width.saturating_sub(pdf_viewport_width);
                    app.pdf_scroll_x = (app.pdf_scroll_x + 5).min(max_scroll_x);
                    app.needs_redraw = true;
                } else {
                    // Text pane - scroll right
                    if let Some(renderer) = &mut app.edit_display {
                        renderer.scroll_right(5);
                        app.needs_redraw = true;
                    }
                }
            }
        }

        // Ignore other mouse events
        _ => {}
    }

    Ok(())
}