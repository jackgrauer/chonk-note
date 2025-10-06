// CHONK-NOTE - Lightweight notes editor with chunked grid
use anyhow::Result;
use std::io::{self, Write};

mod keyboard;
mod kitty_native;
mod mouse;
mod notes_database;
mod notes_mode;
mod chunked_grid;

use kitty_native::KittyTerminal;
use mouse::MouseState;
use chunked_grid::ChunkedGrid;

pub struct App {
    // Notes database
    pub notes_mode: notes_mode::NotesMode,

    // Chunked grid - the ONLY data structure
    pub grid: ChunkedGrid,
    pub cursor_row: usize,
    pub cursor_col: usize,

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
}

impl App {
    pub fn new() -> Result<Self> {
        let notes_mode = notes_mode::NotesMode::new()?;
        let mut notes_list = Vec::new();

        // Load existing notes
        if let Ok(notes) = notes_mode.db.list_notes(100) {
            notes_list = notes;
        }

        Ok(Self {
            notes_mode,
            grid: ChunkedGrid::new(),
            cursor_row: 0,
            cursor_col: 0,
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
        })
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

        // Check if terminal was resized
        if (term_width, term_height) != last_term_size {
            app.needs_redraw = true;
            last_term_size = (term_width, term_height);
        }

        // Redraw when necessary (max 20 FPS)
        let now = std::time::Instant::now();
        let frame_time = now.duration_since(last_render_time);

        if app.needs_redraw && frame_time.as_millis() >= 50 {
            KittyTerminal::move_to(0, 0)?;
            last_render_time = now;

            // BEGIN SYNCHRONIZED UPDATE - prevents flicker
            print!("\x1b[?2026h");
            print!("\x1b[s");

            // Render title bar
            let note_name = if !app.notes_list.is_empty() && app.selected_note_index < app.notes_list.len() {
                &app.notes_list[app.selected_note_index].title
            } else {
                "Untitled"
            };
            print!("\x1b[1;1H\x1b[48;2;255;255;0m\x1b[38;2;0;0;0m\x1b[1m {}{}\x1b[0m",
                note_name,
                " ".repeat((term_width as usize).saturating_sub(note_name.len() + 1)));

            // Sidebar width
            let notes_list_width = if app.sidebar_expanded { 30 } else { 4 };
            let remaining_width = term_width.saturating_sub(notes_list_width);

            // Render notes list sidebar
            render_notes_list(&app, 0, 1, notes_list_width, term_height.saturating_sub(1))?;

            // Render notes editor and get cursor position
            let cursor_screen_pos = render_notes_pane(&mut *app, notes_list_width, 1, remaining_width, term_height.saturating_sub(1))?;

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

    Ok(())
}

fn render_notes_pane(app: &mut App, x: u16, y: u16, width: u16, height: u16) -> Result<Option<(u16, u16)>> {
    // Simple direct rendering from chunked grid
    // No need for complex renderer - just draw what's visible

    // Determine viewport (what rows/cols are visible)
    let viewport_start_row = 0; // TODO: Add scrolling later
    let viewport_start_col = 0;

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
                print!("\x1b[48;2;255;20;147m\x1b[38;2;255;255;255m{}\x1b[0m", ch); // Hot pink background, white text
            } else {
                print!("{}", ch);
            }
        }
    }

    // Render grid lines if enabled
    if app.show_grid_lines {
        // Vertical lines every 8 characters
        for col in (8..width).step_by(8) {
            for row in 0..height {
                let grid_row = viewport_start_row + row as usize;
                let grid_col = viewport_start_col + col as usize;
                let ch = app.grid.get(grid_row, grid_col);

                // Only draw grid line if cell is empty
                if ch == ' ' {
                    print!("\x1b[{};{}H\x1b[38;2;60;60;60m│\x1b[0m",
                           y + row + 1, x + col + 1);
                }
            }
        }

        // Horizontal lines every 4 rows
        for row in (4..height).step_by(4) {
            for col in 0..width {
                let grid_row = viewport_start_row + row as usize;
                let grid_col = viewport_start_col + col as usize;
                let ch = app.grid.get(grid_row, grid_col);

                // Only draw grid line if cell is empty
                if ch == ' ' {
                    print!("\x1b[{};{}H\x1b[38;2;60;60;60m─\x1b[0m",
                           y + row + 1, x + col + 1);
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
                let num_prefix = format!("{}. ", note_idx + 1);

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

                let max_title_len = (width as usize).saturating_sub(num_prefix.len());
                let truncated_title: String = if display_title.len() > max_title_len {
                    format!("{}…", &display_title[..max_title_len.saturating_sub(1)])
                } else {
                    display_title
                };

                print!("\x1b[{};{}H{}\x1b[1m{}{}{}\x1b[0m",
                    y + display_pos as u16 + 1, x + 1,
                    bg_color, text_color, num_prefix, truncated_title);
            } else {
                let note_num = note_idx + 1;
                let indicator = if is_selected { "> " } else { "  " };
                print!("\x1b[{};{}H{}{}{}{}\x1b[0m",
                    y + display_pos as u16 + 1, x,
                    bg_color, text_color, indicator, note_num);
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
