// CHONK-NOTE - Lightweight notes editor with chunked grid
use anyhow::Result;
use std::io::{self, Write};

// Embed hamster emoji PNG at compile time
const HAMSTER_PNG: &[u8] = include_bytes!("../assets/hamster.png");

mod config;
mod keyboard;
mod kitty_native;
mod mouse;
mod notes;
mod text_buffer;
mod undo;

use kitty_native::KittyTerminal;
use mouse::MouseState;
use text_buffer::TextBuffer;
use config::{layout, timing, colors, rgb_bg, rgb_fg};

pub struct App {
    // Notes manager
    pub notes_manager: notes::NotesManager,
    pub current_note: Option<notes::Note>,

    // Simple text buffer
    pub text_buffer: TextBuffer,
    pub cursor_row: usize,
    pub cursor_col: usize,

    // Viewport scrolling
    pub viewport_row: usize,
    pub viewport_col: usize,

    // Notes list sidebar
    pub notes_list: Vec<notes::Note>,
    pub selected_note_index: usize,
    pub notes_list_scroll: usize,
    pub sidebar_expanded: bool,
    pub editing_title: bool,
    pub title_buffer: String,

    // App state
    pub status_message: String,
    pub exit_requested: bool,
    pub needs_redraw: bool,
    pub force_immediate_render: bool,

    // Delete confirmation
    pub delete_confirmation_note: Option<usize>,

    // Auto-save debouncing
    pub dirty: bool,
    pub last_save_time: std::time::Instant,

    // Undo/Redo system
    pub undo_stack: undo::UndoStack,

    // Search functionality
    pub search_mode: bool,
    pub search_query: String,
    pub search_results: Vec<(usize, usize)>, // (row, col) positions
    pub current_search_index: usize,

    // Menu bar and settings
    pub soft_wrap_paste: bool,
    pub notes_menu_expanded: bool,
    pub settings_panel_expanded: bool,
    pub help_menu_expanded: bool,
}

impl App {
    pub fn new() -> Result<Self> {
        let notes_manager = notes::NotesManager::new()?;
        let mut notes_list = notes_manager.list_notes()?;
        let mut text_buffer = TextBuffer::new();
        let mut current_note = None;

        // If no notes exist, create two default notes
        if notes_list.is_empty() {
            let note1 = notes_manager.create_note(
                "Welcome to Chonk-Note",
                "This is your first note.\nYou can edit this text.\n\nPress Ctrl+N to create a new note.\nPress Ctrl+S to save.\nPress Ctrl+Q to quit."
            )?;
            let note2 = notes_manager.create_note(
                "Second Note",
                "This is your second note.\nYou can switch between notes using the Notes menu.\n\nClick on \"Notes\" at the top to see all your notes!"
            )?;
            notes_list = vec![note1.clone(), note2];
            text_buffer = TextBuffer::from_string(&note1.content);
            current_note = Some(note1);
        } else {
            // Load the first note if available
            let first_note = notes_list[0].clone();
            text_buffer = TextBuffer::from_string(&first_note.content);
            current_note = Some(first_note);
        }

        Ok(Self {
            notes_manager,
            current_note,
            text_buffer,
            cursor_row: 0,
            cursor_col: 0,
            viewport_row: 0,
            viewport_col: 0,
            notes_list,
            selected_note_index: 0,
            notes_list_scroll: 0,
            sidebar_expanded: false,
            editing_title: false,
            title_buffer: String::new(),
            status_message: "Ready".to_string(),
            exit_requested: false,
            needs_redraw: true,
            force_immediate_render: false,
            delete_confirmation_note: None,
            dirty: false,
            last_save_time: std::time::Instant::now(),
            undo_stack: undo::UndoStack::new(100), // Max 100 undo levels
            search_mode: false,
            search_query: String::new(),
            search_results: Vec::new(),
            current_search_index: 0,
            soft_wrap_paste: true, // Default to ON
            notes_menu_expanded: false,
            settings_panel_expanded: false,
            help_menu_expanded: false,
        })
    }

    /// Update viewport to keep cursor visible
    pub fn clamp_cursor_to_visible_area(&mut self, sidebar_width: u16) {
        // Ensure cursor is not in the area covered by sidebar when expanded
        let min_col = if self.sidebar_expanded { sidebar_width as usize } else { 0 };
        if self.cursor_col < min_col {
            self.cursor_col = min_col;
        }
    }

    pub fn update_viewport(&mut self, viewport_width: u16, viewport_height: u16) {
        // With text wrapping, we don't need horizontal scrolling
        self.viewport_col = 0;

        // Calculate how many screen rows the cursor is at (accounting for wrapping)
        let wrap_width = viewport_width as usize;
        let mut screen_row = 0;

        for row in 0..=self.cursor_row {
            if row >= self.text_buffer.line_count() {
                break;
            }

            let line_len = self.text_buffer.get_line_length(row);

            if row < self.cursor_row {
                // Count wrapped lines for rows before cursor
                let wrapped_lines = if line_len == 0 {
                    1
                } else {
                    (line_len + wrap_width - 1) / wrap_width
                };
                screen_row += wrapped_lines;
            } else {
                // For cursor row, count up to cursor position
                let cursor_wrapped_line = self.cursor_col / wrap_width;
                screen_row += cursor_wrapped_line;
            }
        }

        // Scroll to keep cursor visible with margin
        let margin_rows = (viewport_height / 3) as usize;

        if screen_row >= viewport_height as usize - margin_rows {
            // Cursor too far down - scroll down
            // Simple approach: if cursor goes off screen, move viewport down by one logical line
            if self.viewport_row < self.cursor_row {
                self.viewport_row = self.cursor_row.saturating_sub(margin_rows);
            }
        } else if screen_row < margin_rows && self.viewport_row > 0 {
            // Cursor too far up - scroll up
            self.viewport_row = self.viewport_row.saturating_sub(1);
        }
    }

    /// Save current note if dirty and enough time has passed
    pub fn auto_save(&mut self) -> Result<()> {
        if !self.dirty {
            return Ok(());
        }

        let now = std::time::Instant::now();
        if now.duration_since(self.last_save_time).as_millis() < timing::SAVE_INTERVAL_MS {
            return Ok(());
        }

        self.save_current_note()?;
        Ok(())
    }

    /// Force save current note immediately
    pub fn save_current_note(&mut self) -> Result<()> {
        if let Some(ref mut current_note) = self.current_note {
            // Update content and title from buffer
            current_note.content = self.text_buffer.to_string();

            // Extract title from first line
            let lines: Vec<&str> = current_note.content.lines().collect();
            current_note.title = lines.first().unwrap_or(&"Untitled").to_string();

            // Save to disk
            self.notes_manager.save_note(current_note)?;

            // Update the cached notes_list entry
            if self.selected_note_index < self.notes_list.len() {
                self.notes_list[self.selected_note_index] = current_note.clone();
            }

            self.dirty = false;
            self.last_save_time = std::time::Instant::now();
        }
        Ok(())
    }

    /// Mark note as dirty (needs saving)
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    /// Perform search and populate results
    pub fn perform_search(&mut self) {
        self.search_results.clear();

        if self.search_query.is_empty() {
            return;
        }

        let query_lower = self.search_query.to_lowercase();

        for row in 0..self.text_buffer.line_count() {
            if let Some(line) = self.text_buffer.get_line(row) {
                let line_lower = line.to_lowercase();
                let mut start = 0;

                while let Some(pos) = line_lower[start..].find(&query_lower) {
                    let col = start + pos;
                    self.search_results.push((row, col));
                    start = col + 1;
                }
            }
        }

        self.current_search_index = 0;
    }

    /// Jump to next search result
    pub fn next_search_result(&mut self) {
        if !self.search_results.is_empty() {
            self.current_search_index = (self.current_search_index + 1) % self.search_results.len();
            let (row, col) = self.search_results[self.current_search_index];
            self.cursor_row = row;
            self.cursor_col = col;
        }
    }

    /// Jump to previous search result
    pub fn prev_search_result(&mut self) {
        if !self.search_results.is_empty() {
            self.current_search_index = if self.current_search_index == 0 {
                self.search_results.len() - 1
            } else {
                self.current_search_index - 1
            };
            let (row, col) = self.search_results[self.current_search_index];
            self.cursor_row = row;
            self.cursor_col = col;
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let _ = std::fs::write("/tmp/chonk-debug.log", "=== CHONK-NOTE STARTED ===\n");

    let mut app = App::new()?;

    setup_terminal()?;
    let result = run_app(&mut app).await;
    restore_terminal()?;

    result
}

fn setup_terminal() -> Result<()> {
    KittyTerminal::enable_raw_mode().map_err(|e| anyhow::anyhow!("Terminal setup failed: {}", e))?;
    KittyTerminal::enter_fullscreen().map_err(|e| anyhow::anyhow!("Fullscreen failed: {}", e))?;

    // Show and configure cursor
    print!("\x1b[?25h");  // Show cursor
    print!("\x1b[1 q");   // Blinking block
    print!("\x1b[?12h");  // Enable blinking
    std::io::Write::flush(&mut std::io::stdout())?;

    Ok(())
}

fn restore_terminal() -> Result<()> {
    // Reset cursor to default
    print!("\x1b[0 q");  // Default cursor
    print!("\x1b[?12l"); // Disable blinking
    std::io::Write::flush(&mut std::io::stdout())?;

    KittyTerminal::exit_fullscreen().map_err(|e| anyhow::anyhow!("Exit fullscreen failed: {}", e))?;
    KittyTerminal::disable_raw_mode().map_err(|e| anyhow::anyhow!("Disable raw mode failed: {}", e))?;
    Ok(())
}

async fn run_app(app: &mut App) -> Result<()> {
    let mut stdout = io::stdout();
    let mut last_term_size = (0, 0);
    let mut last_render_time = std::time::Instant::now();
    let mut mouse_state = MouseState::default();

    loop {
        let (term_width, term_height) = KittyTerminal::size()?;

        // Auto-save debounced
        if let Err(e) = app.auto_save() {
            let _ = std::fs::write("/tmp/chonk-debug.log", format!("Auto-save error: {}\n", e));
        }

        // Check if terminal was resized
        if (term_width, term_height) != last_term_size {
            app.needs_redraw = true;
            last_term_size = (term_width, term_height);
        }

        // Redraw when necessary (max 120 FPS, or immediately if forced)
        let now = std::time::Instant::now();
        let frame_time = now.duration_since(last_render_time);

        let should_render = app.needs_redraw && (app.force_immediate_render || frame_time.as_millis() >= timing::FRAME_TIME_MS);

        if should_render {
            app.force_immediate_render = false;
            last_render_time = now;

            // Build entire frame in a string buffer (double buffering)
            let mut frame = String::with_capacity(65536); // 64KB buffer

            // BEGIN SYNCHRONIZED UPDATE - atomic screen update
            frame.push_str("\x1b[?2026h"); // Begin sync
            frame.push_str("\x1b[?25l");   // Hide cursor during rendering
            frame.push_str("\x1b[H");      // Move to 0,0
            frame.push_str("\x1b[2J");     // Clear screen

            // Render title bar
            let total_width = term_width as usize;
            let title_bg = rgb_bg(colors::TITLE_BAR_BG.0, colors::TITLE_BAR_BG.1, colors::TITLE_BAR_BG.2);
            let title_fg = rgb_fg(colors::TITLE_BAR_FG.0, colors::TITLE_BAR_FG.1, colors::TITLE_BAR_FG.2);

            // Draw full teal bar first (always full width)
            frame.push_str(&format!("\x1b[1;1H{}{}\x1b[0m", title_bg, " ".repeat(total_width)));

            // Left side: "Notes ▾" and "Help ▾" menu buttons
            let notes_text = if app.notes_menu_expanded { "Notes ▴" } else { "Notes ▾" };
            let help_text = if app.help_menu_expanded { "Help ▴" } else { "Help ▾" };

            let notes_start_col = 0;
            let help_start_col = 10; // After "Notes ▾ "

            frame.push_str(&format!("\x1b[1;{}H{}{}\x1b[1m{}\x1b[0m", notes_start_col + 1, title_bg, title_fg, notes_text));
            frame.push_str(&format!("\x1b[1;{}H{}{}\x1b[1m{}\x1b[0m", help_start_col + 1, title_bg, title_fg, help_text));

            // Current note indicator (after Help menu)
            let note_indicator_col = 18;
            if let Some(ref current_note) = app.current_note {
                let dirty_marker = if app.dirty { "*" } else { "" };
                let max_title_width = total_width.saturating_sub(40); // Leave space for branding on right
                let title = if current_note.title.len() > max_title_width {
                    format!("{}...", &current_note.title[..max_title_width.saturating_sub(3)])
                } else {
                    current_note.title.clone()
                };
                let indicator = format!(" │ {}{}", title, dirty_marker);
                frame.push_str(&format!("\x1b[1;{}H{}{}{}\x1b[0m", note_indicator_col + 1, title_bg, title_fg, indicator));
            }

            // Right side: Hamster + "Chonk-Note"
            let branding_text = "  Chonk-Note "; // Extra space at start to move text right
            let branding_len = branding_text.len();
            let hamster_cols = 2;
            let right_col = total_width.saturating_sub(branding_len + hamster_cols + 1);

            frame.push_str(&format!("\x1b[1;{}H", right_col + 1)); // Position for hamster

            // Write frame header to terminal ONCE
            print!("{}", frame);
            stdout.flush()?;

            // Display hamster PNG (must be after frame write, can't be buffered)
            let _ = KittyTerminal::display_inline_png(HAMSTER_PNG, hamster_cols as u16, 1);
            print!("{}{}\x1b[1m{}\x1b[0m", title_bg, title_fg, branding_text);
            stdout.flush()?;

            // Sidebar widths
            let notes_list_width = if app.sidebar_expanded { layout::SIDEBAR_WIDTH_EXPANDED } else { layout::SIDEBAR_WIDTH_COLLAPSED };
            let settings_panel_width = if app.settings_panel_expanded { layout::SETTINGS_PANEL_WIDTH } else { 0 };

            // Ensure cursor is not under the sidebar
            app.clamp_cursor_to_visible_area(notes_list_width);

            // Update viewport to keep cursor visible (subtract 2 rows: 1 for title bar, 1 for status line)
            let editor_height = term_height.saturating_sub(2);
            app.update_viewport(term_width, editor_height);

            // Render notes editor at full width starting at row 2 (after 1-row title bar)
            let cursor_screen_pos = render_notes_pane(&mut *app, 0, 1, term_width, editor_height)?;

            // Render notes list sidebar on top of editor (overlay, also starting at row 2)
            render_notes_list(&app, 0, 1, notes_list_width, editor_height)?;

            // Render settings panel on right side (overlay)
            if settings_panel_width > 0 {
                let panel_x = term_width.saturating_sub(settings_panel_width);
                render_settings_panel(&app, panel_x, 1, settings_panel_width, editor_height)?;
            }

            // Render dropdown menus AFTER everything else (they're overlays on top)
            if app.notes_menu_expanded {
                render_notes_menu(app, notes_start_col as u16, 2)?;
            }
            if app.help_menu_expanded {
                render_help_menu(app, help_start_col as u16, 2)?;
            }

            // Render status line at bottom
            render_status_line(&app, term_width, term_height)?;

            // Position terminal cursor and end synchronized update
            let mut frame_end = String::with_capacity(256);

            // Position terminal cursor at the actual cursor location
            if app.notes_menu_expanded || app.help_menu_expanded {
                // Keep cursor hidden for menus
            } else {
                if let Some((screen_x, screen_y)) = cursor_screen_pos {
                    frame_end.push_str(&format!("\x1b[{};{}H", screen_y + 1, screen_x + 1));
                }
                frame_end.push_str("\x1b[?25h"); // Show cursor
            }

            // END SYNCHRONIZED UPDATE - atomic flush
            frame_end.push_str("\x1b[?2026l");

            // Write final frame footer
            print!("{}", frame_end);
            stdout.flush()?;

            app.needs_redraw = false;
        }

        // Handle input (but skip if we need immediate render)
        if !app.force_immediate_render && KittyTerminal::poll_input()? {
            if let Some(input) = KittyTerminal::read_input()? {
                match input {
                    kitty_native::InputEvent::Key(key) => {
                        if !keyboard::handle_input(app, key).await? {
                            break;
                        }
                        if app.exit_requested {
                            break;
                        }
                    }
                    kitty_native::InputEvent::Mouse(mouse_event) => {
                        mouse::handle_mouse(app, mouse_event, &mut mouse_state).await?;
                    }
                    kitty_native::InputEvent::Paste(text) => {
                        // Handle bracketed paste - insert all text at once
                        if !text.is_empty() {
                            // Save undo state before modification
                            app.undo_stack.push_state(&app.text_buffer, app.cursor_row, app.cursor_col);

                            // Delete selection if exists
                            if app.text_buffer.selection.is_some() {
                                let sel = app.text_buffer.selection.as_ref().unwrap();
                                let (start_row, start_col, _, _) = sel.normalized();
                                app.text_buffer.delete_selection();
                                app.cursor_row = start_row;
                                app.cursor_col = start_col;
                            }

                            // Insert the pasted text
                            let (new_row, new_col) = app.text_buffer.insert_text(app.cursor_row, app.cursor_col, &text);
                            app.cursor_row = new_row;
                            app.cursor_col = new_col;
                            app.mark_dirty();
                            app.status_message = format!("Pasted {} chars", text.len());
                            app.needs_redraw = true;
                        }
                    }
                }
            }
        }
    }

    // Final save on exit
    app.save_current_note()?;
    Ok(())
}

fn render_notes_pane(app: &mut App, x: u16, y: u16, width: u16, height: u16) -> Result<Option<(u16, u16)>> {
    render_notes_pane_normal(app, x, y, width, height)
}

fn render_notes_pane_normal(app: &mut App, x: u16, y: u16, width: u16, height: u16) -> Result<Option<(u16, u16)>> {
    let viewport_start_row = app.viewport_row;
    let wrap_width = width as usize;

    let mut screen_row = 0;
    let mut cursor_screen_pos = None;

    // Render lines with wrapping
    for buffer_row in viewport_start_row.. {
        if screen_row >= height as usize || buffer_row >= app.text_buffer.line_count() {
            break;
        }

        // Get the line content
        if let Some(line) = app.text_buffer.get_line(buffer_row) {
            // Remove trailing newline if present
            let line = line.trim_end_matches('\n');
            let line_chars: Vec<char> = line.chars().collect();

            // Check if this is the title line (first line)
            let is_title_line = buffer_row == 0;

            // Wrap the line into chunks that fit the width (word-aware)
            let mut col = 0;
            while col < line_chars.len() || (col == 0 && line_chars.is_empty()) {
                if screen_row >= height as usize {
                    break;
                }

                // Clear line
                print!("\x1b[{};{}H\x1b[K", y + screen_row as u16 + 1, x + 1);

                // Build output string
                let mut output = String::with_capacity(wrap_width * 20);

                // If title line, set background and foreground
                if is_title_line {
                    let title_bg = rgb_bg(colors::TITLE_LINE_BG.0, colors::TITLE_LINE_BG.1, colors::TITLE_LINE_BG.2);
                    let title_fg = rgb_fg(colors::TITLE_LINE_FG.0, colors::TITLE_LINE_FG.1, colors::TITLE_LINE_FG.2);
                    output.push_str(&format!("{}{}", title_bg, title_fg));
                }

                // Find word-aware wrap point
                let end_col = if col + wrap_width >= line_chars.len() {
                    // Fits entirely, use rest of line
                    line_chars.len()
                } else {
                    // Need to wrap - find last space before wrap_width
                    let max_end = col + wrap_width;
                    let mut wrap_point = max_end;

                    // Look backwards from max_end for a space
                    for i in (col..max_end).rev() {
                        if line_chars[i].is_whitespace() {
                            wrap_point = i + 1; // Break after the space
                            break;
                        }
                    }

                    // If no space found (single long word), hard break at width
                    if wrap_point == max_end && wrap_point > col {
                        wrap_point = max_end;
                    }

                    wrap_point.min(line_chars.len())
                };
                for buffer_col in col..end_col {
                    let ch = line_chars[buffer_col];

                    // Check if this position is in the selection
                    let in_selection = if let Some(ref sel) = app.text_buffer.selection {
                        sel.contains(buffer_row, buffer_col)
                    } else {
                        false
                    };

                    // Add character with appropriate color
                    if in_selection {
                        let sel_bg = rgb_bg(colors::SELECTION_BG.0, colors::SELECTION_BG.1, colors::SELECTION_BG.2);
                        let sel_fg = rgb_fg(colors::SELECTION_FG.0, colors::SELECTION_FG.1, colors::SELECTION_FG.2);
                        output.push_str(&format!("{}{}{}\x1b[0m", sel_bg, sel_fg, ch));
                        // Restore title colors if on title line
                        if is_title_line {
                            let title_bg = rgb_bg(colors::TITLE_LINE_BG.0, colors::TITLE_LINE_BG.1, colors::TITLE_LINE_BG.2);
                            let title_fg = rgb_fg(colors::TITLE_LINE_FG.0, colors::TITLE_LINE_FG.1, colors::TITLE_LINE_FG.2);
                            output.push_str(&format!("{}{}", title_bg, title_fg));
                        }
                    } else {
                        output.push(ch);
                    }

                    // Check if cursor is at this position
                    if buffer_row == app.cursor_row && buffer_col == app.cursor_col {
                        let screen_col = buffer_col - col;
                        cursor_screen_pos = Some((x + screen_col as u16, y + screen_row as u16));
                    }
                }

                // Fill rest of title line with spaces to show background
                if is_title_line && end_col - col < wrap_width {
                    for _ in 0..(wrap_width - (end_col - col)) {
                        output.push(' ');
                    }
                }

                // Reset colors at end of line
                if is_title_line {
                    output.push_str("\x1b[0m");
                }

                // Print entire wrapped segment
                print!("{}", output);

                // Check if cursor is at end of this line
                if buffer_row == app.cursor_row && app.cursor_col == line_chars.len() && col == 0 && line_chars.is_empty() {
                    cursor_screen_pos = Some((x, y + screen_row as u16));
                } else if buffer_row == app.cursor_row && app.cursor_col >= end_col && app.cursor_col < end_col + wrap_width {
                    let screen_col = app.cursor_col - col;
                    if screen_col < wrap_width {
                        cursor_screen_pos = Some((x + screen_col as u16, y + screen_row as u16));
                    }
                }

                col = end_col;
                screen_row += 1;

                // If this was an empty line, still increment screen_row and break
                if line_chars.is_empty() {
                    break;
                }
            }
        }
    }

    // Clear remaining screen rows
    for row in screen_row..height as usize {
        print!("\x1b[{};{}H\x1b[K", y + row as u16 + 1, x + 1);
    }

    Ok(cursor_screen_pos)
}


fn render_notes_list(app: &App, x: u16, y: u16, width: u16, height: u16) -> Result<()> {
    // Don't render anything if sidebar is collapsed (width = 0)
    if width == 0 {
        return Ok(());
    }

    let sidebar_bg = rgb_bg(colors::SIDEBAR_BG.0, colors::SIDEBAR_BG.1, colors::SIDEBAR_BG.2);
    let sidebar_fg = rgb_fg(colors::SIDEBAR_FG.0, colors::SIDEBAR_FG.1, colors::SIDEBAR_FG.2);
    let sidebar_icon_fg = rgb_fg(colors::SIDEBAR_ICON_FG.0, colors::SIDEBAR_ICON_FG.1, colors::SIDEBAR_ICON_FG.2);

    // Clear sidebar with blue background
    for row in 0..height {
        print!("\x1b[{};{}H{}{}\x1b[0m", y + row + 1, x + 1, sidebar_bg, " ".repeat(width as usize));
    }

    if app.notes_list.is_empty() {
        print!("\x1b[{};{}H{}{} +\x1b[0m", y + 2, x + 1, sidebar_bg, sidebar_icon_fg);
    } else {
        let visible_count = (height - 2) as usize;
        let start_index = app.notes_list_scroll;
        let end_index = (start_index + visible_count).min(app.notes_list.len());

        let selected_bg = rgb_bg(colors::SELECTED_ITEM_BG.0, colors::SELECTED_ITEM_BG.1, colors::SELECTED_ITEM_BG.2);
        let selected_fg = rgb_fg(colors::SELECTED_ITEM_FG.0, colors::SELECTED_ITEM_FG.1, colors::SELECTED_ITEM_FG.2);

        for (display_pos, note_idx) in (start_index..end_index).enumerate() {
            let is_selected = note_idx == app.selected_note_index;
            let note = &app.notes_list[note_idx];

            let (bg_color, text_color) = if is_selected {
                (&selected_bg, &selected_fg)
            } else {
                (&sidebar_bg, &sidebar_fg)
            };

            if app.sidebar_expanded {
                // If this is the selected note and we're editing the title, show the buffer with cursor
                let display_title = if is_selected && app.editing_title {
                    format!("{}_", &app.title_buffer) // Show cursor with underscore
                } else {
                    let title = if note.title.is_empty() {
                        "Untitled".to_string()
                    } else {
                        note.title.clone()
                    };
                    title
                };

                let prefix = if is_selected { "▸ " } else { "  " };
                let max_title_len = (width as usize).saturating_sub(prefix.len());
                let truncated_title: String = if display_title.len() > max_title_len {
                    format!("{}…", &display_title[..max_title_len.saturating_sub(1)])
                } else {
                    display_title
                };

                print!("\x1b[{};{}H{}\x1b[1m{}{}{}\x1b[0m",
                    y + display_pos as u16 + 1, x + 1,
                    bg_color, text_color, prefix, truncated_title);
            } else {
                let indicator = if is_selected { "▸" } else { " " };
                print!("\x1b[{};{}H{}{}{}\x1b[0m",
                    y + display_pos as u16 + 1, x + 1,
                    bg_color, text_color, indicator);
            }
        }

        // Scroll indicators
        let scroll_fg = rgb_fg(colors::SIDEBAR_SCROLL_FG.0, colors::SIDEBAR_SCROLL_FG.1, colors::SIDEBAR_SCROLL_FG.2);
        if start_index > 0 {
            print!("\x1b[{};{}H{}{}↑\x1b[0m", y, x + 2, sidebar_bg, scroll_fg);
        }
        if end_index < app.notes_list.len() {
            print!("\x1b[{};{}H{}{}↓\x1b[0m", y + height - 1, x + 2, sidebar_bg, scroll_fg);
        }
    }

    Ok(())
}

fn render_notes_menu(app: &App, x: u16, y: u16) -> Result<()> {
    let menu_bg = rgb_bg(250, 250, 250); // Light gray
    let menu_fg = rgb_fg(0, 0, 0); // Black text
    let selected_bg = rgb_bg(200, 200, 255); // Light blue for selected

    let menu_width = 45;
    let max_notes = 15; // Max notes to show in dropdown

    // Header
    print!("\x1b[{};{}H{}{}{:<width$}\x1b[0m",
        y, x + 1, menu_bg, menu_fg,
        "─────────────────────────────────────────────",
        width = menu_width);

    let row_offset = 1;

    // Show notes list
    if app.notes_list.is_empty() {
        let empty_msg = "  No notes - Press Ctrl+N to create";
        print!("\x1b[{};{}H{}{}{:<width$}\x1b[0m",
            y + row_offset, x + 1, menu_bg, menu_fg, empty_msg, width = menu_width);
    } else {
        let notes_to_show = app.notes_list.len().min(max_notes);
        for i in 0..notes_to_show {
            let note = &app.notes_list[i];
            let is_current = i == app.selected_note_index;
            let bg = if is_current { &selected_bg } else { &menu_bg };

            // Show editing buffer if this note is being renamed
            let title = if is_current && app.editing_title {
                format!("{}_", &app.title_buffer) // Show cursor with underscore
            } else if note.title.is_empty() {
                "Untitled".to_string()
            } else {
                note.title.clone()
            };

            let prefix = if is_current { "▸ " } else { "  " };
            let display = format!("{}{}", prefix, title);
            let truncated = if display.len() > menu_width - 2 {
                format!("{}…", &display[..menu_width - 3])
            } else {
                display
            };

            print!("\x1b[{};{}H{}{}{:<width$}\x1b[0m",
                y + row_offset + i as u16, x + 1, bg, menu_fg, truncated, width = menu_width);
        }

        if app.notes_list.len() > max_notes {
            print!("\x1b[{};{}H{}{}{:<width$}\x1b[0m",
                y + row_offset + max_notes as u16, x + 1, menu_bg, menu_fg,
                format!("  ... {} more notes", app.notes_list.len() - max_notes),
                width = menu_width);
        }
    }

    Ok(())
}

fn render_help_menu(_app: &App, x: u16, y: u16) -> Result<()> {
    let menu_bg = rgb_bg(250, 250, 250); // Light gray
    let menu_fg = rgb_fg(0, 0, 0); // Black text

    let menu_width = 45;
    let menu_items = vec![
        "─────────────────────────────────────────────".to_string(),
        "Keyboard Shortcuts:".to_string(),
        "─────────────────────────────────────────────".to_string(),
        "  Ctrl+N  - New note".to_string(),
        "  Ctrl+D  - Delete note (press twice)".to_string(),
        "  Ctrl+S  - Save note".to_string(),
        "  Ctrl+F  - Search in note".to_string(),
        "  Ctrl+↑/↓ - Navigate notes".to_string(),
        "  Ctrl+Z/Y - Undo/Redo".to_string(),
        "  Ctrl+C/X/V - Copy/Cut/Paste".to_string(),
        "  Ctrl+A  - Select all".to_string(),
        "  Ctrl+Q  - Quit".to_string(),
        "─────────────────────────────────────────────".to_string(),
        "  Double-click note to rename".to_string(),
        "  Drag to select text".to_string(),
        "─────────────────────────────────────────────".to_string(),
    ];

    for (i, item) in menu_items.iter().enumerate() {
        let display_text = format!("{:<width$}", item, width = menu_width);
        print!("\x1b[{};{}H{}{}{}\x1b[0m", y + i as u16, x + 1, menu_bg, menu_fg, display_text);
    }

    Ok(())
}

fn render_settings_panel(app: &App, x: u16, y: u16, width: u16, height: u16) -> Result<()> {
    let panel_bg = rgb_bg(colors::SIDEBAR_BG.0, colors::SIDEBAR_BG.1, colors::SIDEBAR_BG.2);
    let panel_fg = rgb_fg(colors::SIDEBAR_FG.0, colors::SIDEBAR_FG.1, colors::SIDEBAR_FG.2);
    let on_bg = rgb_bg(76, 175, 80); // Green
    let off_bg = rgb_bg(200, 200, 200); // Gray
    let toggle_fg = rgb_fg(255, 255, 255); // White

    // Clear panel background
    for row in 0..height {
        print!("\x1b[{};{}H{}{}\x1b[0m", y + row + 1, x + 1, panel_bg, " ".repeat(width as usize));
    }

    // Title
    print!("\x1b[{};{}H{}\x1b[1m{}Settings\x1b[0m", y + 1, x + 2, panel_bg, panel_fg);

    // Separator
    print!("\x1b[{};{}H{}{}{}\x1b[0m", y + 2, x + 1, panel_bg, panel_fg, "─".repeat(width as usize));

    // Toggle switches
    let toggle_row_start = 3;

    // 1. Soft-Wrapped Paste
    let soft_wrap_label = "Soft-Wrapped Paste";
    let soft_wrap_state = if app.soft_wrap_paste { " ON " } else { " OFF" };
    let soft_wrap_bg = if app.soft_wrap_paste { &on_bg } else { &off_bg };

    print!("\x1b[{};{}H{}{}{}\x1b[0m",
        y + toggle_row_start, x + 2, panel_bg, panel_fg, soft_wrap_label);
    print!("\x1b[{};{}H{}{}{}\x1b[0m",
        y + toggle_row_start + 1, x + 2, soft_wrap_bg, &toggle_fg, soft_wrap_state);

    // 2. Auto-Save (always on)
    let autosave_label = "Auto-Save";
    let autosave_state = " ON ";

    print!("\x1b[{};{}H{}{}{}\x1b[0m",
        y + toggle_row_start + 3, x + 2, panel_bg, panel_fg, autosave_label);
    print!("\x1b[{};{}H{}{}{}\x1b[0m",
        y + toggle_row_start + 4, x + 2, &on_bg, &toggle_fg, autosave_state);

    Ok(())
}

fn render_status_line(app: &App, term_width: u16, term_height: u16) -> Result<()> {
    // Status line at bottom row
    let status_row = term_height;

    // Status line colors (dark gray background)
    let status_bg = rgb_bg(40, 40, 40);
    let status_fg = rgb_fg(200, 200, 200);
    let dirty_fg = rgb_fg(255, 193, 7); // Amber for dirty indicator

    // Build status line content
    let dirty_indicator = if app.dirty { "*" } else { " " };
    let position_info = format!("Ln {}, Col {} ", app.cursor_row + 1, app.cursor_col + 1);

    // Left side: status message with dirty indicator
    let left_text = format!("{}{}", dirty_indicator, app.status_message);

    // Calculate how much space we have
    let total_width = term_width as usize;
    let position_len = position_info.len();
    let max_message_len = total_width.saturating_sub(position_len).saturating_sub(1); // -1 for spacing

    // Truncate message if needed
    let truncated_left = if left_text.len() > max_message_len {
        format!("{}…", &left_text[..max_message_len.saturating_sub(1)])
    } else {
        left_text
    };

    // Clear status line with background color
    print!("\x1b[{};1H{}{}\x1b[0m", status_row, status_bg, " ".repeat(total_width));

    // Draw left side (message + dirty indicator)
    if app.dirty {
        // Highlight dirty indicator in amber
        print!("\x1b[{};1H{}{}{}\x1b[0m{}{}{}\x1b[0m",
            status_row,
            status_bg, dirty_fg, dirty_indicator,
            status_bg, status_fg, &truncated_left[1..] // Skip first char (dirty indicator)
        );
    } else {
        print!("\x1b[{};1H{}{}{}\x1b[0m",
            status_row,
            status_bg, status_fg, truncated_left
        );
    }

    // Draw right side (position info) - right-aligned
    let position_col = total_width.saturating_sub(position_len) + 1;
    print!("\x1b[{};{}H{}{}{}\x1b[0m",
        status_row, position_col,
        status_bg, status_fg, position_info
    );

    Ok(())
}
