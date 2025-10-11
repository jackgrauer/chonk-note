// CHONK-NOTE - Lightweight notes editor with chunked grid
use anyhow::Result;
use std::io::{self, Write};

// Embed hamster emoji PNG at compile time
const HAMSTER_PNG: &[u8] = include_bytes!("../assets/hamster.png");

mod config;
mod keyboard;
mod kitty_native;
mod mouse;
mod notes_database;
mod notes_mode;
mod chunked_grid;
mod undo;

use kitty_native::KittyTerminal;
use mouse::MouseState;
use chunked_grid::ChunkedGrid;
use config::{layout, timing, colors, rgb_bg, rgb_fg};

pub struct App {
    // Notes database
    pub notes_mode: notes_mode::NotesMode,

    // Chunked grid - the ONLY data structure
    pub grid: ChunkedGrid,
    pub cursor_row: usize,
    pub cursor_col: usize,

    // Viewport scrolling
    pub viewport_row: usize,
    pub viewport_col: usize,

    // Notes list sidebar
    pub notes_list: Vec<notes_database::Note>,
    pub selected_note_index: usize,
    pub notes_list_scroll: usize,
    pub sidebar_expanded: bool,
    pub editing_title: bool,
    pub title_buffer: String,

    // App state
    pub status_message: String,
    pub exit_requested: bool,
    pub needs_redraw: bool,
    pub show_grid_lines: bool,
    pub block_clipboard: Option<Vec<String>>,

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
    pub settings_menu_expanded: bool,
    pub settings_panel_expanded: bool,
}

impl App {
    pub fn new() -> Result<Self> {
        let mut notes_mode = notes_mode::NotesMode::new()?;
        let mut notes_list = Vec::new();
        let mut grid = ChunkedGrid::new();

        // Load existing notes
        if let Ok(notes) = notes_mode.db.list_notes(100) {
            notes_list = notes;
        }

        // Load the first note if available
        if !notes_list.is_empty() {
            let first_note = &notes_list[0];
            let lines: Vec<String> = first_note.content.lines().map(|s| s.to_string()).collect();
            grid = ChunkedGrid::from_lines(&lines);
            notes_mode.current_note = Some(first_note.clone());
        }

        Ok(Self {
            notes_mode,
            grid,
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
            show_grid_lines: false,
            block_clipboard: None,
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
            settings_menu_expanded: false,
            settings_panel_expanded: false,
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
        // Normal mode - logical lines
        let margin_rows = (viewport_height / 3) as usize;
        let margin_cols = 0; // No margin for columns - scroll only at edge

        // Scroll down if cursor is too far down
        if self.cursor_row >= self.viewport_row + viewport_height as usize - margin_rows {
            self.viewport_row = self.cursor_row.saturating_sub(viewport_height as usize - margin_rows - 1);
        }

        // Scroll up if cursor is too far up
        if self.cursor_row < self.viewport_row + margin_rows {
            self.viewport_row = self.cursor_row.saturating_sub(margin_rows);
        }

        // Scroll right if cursor is too far right
        if self.cursor_col >= self.viewport_col + viewport_width as usize - margin_cols {
            self.viewport_col = self.cursor_col.saturating_sub(viewport_width as usize - margin_cols - 1);
        }

        // Scroll left if cursor is too far left
        if self.cursor_col < self.viewport_col + margin_cols {
            self.viewport_col = self.cursor_col.saturating_sub(margin_cols);
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
        if let Some(ref current_note) = self.notes_mode.current_note {
            let lines = self.grid.to_lines();
            let content = lines.join("\n");
            self.notes_mode.db.update_note(&current_note.id, current_note.title.clone(), content, current_note.tags.clone())?;
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

        let lines = self.grid.to_lines();
        let query_lower = self.search_query.to_lowercase();

        for (row, line) in lines.iter().enumerate() {
            let line_lower = line.to_lowercase();
            let mut start = 0;

            while let Some(pos) = line_lower[start..].find(&query_lower) {
                let col = start + pos;
                self.search_results.push((row, col));
                start = col + 1;
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

        // Redraw when necessary (max 60 FPS)
        let now = std::time::Instant::now();
        let frame_time = now.duration_since(last_render_time);

        if app.needs_redraw && frame_time.as_millis() >= timing::FRAME_TIME_MS {
            KittyTerminal::move_to(0, 0)?;
            last_render_time = now;

            // BEGIN SYNCHRONIZED UPDATE - prevents flicker
            print!("\x1b[?2026h");
            print!("\x1b[s");

            // Clear entire screen first to prevent artifacts
            print!("\x1b[2J");

            // Render title bar
            let total_width = term_width as usize;
            let title_bg = rgb_bg(colors::TITLE_BAR_BG.0, colors::TITLE_BAR_BG.1, colors::TITLE_BAR_BG.2);
            let title_fg = rgb_fg(colors::TITLE_BAR_FG.0, colors::TITLE_BAR_FG.1, colors::TITLE_BAR_FG.2);

            // Draw full teal bar first (always full width)
            print!("\x1b[1;1H{}{}\x1b[0m", title_bg, " ".repeat(total_width));

            // Left side: "Notes ▾" and "Settings ▾" menu buttons
            let notes_text = if app.notes_menu_expanded { "Notes ▴" } else { "Notes ▾" };
            let settings_text = if app.settings_menu_expanded { "Settings ▴" } else { "Settings ▾" };

            let notes_start_col = 0;
            let settings_start_col = 10; // After "Notes ▾ "

            print!("\x1b[1;{}H{}{}\x1b[1m{}\x1b[0m", notes_start_col + 1, title_bg, title_fg, notes_text);
            print!("\x1b[1;{}H{}{}\x1b[1m{}\x1b[0m", settings_start_col + 1, title_bg, title_fg, settings_text);

            // Right side: Hamster + "Chonk-Note"
            let branding_text = "  Chonk-Note "; // Extra space at start to move text right
            let branding_len = branding_text.len();
            let hamster_cols = 2;
            let hamster_rows = 1;
            let right_col = total_width.saturating_sub(branding_len + hamster_cols + 1); // Move left by 1

            print!("\x1b[1;{}H", right_col + 1); // Position for hamster
            let _ = KittyTerminal::display_inline_png(HAMSTER_PNG, hamster_cols as u16, hamster_rows as u16);
            print!("{}{}\x1b[1m{}\x1b[0m", title_bg, title_fg, branding_text);

            // Render dropdown menus if expanded
            if app.notes_menu_expanded {
                render_notes_menu(app, notes_start_col as u16, 2)?;
            }
            if app.settings_menu_expanded {
                render_settings_menu(app, settings_start_col as u16, 2)?;
            }

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

            // Render status line at bottom
            render_status_line(&app, term_width, term_height)?;

            // Position terminal cursor at the actual cursor location
            if let Some((screen_x, screen_y)) = cursor_screen_pos {
                print!("\x1b[{};{}H", screen_y + 1, screen_x + 1); // Move to cursor position (1-based)
            }

            // Make sure cursor is visible
            print!("\x1b[?25h");  // Show cursor
            print!("\x1b[?2026l");
            stdout.flush()?;

            app.needs_redraw = false;
        }

        // Handle input
        if KittyTerminal::poll_input()? {
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
    let viewport_start_col = app.viewport_col;

    // Render visible lines with selection highlighting
    for screen_row in 0..height {
        let grid_row = viewport_start_row + screen_row as usize;

        // Clear line
        print!("\x1b[{};{}H\x1b[K", y + screen_row + 1, x + 1);

        // Render each character with selection highlighting
        for screen_col in 0..width as usize {
            let grid_col = viewport_start_col + screen_col;
            let ch = app.grid.get(grid_row, grid_col);

            // Check if this position is in the selection
            let in_selection = if let Some(ref sel) = app.grid.selection {
                sel.contains(grid_row, grid_col)
            } else {
                false
            };

            // Render with appropriate color
            if in_selection {
                // For selected cells, always show background even for spaces
                let display_ch = if ch == ' ' { ' ' } else { ch };
                let sel_bg = rgb_bg(colors::SELECTION_BG.0, colors::SELECTION_BG.1, colors::SELECTION_BG.2);
                let sel_fg = rgb_fg(colors::SELECTION_FG.0, colors::SELECTION_FG.1, colors::SELECTION_FG.2);
                print!("{}{}{}\x1b[0m", sel_bg, sel_fg, display_ch);
            } else {
                print!("{}", ch);
            }
        }
    }

    // Render grid lines if enabled
    if app.show_grid_lines {
        let grid_fg = rgb_fg(colors::GRID_LINE_FG.0, colors::GRID_LINE_FG.1, colors::GRID_LINE_FG.2);

        // Vertical lines every 8 characters
        for col in (layout::GRID_VERTICAL_SPACING..width as usize).step_by(layout::GRID_VERTICAL_SPACING) {
            for row in 0..height {
                let grid_row = viewport_start_row + row as usize;
                let grid_col = viewport_start_col + col;
                let ch = app.grid.get(grid_row, grid_col);

                // Only draw grid line if cell is empty
                if ch == ' ' {
                    print!("\x1b[{};{}H{}│\x1b[0m",
                           y + row as u16 + 1, x + col as u16 + 1, grid_fg);
                }
            }
        }

        // Horizontal lines every 4 rows
        for row in (layout::GRID_HORIZONTAL_SPACING..height as usize).step_by(layout::GRID_HORIZONTAL_SPACING) {
            for col in 0..width as usize {
                let grid_row = viewport_start_row + row;
                let grid_col = viewport_start_col + col;
                let ch = app.grid.get(grid_row, grid_col);

                // Only draw grid line if cell is empty
                if ch == ' ' {
                    print!("\x1b[{};{}H{}─\x1b[0m",
                           y + row as u16 + 1, x + col as u16 + 1, grid_fg);
                }
            }
        }
    }

    // Calculate cursor screen position
    let cursor_screen_row = app.cursor_row.saturating_sub(viewport_start_row);
    let cursor_screen_col = app.cursor_col.saturating_sub(viewport_start_col);

    let cursor_screen_pos = if cursor_screen_row < height as usize && cursor_screen_col < width as usize {
        Some((x + cursor_screen_col as u16, y + cursor_screen_row as u16))
    } else {
        None
    };

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

    let menu_width = 45;
    let menu_items = vec![
        "─────────────────────────────────────────────".to_string(),
        "Manage Notes:".to_string(),
        "  Ctrl+N  - New note".to_string(),
        "  Ctrl+D  - Delete note (press twice)".to_string(),
        "  Ctrl+S  - Save note".to_string(),
        "  Ctrl+↑/↓ - Navigate notes".to_string(),
        "  Double-click - Rename note".to_string(),
        "─────────────────────────────────────────────".to_string(),
    ];

    for (i, item) in menu_items.iter().enumerate() {
        let display_text = format!("{:<width$}", item, width = menu_width);
        print!("\x1b[{};{}H{}{}{}\x1b[0m", y + i as u16, x + 1, menu_bg, menu_fg, display_text);
    }

    Ok(())
}

fn render_settings_menu(app: &App, x: u16, y: u16) -> Result<()> {
    // Menu background color
    let menu_bg = rgb_bg(250, 250, 250); // Light gray
    let menu_fg = rgb_fg(0, 0, 0); // Black text
    let button_bg = if app.soft_wrap_paste {
        rgb_bg(76, 175, 80) // Green when ON
    } else {
        rgb_bg(200, 200, 200) // Gray when OFF
    };
    let button_fg = rgb_fg(255, 255, 255); // White text

    let menu_width = 45;
    let button_text = if app.soft_wrap_paste { " ON " } else { " OFF " };
    let soft_wrap_line = format!("Soft-Wrapped Paste: {}", button_text);
    let menu_items = vec![
        "─────────────────────────────────────────────".to_string(),
        soft_wrap_line,
        "─────────────────────────────────────────────".to_string(),
    ];

    for (i, item) in menu_items.iter().enumerate() {
        if i == 1 {
            // Render toggle button for soft-wrap line
            let label = "Soft-Wrapped Paste: ";
            print!("\x1b[{};{}H{}{}{}", y + i as u16, x + 1, menu_bg, menu_fg, label);
            print!("{}{}{}\x1b[0m", button_bg, button_fg, button_text);
            // Fill rest of line with menu background
            let remaining = menu_width - label.len() - button_text.len();
            print!("{}{}{}\x1b[0m", menu_bg, menu_fg, " ".repeat(remaining));
        } else {
            let display_text = format!("{:<width$}", item, width = menu_width);
            print!("\x1b[{};{}H{}{}{}\x1b[0m",
                y + i as u16,
                x + 1,
                menu_bg,
                menu_fg,
                display_text
            );
        }
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

    // 2. Grid Lines (placeholder for future)
    let grid_lines_label = "Show Grid Lines";
    let grid_lines_state = if app.show_grid_lines { " ON " } else { " OFF" };
    let grid_lines_bg = if app.show_grid_lines { &on_bg } else { &off_bg };

    print!("\x1b[{};{}H{}{}{}\x1b[0m",
        y + toggle_row_start + 3, x + 2, panel_bg, panel_fg, grid_lines_label);
    print!("\x1b[{};{}H{}{}{}\x1b[0m",
        y + toggle_row_start + 4, x + 2, grid_lines_bg, &toggle_fg, grid_lines_state);

    // 3. Auto-Save (placeholder - always on for now)
    let autosave_label = "Auto-Save";
    let autosave_state = " ON ";

    print!("\x1b[{};{}H{}{}{}\x1b[0m",
        y + toggle_row_start + 6, x + 2, panel_bg, panel_fg, autosave_label);
    print!("\x1b[{};{}H{}{}{}\x1b[0m",
        y + toggle_row_start + 7, x + 2, &on_bg, &toggle_fg, autosave_state);

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
