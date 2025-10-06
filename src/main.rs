// CHONK-NOTE - Lightweight notes editor
use anyhow::Result;
use helix_core::{Rope, Selection, history::History};
use std::io::{self, Write};

mod edit_renderer;
mod keyboard;
mod kitty_native;
mod mouse;
mod block_selection;
mod notes_database;
mod notes_mode;
mod debug;
mod virtual_grid;
mod grid_cursor;
mod text_filter;

use edit_renderer::EditPanelRenderer;
use kitty_native::KittyTerminal;
use mouse::MouseState;
use block_selection::BlockSelection;

pub struct App {
    // Notes database
    pub notes_mode: notes_mode::NotesMode,

    // Current note editing
    pub notes_rope: Rope,
    pub notes_selection: Selection,
    pub notes_history: History,
    pub notes_block_selection: Option<BlockSelection>,

    // Virtual grid for cursor beyond text
    pub notes_grid: virtual_grid::VirtualGrid,
    pub notes_cursor: grid_cursor::GridCursor,

    // Notes list sidebar
    pub notes_list: Vec<notes_database::Note>,
    pub selected_note_index: usize,
    pub notes_list_scroll: usize,
    pub sidebar_expanded: bool,
    pub editing_title: bool,
    pub title_buffer: String,

    // Rendering
    pub notes_display: Option<EditPanelRenderer>,

    // App state
    pub status_message: String,
    pub exit_requested: bool,
    pub needs_redraw: bool,
    pub block_clipboard: Option<Vec<String>>,
    pub wrap_text: bool,
}

impl App {
    pub fn new() -> Result<Self> {
        let mut notes_mode = notes_mode::NotesMode::new()?;
        let mut notes_list = Vec::new();

        // Load existing notes
        if let Ok(notes) = notes_mode.db.list_notes(100) {
            notes_list = notes;
        }

        // Start with an empty note
        let mut notes_rope = Rope::from("");
        let mut notes_selection = Selection::point(0);
        notes_mode.handle_command(&mut notes_rope, &mut notes_selection, "new")?;

        Ok(Self {
            notes_mode,
            notes_rope: notes_rope.clone(),
            notes_selection,
            notes_history: History::default(),
            notes_block_selection: None,
            notes_grid: virtual_grid::VirtualGrid::new(notes_rope),
            notes_cursor: grid_cursor::GridCursor::new(),
            notes_list,
            selected_note_index: 0,
            notes_list_scroll: 0,
            sidebar_expanded: false,
            editing_title: false,
            title_buffer: String::new(),
            notes_display: None,
            status_message: "Ctrl+N: New | Ctrl+Up/Down: Navigate | Ctrl+Q: Quit".to_string(),
            exit_requested: false,
            needs_redraw: true,
            block_clipboard: None,
            wrap_text: false,
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
    Ok(())
}

fn restore_terminal() -> Result<()> {
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

            // Render notes editor
            render_notes_pane(&mut *app, notes_list_width, 1, remaining_width, term_height.saturating_sub(1))?;

            print!("\x1b[u");
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

fn render_notes_pane(app: &mut App, x: u16, y: u16, width: u16, height: u16) -> Result<()> {
    // Create renderer if needed
    if app.notes_display.is_none() {
        let mut renderer = EditPanelRenderer::new(width, height);
        renderer.update_from_rope_with_wrap(&app.notes_rope, app.wrap_text);
        app.notes_display = Some(renderer);
    }

    if let Some(renderer) = &mut app.notes_display {
        renderer.resize(width, height);
        renderer.update_from_rope_with_wrap(&app.notes_rope, app.wrap_text);

        let cursor_line = app.notes_cursor.row;
        let cursor_col = app.notes_cursor.col;

        renderer.follow_cursor(cursor_col, cursor_line, 3);

        // Render with block selection if active
        if app.notes_block_selection.is_some() {
            renderer.render_with_block_selection(
                x, y, width, height,
                (cursor_col, cursor_line),
                app.notes_block_selection.as_ref(),
                true
            )?;
        } else {
            // Render with normal selection
            let (sel_start, sel_end) = {
                let range = app.notes_selection.primary();
                if range.from() != range.to() {
                    let start_line = app.notes_rope.char_to_line(range.from());
                    let end_line = app.notes_rope.char_to_line(range.to().saturating_sub(1).max(0));
                    let start_line_char = app.notes_rope.line_to_char(start_line);
                    let end_line_char = app.notes_rope.line_to_char(end_line);

                    let start_col = range.from().saturating_sub(start_line_char);
                    let end_col = range.to().saturating_sub(end_line_char);

                    (Some((start_col, start_line)), Some((end_col, end_line)))
                } else {
                    (None, None)
                }
            };

            renderer.render_with_cursor_and_selection(
                x, y, width, height,
                (cursor_col, cursor_line),
                sel_start,
                sel_end,
                true
            )?;
        }
    }

    Ok(())
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
                let title = if note.title.is_empty() {
                    "Untitled".to_string()
                } else {
                    note.title.clone()
                };
                let num_prefix = format!("{}. ", note_idx + 1);
                let max_title_len = (width as usize).saturating_sub(num_prefix.len());
                let display_title: String = if title.len() > max_title_len {
                    format!("{}…", &title[..max_title_len.saturating_sub(1)])
                } else {
                    title
                };

                print!("\x1b[{};{}H{}\x1b[1m{}{}{}\x1b[0m",
                    y + display_pos as u16 + 1, x + 1,
                    bg_color, text_color, num_prefix, display_title);
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
