// CHONK-NOTE - Lightweight notes editor
use anyhow::Result;
use kitty_native::KittyTerminal;
use std::io::{self, Write};

// HELIX-CORE INTEGRATION! Professional text editing
use helix_core::{
    Rope, Selection,
    history::History,
};

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
mod coordinate_system;
mod text_filter;
mod dual_pane_keyboard;  // Stub for notes-only
// PDF-only modules removed
// mod content_extractor;
// mod viuer_display;
// mod kitty_file_picker;

use edit_renderer::EditPanelRenderer;
use mouse::MouseState;
use block_selection::BlockSelection;


// Simplified enums for notes-only mode
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AppMode {
    NotesEditor,  // Only notes mode
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ActivePane {
    Left,   // Notes editor
    Right,  // Reserved for future use
}

pub struct App {
    // Mode (always NotesEditor)
    pub app_mode: AppMode,
    pub active_pane: ActivePane,

    // Notes mode (Some for compatibility with keyboard.rs)
    pub notes_mode: Option<notes_mode::NotesMode>,

    // Text editing with Helix-core
    pub notes_rope: Rope,
    pub notes_selection: Selection,
    pub notes_history: History,
    pub notes_block_selection: Option<BlockSelection>,

    // Grid-based cursor
    pub notes_grid: virtual_grid::VirtualGrid,
    pub notes_cursor: grid_cursor::GridCursor,

    // Stub fields for PDF compatibility (unused in notes-only mode)
    pub extraction_rope: Rope,
    pub extraction_selection: Selection,
    pub extraction_history: History,
    pub extraction_block_selection: Option<BlockSelection>,
    pub extraction_grid: virtual_grid::VirtualGrid,
    pub extraction_cursor: grid_cursor::GridCursor,
    pub edit_display: Option<EditPanelRenderer>,

    // Notes list for sidebar
    pub notes_list: Vec<notes_database::Note>,
    pub selected_note_index: usize,
    pub notes_list_scroll: usize,
    pub unsaved_changes: bool,
    pub sidebar_expanded: bool,
    pub editing_title: bool,
    pub title_buffer: String,

    // Rendering
    pub notes_display: Option<EditPanelRenderer>,

    // App state
    pub status_message: String,
    pub exit_requested: bool,
    pub needs_redraw: bool,
    pub open_file_picker: bool,

    // Block clipboard
    pub block_clipboard: Option<Vec<String>>,

    // Text wrapping
    pub wrap_text: bool,

    // More PDF stubs (unused)
    pub pdf_scroll_x: u16,
    pub pdf_scroll_y: u16,
    pub pdf_zoom: f32,
    pub pdf_full_width: u16,
    pub pdf_full_height: u16,
    pub split_position: Option<u16>,
    pub is_dragging_divider: bool,
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
            app_mode: AppMode::NotesEditor,
            active_pane: ActivePane::Left,
            notes_mode: Some(notes_mode),
            notes_rope: notes_rope.clone(),
            notes_selection,
            notes_history: History::default(),
            notes_block_selection: None,
            notes_grid: virtual_grid::VirtualGrid::new(notes_rope.clone()),
            notes_cursor: grid_cursor::GridCursor::new(),
            // Stub fields (unused)
            extraction_rope: Rope::from(""),
            extraction_selection: Selection::point(0),
            extraction_history: History::default(),
            extraction_block_selection: None,
            extraction_grid: virtual_grid::VirtualGrid::new(Rope::from("")),
            extraction_cursor: grid_cursor::GridCursor::new(),
            edit_display: None,
            // Notes state
            notes_list,
            selected_note_index: 0,
            notes_list_scroll: 0,
            unsaved_changes: false,
            sidebar_expanded: false,
            editing_title: false,
            title_buffer: String::new(),
            notes_display: None,
            status_message: "Notes Mode (auto-save) - Ctrl+Up/Down: Nav | Ctrl+N: New | Ctrl+Q: Quit".to_string(),
            exit_requested: false,
            needs_redraw: true,
            open_file_picker: false,
            block_clipboard: None,
            wrap_text: false,
            // PDF stubs
            pdf_scroll_x: 0,
            pdf_scroll_y: 0,
            pdf_zoom: 1.0,
            pdf_full_width: 0,
            pdf_full_height: 0,
            split_position: None,
            is_dragging_divider: false,
        })
    }

    // Stub methods for PDF compatibility (do nothing in notes-only mode)
    pub async fn load_pdf_page(&mut self) -> Result<()> { Ok(()) }
    pub async fn extract_current_page(&mut self) -> Result<()> { Ok(()) }
    pub async fn toggle_extraction_method(&mut self) -> Result<()> { Ok(()) }
    pub fn next_page(&mut self) {}
    pub fn prev_page(&mut self) {}
    pub fn switch_active_pane(&mut self, _pane: ActivePane) {}
    pub fn toggle_notes_mode(&mut self) -> Result<()> { Ok(()) }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Clear debug log at startup
    let _ = std::fs::write("/tmp/chonk-debug.log", "=== CHONK-NOTE STARTED ===\n");

    let mut app = App::new()?;

    setup_terminal()?;
    let result = run_app(&mut app).await;
    restore_terminal()?;

    result
}


fn setup_terminal() -> Result<()> {
    // CROSSTERM ELIMINATED! Pure Kitty-native
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

    // Initial render
    app.needs_redraw = true;

    loop {
        let (term_width, term_height) = KittyTerminal::size()?;

        // Check if terminal was resized
        if (term_width, term_height) != last_term_size {
            app.needs_redraw = true;
            last_term_size = (term_width, term_height);
        }

        // NUCLEAR ANTI-FLICKER: Only redraw when absolutely necessary
        let now = std::time::Instant::now();
        let frame_time = now.duration_since(last_render_time);

        if app.needs_redraw && frame_time.as_millis() >= 50 { // Max 20 FPS - nuclear anti-flicker
            KittyTerminal::move_to(0, 0)?;
            last_render_time = now;

            // BEGIN SYNCHRONIZED UPDATE - prevents flicker by batching all drawing
            print!("\x1b[?2026h");

            // Save cursor position
            print!("\x1b[s");

            // Render top bar
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
            let notes_start_x = notes_list_width;
            render_notes_pane(&mut *app, notes_start_x, 1, remaining_width, term_height.saturating_sub(1))?;

            // Restore cursor position
            print!("\x1b[u");

            // END SYNCHRONIZED UPDATE - now display everything at once
            print!("\x1b[?2026l");
            stdout.flush()?;


            // Status bar disabled to prevent debug flood
            // render_status_bar(&mut stdout, app, term_width, term_height)?;

            app.needs_redraw = false;
        }

        // DISABLED: Text pane trackpad gestures are not active yet
        // Momentum-based scrolling is disabled for the text pane
        // if mouse_state.scroll_momentum.velocity_y.abs() > 0.1 ||
        //    mouse_state.scroll_momentum.velocity_x.abs() > 0.1 {
        //     mouse::apply_smooth_scroll(app, &mut mouse_state);
        // }

        // CROSSTERM ELIMINATED! Direct Kitty input
        if KittyTerminal::poll_input()? {
            // Use new unified input API
            if let Some(input) = KittyTerminal::read_input()? {
                match input {
                    kitty_native::InputEvent::Key(key) => {
                        // Track selection changes
                        let old_selection = app.notes_selection.clone();

                        if !keyboard::handle_input(app, key).await? {
                            break;
                        }
                        if app.exit_requested {
                            break;
                        }

                        // Check for changes
                        let selection_changed = app.notes_selection != old_selection;

                        if selection_changed {
                            // Any selection change triggers redraw
                            app.needs_redraw = true;
                        }
                    }
                    kitty_native::InputEvent::Mouse(mouse_event) => {
                        // Handle mouse events
                        mouse::handle_mouse(app, mouse_event, &mut mouse_state).await?;
                    }
                }
            }
        }
    }
    
    Ok(())
}

// PDF rendering removed - notes only

/// Render the notes pane (no borders)
fn render_notes_pane(app: &mut App, x: u16, y: u16, width: u16, height: u16) -> Result<()> {
    // Render title bar at the top
    let title_height = 1;
    let title = if app.editing_title {
        // Show the buffer being edited with a cursor
        format!("{}|", app.title_buffer)
    } else if !app.notes_list.is_empty() && app.selected_note_index < app.notes_list.len() {
        app.notes_list[app.selected_note_index].title.clone()
    } else {
        "Untitled".to_string()
    };

    // Draw title bar with bold text and amber background
    print!("\x1b[{};{}H\x1b[48;2;255;193;7m\x1b[38;2;0;0;0m\x1b[1m{}{}\x1b[0m",
        y + 1, x + 1,
        title,
        " ".repeat((width as usize).saturating_sub(title.len())));

    // Adjust content area to account for title bar
    let content_y = y + title_height;
    let content_height = height.saturating_sub(title_height);

    // Create notes renderer if needed
    if app.notes_display.is_none() {
        let mut renderer = EditPanelRenderer::new(width, content_height);
        renderer.update_from_rope_with_wrap(&app.notes_rope, app.wrap_text);
        app.notes_display = Some(renderer);
    }

    if let Some(renderer) = &mut app.notes_display {
        // Always resize to current width/height in case it changed
        renderer.resize(width, content_height);
        renderer.update_from_rope_with_wrap(&app.notes_rope, app.wrap_text);

        // Always show cursor in notes mode
        let show_cursor = true;

        // Use grid cursor position (can be in virtual space!)
        let cursor_line = app.notes_cursor.row;
        let cursor_col = app.notes_cursor.col;

        renderer.follow_cursor(cursor_col, cursor_line, 3);

        // The cursor position for rendering should be absolute (in buffer coordinates)
        // Not relative to viewport
        let absolute_cursor = (cursor_col, cursor_line);

        // Use full space - no padding
        let content_x = x;
        let display_width = width;
        let display_height = content_height;

        // Render notes with block selection support
        if app.notes_block_selection.is_some() {
            renderer.render_with_block_selection(
                content_x, content_y, display_width, display_height,
                absolute_cursor,
                app.notes_block_selection.as_ref(),
                show_cursor
            )?;
        } else {
            let (sel_start, sel_end) = if show_cursor {
                let range = app.notes_selection.primary();
                // Only show selection highlighting if it's not collapsed to a point
                if range.from() != range.to() {
                    let start_line = app.notes_rope.char_to_line(range.from());
                    let end_line = app.notes_rope.char_to_line(range.to().saturating_sub(1).max(0));
                    let start_line_char = app.notes_rope.line_to_char(start_line);
                    let end_line_char = app.notes_rope.line_to_char(end_line);

                    // Safety: ensure we don't underflow
                    let start_col = range.from().saturating_sub(start_line_char);
                    let end_col = range.to().saturating_sub(end_line_char);

                    (
                        Some((start_col, start_line)),
                        Some((end_col, end_line))
                    )
                } else {
                    (None, None)
                }
            } else {
                (None, None)
            };

            renderer.render_with_cursor_and_selection(
                content_x, content_y, display_width, display_height,
                absolute_cursor,
                sel_start,
                sel_end,
                show_cursor
            )?;
        }
    }

    Ok(())
}

// Text extraction pane removed - notes only

/// Render the minimal notes list sidebar (just numbers)
fn render_notes_list(app: &App, x: u16, y: u16, width: u16, height: u16) -> Result<()> {
    // No borders for minimal design - just a subtle divider line is drawn separately

    // Clear all lines in the notes list area first with bright blue background
    for row in 0..height {
        print!("\x1b[{};{}H\x1b[48;2;30;60;100m{}\x1b[0m", y + row + 1, x + 1, " ".repeat(width as usize));
    }

    // Show notes as simple numbers
    if app.notes_list.is_empty() {
        // Show + for new note
        print!("\x1b[{};{}H\x1b[48;2;30;60;100m\x1b[38;2;200;200;200m +\x1b[0m", y + 2, x + 1);
    } else {
        // Display notes as numbers with scrolling support
        let visible_count = (height - 2) as usize;

        // Use the scroll offset from app
        let start_index = app.notes_list_scroll;
        let end_index = (start_index + visible_count).min(app.notes_list.len());

        for (display_pos, note_idx) in (start_index..end_index).enumerate() {
            let is_selected = note_idx == app.selected_note_index;
            let note = &app.notes_list[note_idx];

            // Highlight selected note with Material Design colors
            let (bg_color, text_color) = if is_selected {
                ("\x1b[48;2;255;193;7m", "\x1b[38;2;0;0;0m")  // Material amber with black text
            } else {
                ("\x1b[48;2;30;60;100m", "\x1b[38;2;220;220;220m")  // Bright blue background with light grey text
            };

            if app.sidebar_expanded {
                // Show number and title (truncated to fit width), or buffer if editing this note
                let title = if app.editing_title && is_selected {
                    format!("{}|", app.title_buffer)
                } else if note.title.is_empty() {
                    "Untitled".to_string()
                } else {
                    note.title.clone()
                };
                // Reserve space for "N. " prefix (number, dot, space)
                let num_prefix = format!("{}. ", note_idx + 1);
                let max_title_len = (width as usize).saturating_sub(num_prefix.len());
                let display_title: String = if title.len() > max_title_len {
                    format!("{}…", &title[..max_title_len.saturating_sub(1)])
                } else {
                    title
                };

                // Make title bold for emphasis with number prefix
                print!("\x1b[{};{}H{}\x1b[1m{}{}{}\x1b[0m",
                    y + display_pos as u16 + 1, x + 1,
                    bg_color, text_color, num_prefix, display_title);
            } else {
                // Show note number (1-indexed for user friendliness)
                let note_num = note_idx + 1;

                // Add indicator: > for selected
                let indicator = if is_selected { "> " } else { "  " };

                // Draw the indicator and note number
                print!("\x1b[{};{}H{}{}{}{}\x1b[0m",
                    y + display_pos as u16 + 1, x,
                    bg_color, text_color, indicator, note_num);
            }
        }

        // Show scroll indicators if needed
        if start_index > 0 {
            // Show up arrow at top (Material green)
            print!("\x1b[{};{}H\x1b[48;2;30;60;100m\x1b[38;2;76;175;80m↑\x1b[0m", y, x + 2);
        }
        if end_index < app.notes_list.len() {
            // Show down arrow at bottom (Material green)
            print!("\x1b[{};{}H\x1b[48;2;30;60;100m\x1b[38;2;76;175;80m↓\x1b[0m", y + height - 1, x + 2);
        }
    }

    Ok(())
}

// Divider rendering removed - notes only, no PDF pane