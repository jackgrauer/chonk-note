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
            // In Notes mode, we only have:
            // 1. Notes list (0-4)
            // 2. Notes editor (everything else)
            let notes_list_width = 4;

            if x < notes_list_width {
                // Click is in notes list, not in a text pane
                return None;
            } else {
                // Click is in notes editor
                (&self.notes_rope, &self.notes_display, notes_list_width)
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


    match event {
        // Handle drag events FIRST before regular clicks
        MouseEvent { is_drag: true, x, y, .. } => {
            // Check if we're dragging the divider
            if app.is_dragging_divider {
                // Update split position based on mouse X, with constraints
                if app.app_mode == crate::AppMode::NotesEditor {
                    // In Notes mode, split is relative to notes list width
                    let notes_list_width = 4;
                    let remaining_width = term_width.saturating_sub(notes_list_width);
                    let min_split = remaining_width / 4;  // Minimum 25% for notes editor
                    let max_split = remaining_width * 3 / 4;  // Maximum 75% for notes editor
                    let relative_x = x.saturating_sub(notes_list_width);
                    app.split_position = Some(relative_x.max(min_split).min(max_split));
                } else {
                    // PDF mode - split is absolute
                    let min_split = term_width / 4;  // Minimum 25% for each pane
                    let max_split = term_width * 3 / 4;  // Maximum 75% for each pane
                    app.split_position = Some(x.max(min_split).min(max_split));
                }
                app.needs_redraw = true;
                return Ok(());
            }
            // USE THE COORDINATE SYSTEM for drag too!
            // Calculate coordinates FIRST before borrowing mutably
            let coord_sys = crate::coordinate_system::CoordinateSystem::new(app, term_width, term_height);
            let coords = match coord_sys.process_click(x, y) {
                Some(c) => c,
                None => return Ok(()), // Drag outside valid area
            };

            // Check if we've dragged into a different pane - if so, ignore this drag event
            // This prevents selection from "flipping" when dragging past pane edge
            let current_pane_matches = if app.app_mode == crate::AppMode::NotesEditor {
                // In notes mode, only NotesEditor pane exists (ignore NotesList)
                coords.pane == crate::coordinate_system::Pane::NotesEditor
            } else {
                coords.pane == crate::coordinate_system::Pane::Extraction
            };

            // If drag moved into different pane or divider, ignore it
            if !current_pane_matches {
                return Ok(());
            }

            let end_line = coords.grid.1;
            let end_col = coords.grid.0;

            // NOW determine which grid, cursor, rope, selection, and block selection to use
            let (grid, cursor, rope, selection, block_selection) = if app.app_mode == crate::AppMode::NotesEditor {
                match app.active_pane {
                    crate::ActivePane::Left => (&mut app.notes_grid, &mut app.notes_cursor, &app.notes_rope, &mut app.notes_selection, &mut app.notes_block_selection),
                    crate::ActivePane::Right => (&mut app.extraction_grid, &mut app.extraction_cursor, &app.extraction_rope, &mut app.extraction_selection, &mut app.extraction_block_selection),
                }
            } else {
                // In PDF mode, always use extraction
                (&mut app.extraction_grid, &mut app.extraction_cursor, &app.extraction_rope, &mut app.extraction_selection, &mut app.extraction_block_selection)
            };

            // Move cursor to drag position
            cursor.move_to(end_line, end_col);

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
                        // Prevent selection from extending past pane boundaries
                        // This stops the "flip" when dragging past the edge
                        let clamped_col = end_col;  // Already clamped by coordinate system

                        // Extend existing block selection
                        block_sel.extend_to(end_line, clamped_col, visual_col);

                        // Update helix selection to match block selection
                        *selection = block_sel.to_selection(rope);

                    } else if mouse_state.is_dragging {
                        // Start block selection on first drag movement
                        *block_selection = Some(BlockSelection::new(end_line, end_col));
                        if let Some(block_sel) = block_selection {
                            block_sel.anchor_visual_col = visual_col;
                            block_sel.cursor_visual_col = visual_col;
                        }

                        // Also set selection
                        if let Some(pos) = cursor.to_char_offset(grid) {
                            *selection = Selection::point(pos);
                        }
                    }
                }

                mouse_state.is_dragging = true;
                app.needs_redraw = true;
        }

        MouseEvent { button: Some(crate::kitty_native::MouseButton::Left), is_press: true, is_drag: false, x, y, modifiers, .. } => {
            // Debug log click position

            // In Notes mode, check for clicks on notes list (far left)
            if app.app_mode == crate::AppMode::NotesEditor {
                // Check for click on top yellow bar
                if y == 0 {
                    // Clicked on top bar - enable editing
                    if !app.notes_list.is_empty() {
                        app.editing_title = true;
                        app.title_buffer = app.notes_list[app.selected_note_index].title.clone();
                        app.needs_redraw = true;
                    }
                    return Ok(());
                }

                // Use coordinate system to determine which pane was clicked
                let coord_sys = crate::coordinate_system::CoordinateSystem::new(app, term_width, term_height);
                let clicked_pane = coord_sys.which_pane(x);

                // Check if clicked on notes list sidebar
                if clicked_pane == Some(crate::coordinate_system::Pane::NotesList) && y > 0 {
                    let clicked_row = (y.saturating_sub(1)) as usize; // Account for top bar offset

                    // If sidebar is expanded, clicking anywhere on it should select a note if valid
                    if app.sidebar_expanded {
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
                        }

                        // Always collapse sidebar after any click when expanded
                        app.sidebar_expanded = false;
                        app.needs_redraw = true;
                        return Ok(());
                    } else {
                        // If collapsed sidebar, expand when clicking in sidebar
                        app.sidebar_expanded = true;
                        app.needs_redraw = true;
                        return Ok(());
                    }
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

            // Check if click is exactly on the divider column (much more precise now)
            let divider_x = if app.app_mode == crate::AppMode::NotesEditor {
                // In Notes mode, divider is at notes_list + notes_editor_width
                let notes_list_width = 4;
                notes_list_width + current_split
            } else {
                // In PDF mode, divider is at current_split
                current_split
            };

            if x == divider_x {
                app.is_dragging_divider = true;
                app.needs_redraw = true;
                return Ok(());
            }

            // Determine which pane was clicked and switch to it
            if app.app_mode == crate::AppMode::NotesEditor {
                // In Notes mode, we only have:
                // 1. Notes list (0-4)
                // 2. Notes editor (everything else)
                let notes_list_width = 4;

                if x >= notes_list_width {
                    // Clicked in notes editor
                    app.switch_active_pane(crate::ActivePane::Left);
                }
                // If x < notes_list_width, we already handled it above
            } else {
                // In PDF mode, use simple split
                if x < current_split {
                    // Clicked in PDF pane - no action needed
                } else {
                    // Clicked in extraction pane
                    app.switch_active_pane(crate::ActivePane::Right);
                }
            }

            // USE THE COORDINATE SYSTEM - ALL MATH IN ONE PLACE
            let coord_sys = crate::coordinate_system::CoordinateSystem::new(app, term_width, term_height);

            // Process click through the abstraction layer
            let coords = match coord_sys.process_click(x, y) {
                Some(c) => c,
                None => return Ok(()), // Click was outside valid area
            };

            // Check which pane and get the right cursor/grid
            use crate::coordinate_system::Pane;
            let (grid, cursor) = match coords.pane {
                Pane::NotesEditor => (&mut app.notes_grid, &mut app.notes_cursor),
                Pane::Extraction => (&mut app.extraction_grid, &mut app.extraction_cursor),
                Pane::NotesList | Pane::Pdf => return Ok(()), // These are handled elsewhere
            };

            // Move cursor to the calculated grid position
            cursor.move_to(coords.grid.1, coords.grid.0);

            // Update the selection based on cursor position
            // Even if we're in virtual space, we need to handle clicks
            let now = Instant::now();

            // Try to get text position, but handle virtual space too
            let char_pos = cursor.to_char_offset(grid);

            if let Some(pos) = char_pos {
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
                    // In notes mode, always use notes (no extraction pane exists)
                    (&app.notes_rope, &mut app.notes_selection, &mut app.notes_block_selection)
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

                    // Clear any existing block selection first
                    *block_selection = None;

                    // Don't start block selection on single click - only on drag
                    // This prevents ghost cursors from appearing


                    *selection = Selection::point(pos);

                    // Virtual cursor column is no longer used in dual-pane mode
                    // Each pane tracks its own cursor independently


                    mouse_state.last_click = Some(now);
                    mouse_state.last_click_pos = Some((x, y));

                    // Set dragging to prepare for potential drag
                    mouse_state.is_dragging = true;  // Set to true to catch motion events
                }

                app.needs_redraw = true;
                // Removed duplicate display update to prevent flicker - needs_redraw is sufficient
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
                    // For virtual space, we need to handle clicking beyond existing text
                    if cursor.row < rope.len_lines() {
                        // Clicking on a line that exists
                        let line_start = rope.line_to_char(cursor.row);
                        let line = rope.line(cursor.row);
                        let line_end = line_start + line.len_chars().saturating_sub(1);
                        *selection = Selection::point(line_end);
                    } else {
                        // Clicking beyond all text - position at end of document
                        *selection = Selection::point(rope.len_chars());
                    }

                    // Clear any existing block selection on single click
                    *block_selection = None;

                    mouse_state.last_click = Some(now);
                    mouse_state.last_click_pos = Some((x, y));
                    mouse_state.is_dragging = true;  // Set to true to catch motion events
                }

                app.needs_redraw = true;
                // Removed duplicate display update to prevent flicker - needs_redraw is sufficient
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
                // In Notes mode, only have notes list and notes editor
                let notes_list_width = 4;

                if x <= notes_list_width {
                    // Scrolling in notes list - scroll the list
                    if app.notes_list_scroll > 0 {
                        app.notes_list_scroll = app.notes_list_scroll.saturating_sub(1);
                        app.needs_redraw = true;
                    }
                } else {
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
                // In Notes mode, only have notes list and notes editor
                let notes_list_width = 4;

                if x <= notes_list_width {
                    // Scrolling in notes list - scroll the list
                    let visible_count = (term_height - 2) as usize;
                    let max_scroll = app.notes_list.len().saturating_sub(visible_count);
                    if app.notes_list_scroll < max_scroll {
                        app.notes_list_scroll += 1;
                        app.needs_redraw = true;
                    }
                } else {
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
                // In Notes mode, only have notes list and notes editor
                let notes_list_width = 4;

                if x <= notes_list_width {
                    // Notes list doesn't need horizontal scrolling
                } else {
                    // Scrolling in notes editor pane - scroll left
                    if let Some(renderer) = &mut app.notes_display {
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
                // In Notes mode, only have notes list and notes editor
                let notes_list_width = 4;

                if x <= notes_list_width {
                    // Notes list doesn't need horizontal scrolling
                } else {
                    // Scrolling in notes editor pane - scroll right
                    if let Some(renderer) = &mut app.notes_display {
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