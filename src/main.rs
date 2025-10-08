// CHONK-NOTE - Lightweight notes editor with chunked grid
use anyhow::Result;
use std::io::{self, Write};

// Embed hamster emoji PNG at compile time
const HAMSTER_PNG: &[u8] = include_bytes!("../assets/hamster.png");

mod keyboard;
mod kitty_native;
mod mouse;
mod notes_database;
mod notes_mode;
mod chunked_grid;

use kitty_native::KittyTerminal;
use mouse::MouseState;
use chunked_grid::ChunkedGrid;

// UI Constants
const SIDEBAR_WIDTH_EXPANDED: u16 = 30;
const SIDEBAR_WIDTH_COLLAPSED: u16 = 0;
const GRID_VERTICAL_SPACING: usize = 8;
const GRID_HORIZONTAL_SPACING: usize = 4;
const VISIBLE_NOTE_COUNT_APPROX: usize = 30;
const FRAME_TIME_MS: u128 = 16; // 60 FPS for smooth selection dragging

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
            status_message: "Click anywhere and type! Ctrl+N: New | Ctrl+Q: Quit | Ctrl+G: Grid Lines".to_string(),
            exit_requested: false,
            needs_redraw: true,
            show_grid_lines: false,
            block_clipboard: None,
            delete_confirmation_note: None,
            dirty: false,
            last_save_time: std::time::Instant::now(),
        })
    }

    /// Update viewport to keep cursor visible
    pub fn clamp_cursor_to_visible_area(&mut self, sidebar_width: u16) {
        // Ensure cursor is not in the area covered by sidebar
        let min_col = if self.sidebar_expanded { 0 } else { sidebar_width as usize };
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
        const SAVE_INTERVAL_MS: u128 = 2000; // 2 seconds

        if !self.dirty {
            return Ok(());
        }

        let now = std::time::Instant::now();
        if now.duration_since(self.last_save_time).as_millis() < SAVE_INTERVAL_MS {
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

        // Redraw when necessary (max 20 FPS)
        let now = std::time::Instant::now();
        let frame_time = now.duration_since(last_render_time);

        if app.needs_redraw && frame_time.as_millis() >= FRAME_TIME_MS {
            KittyTerminal::move_to(0, 0)?;
            last_render_time = now;

            // BEGIN SYNCHRONIZED UPDATE - prevents flicker
            print!("\x1b[?2026h");
            print!("\x1b[s");

            // Clear entire screen first to prevent artifacts
            print!("\x1b[2J");

            // Render title bar
            let note_name = if !app.notes_list.is_empty() && app.selected_note_index < app.notes_list.len() {
                &app.notes_list[app.selected_note_index].title
            } else {
                "Untitled"
            };

            // Show sidebar indicator when collapsed (always 2 chars to prevent jostling)
            let sidebar_indicator = if !app.sidebar_expanded { "▸ " } else { "  " };
            let title_text = format!("{}{}", sidebar_indicator, note_name);

            // Right-aligned branding constants (calculate first for fixed positioning)
            let branding_text = "  Chonk-Note";
            let branding_len = branding_text.len();
            let hamster_cols = 2; // Hamster image takes 2 terminal columns
            let hamster_rows = 1; // Hamster image takes 1 terminal row
            let total_width = term_width as usize;

            // Calculate absolute position for branding from right edge
            let branding_start_col = total_width.saturating_sub(branding_len);
            let hamster_start_col = branding_start_col.saturating_sub(hamster_cols);

            // Draw full yellow bar first (always full width)
            print!("\x1b[1;1H\x1b[48;2;255;255;0m{}\x1b[0m", " ".repeat(total_width));

            // Draw title text on top at left
            print!("\x1b[1;1H\x1b[48;2;255;255;0m\x1b[38;2;0;0;0m\x1b[1m{}\x1b[0m", title_text);

            // Position cursor at fixed hamster position using absolute positioning
            print!("\x1b[1;{}H", hamster_start_col + 1); // Move to exact column (1-based)

            // Display hamster emoji as inline PNG (2 cols x 1 row)
            let _ = KittyTerminal::display_inline_png(HAMSTER_PNG, hamster_cols as u16, hamster_rows as u16);

            // Continue with branding text on same background
            print!("\x1b[48;2;255;255;0m\x1b[38;2;0;0;0m\x1b[1m{}\x1b[0m", branding_text);

            // Sidebar width
            let notes_list_width = if app.sidebar_expanded { SIDEBAR_WIDTH_EXPANDED } else { SIDEBAR_WIDTH_COLLAPSED };

            // Ensure cursor is not under the sidebar
            app.clamp_cursor_to_visible_area(notes_list_width);

            // Update viewport to keep cursor visible (use full width for editor, subtract 1 row for title bar)
            let editor_height = term_height.saturating_sub(1);
            app.update_viewport(term_width, editor_height);

            // Render notes editor at full width starting at row 2 (after 1-row title bar)
            let cursor_screen_pos = render_notes_pane(&mut *app, 0, 1, term_width, editor_height)?;

            // Render notes list sidebar on top of editor (overlay, also starting at row 2)
            render_notes_list(&app, 0, 1, notes_list_width, editor_height)?;

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
                print!("\x1b[48;2;255;20;147m\x1b[38;2;255;255;255m{}\x1b[0m", display_ch);
            } else {
                print!("{}", ch);
            }
        }
    }

    // Render grid lines if enabled
    if app.show_grid_lines {
        // Vertical lines every 8 characters
        for col in (GRID_VERTICAL_SPACING..width as usize).step_by(GRID_VERTICAL_SPACING) {
            for row in 0..height {
                let grid_row = viewport_start_row + row as usize;
                let grid_col = viewport_start_col + col;
                let ch = app.grid.get(grid_row, grid_col);

                // Only draw grid line if cell is empty
                if ch == ' ' {
                    print!("\x1b[{};{}H\x1b[38;2;60;60;60m│\x1b[0m",
                           y + row as u16 + 1, x + col as u16 + 1);
                }
            }
        }

        // Horizontal lines every 4 rows
        for row in (GRID_HORIZONTAL_SPACING..height as usize).step_by(GRID_HORIZONTAL_SPACING) {
            for col in 0..width as usize {
                let grid_row = viewport_start_row + row;
                let grid_col = viewport_start_col + col;
                let ch = app.grid.get(grid_row, grid_col);

                // Only draw grid line if cell is empty
                if ch == ' ' {
                    print!("\x1b[{};{}H\x1b[38;2;60;60;60m─\x1b[0m",
                           y + row as u16 + 1, x + col as u16 + 1);
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

    // Clear sidebar with blue background
    for row in 0..height {
        print!("\x1b[{};{}H\x1b[48;2;30;60;100m{}\x1b[0m", y + row + 1, x + 1, " ".repeat(width as usize));
    }

    if app.notes_list.is_empty() {
        print!("\x1b[{};{}H\x1b[48;2;30;60;100m\x1b[38;2;200;200;200m +\x1b[0m", y + 2, x + 1);
    } else {
        let visible_count = (height - 2) as usize;
        let start_index = app.notes_list_scroll;
        let end_index = (start_index + visible_count).min(app.notes_list.len());

        for (display_pos, note_idx) in (start_index..end_index).enumerate() {
            let is_selected = note_idx == app.selected_note_index;
            let note = &app.notes_list[note_idx];

            let (bg_color, text_color) = if is_selected {
                ("\x1b[48;2;255;193;7m", "\x1b[38;2;0;0;0m")
            } else {
                ("\x1b[48;2;30;60;100m", "\x1b[38;2;220;220;220m")
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
        if start_index > 0 {
            print!("\x1b[{};{}H\x1b[48;2;30;60;100m\x1b[38;2;76;175;80m↑\x1b[0m", y, x + 2);
        }
        if end_index < app.notes_list.len() {
            print!("\x1b[{};{}H\x1b[48;2;30;60;100m\x1b[38;2;76;175;80m↓\x1b[0m", y + height - 1, x + 2);
        }
    }

    Ok(())
}
