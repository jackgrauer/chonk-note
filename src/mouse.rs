// Simplified mouse handling for chonk-note
use crate::App;
use crate::kitty_native::MouseEvent;
use crate::config::layout;
use anyhow::Result;

pub struct MouseState {
    pub dragging: bool,
    pub drag_start_row: usize,
    pub drag_start_col: usize,
    pub last_click_time: std::time::Instant,
    pub last_click_row: Option<usize>,
    pub last_click_col: Option<usize>,
}

impl Default for MouseState {
    fn default() -> Self {
        Self {
            dragging: false,
            drag_start_row: 0,
            drag_start_col: 0,
            last_click_time: std::time::Instant::now(),
            last_click_row: None,
            last_click_col: None,
        }
    }
}

pub async fn handle_mouse(
    app: &mut App,
    event: MouseEvent,
    state: &mut MouseState,
) -> Result<()> {
    if event.is_drag {
        handle_mouse_drag(app, event, state).await?;
    } else if event.is_press {
        handle_mouse_down(app, event, state).await?;
    } else {
        handle_mouse_up(app, event, state).await?;
    }

    Ok(())
}

async fn handle_mouse_down(
    app: &mut App,
    event: MouseEvent,
    state: &mut MouseState,
) -> Result<()> {
    let sidebar_width = if app.sidebar_expanded {
        layout::SIDEBAR_WIDTH_EXPANDED
    } else {
        layout::SIDEBAR_WIDTH_COLLAPSED
    };

    let row = event.y;
    let col = event.x;

    // Click on menu bar (row 0)
    if row == 0 {
        handle_menu_bar_click(app, col).await?;
        return Ok(());
    }

    // Check if any menu is open and click is within menu area
    if app.notes_menu_expanded && col < 46 && row >= 2 && row < 18 {
        handle_notes_menu_click(app, row).await?;
        return Ok(());
    }

    // Click anywhere else closes menus
    if app.notes_menu_expanded || app.help_menu_expanded {
        app.notes_menu_expanded = false;
        app.help_menu_expanded = false;
        app.needs_redraw = true;
    }

    // Click in sidebar
    if col < sidebar_width {
        handle_sidebar_click(app, row, col).await?;
        return Ok(());
    }

    // Click in editor area
    let editor_row = row.saturating_sub(1);
    let editor_col = col;

    let buffer_row = app.viewport_row + editor_row as usize;
    let buffer_col = app.viewport_col + (editor_col.saturating_sub(sidebar_width)) as usize;

    // Clamp to buffer bounds
    let buffer_row = buffer_row.min(app.text_buffer.line_count().saturating_sub(1));
    let line_len = app.text_buffer.get_line_length(buffer_row);
    let buffer_col = buffer_col.min(line_len);

    // Double-click detection
    let now = std::time::Instant::now();
    let is_double_click = if let (Some(last_row), Some(last_col)) = (state.last_click_row, state.last_click_col) {
        last_row == buffer_row
            && last_col == buffer_col
            && now.duration_since(state.last_click_time).as_millis() < 500
    } else {
        false
    };

    if is_double_click {
        // Select word or line on double-click
        app.text_buffer.start_selection(buffer_row, 0);
        app.text_buffer.update_selection(buffer_row, line_len);
        // Position cursor at end of selection
        app.cursor_row = buffer_row;
        app.cursor_col = line_len;
        app.needs_redraw = true;
        state.last_click_row = None;
        state.last_click_col = None;
    } else {
        // Single click - position cursor and start drag
        app.cursor_row = buffer_row;
        app.cursor_col = buffer_col;
        state.dragging = true;
        state.drag_start_row = buffer_row;
        state.drag_start_col = buffer_col;
        app.text_buffer.clear_selection();
        app.needs_redraw = true;

        state.last_click_time = now;
        state.last_click_row = Some(buffer_row);
        state.last_click_col = Some(buffer_col);
    }

    Ok(())
}

async fn handle_mouse_drag(
    app: &mut App,
    event: MouseEvent,
    state: &mut MouseState,
) -> Result<()> {
    if !state.dragging {
        return Ok(());
    }

    let sidebar_width = if app.sidebar_expanded {
        layout::SIDEBAR_WIDTH_EXPANDED
    } else {
        layout::SIDEBAR_WIDTH_COLLAPSED
    };

    let row = event.y;
    let col = event.x;

    let editor_row = row.saturating_sub(1);
    let editor_col = col;

    let buffer_row = app.viewport_row + editor_row as usize;
    let buffer_col = app.viewport_col + (editor_col.saturating_sub(sidebar_width)) as usize;

    // Clamp to buffer bounds
    let buffer_row = buffer_row.min(app.text_buffer.line_count().saturating_sub(1));
    let line_len = app.text_buffer.get_line_length(buffer_row);
    let buffer_col = buffer_col.min(line_len);

    // Update selection
    app.text_buffer.start_selection(state.drag_start_row, state.drag_start_col);
    app.text_buffer.update_selection(buffer_row, buffer_col);
    app.cursor_row = buffer_row;
    app.cursor_col = buffer_col;
    app.needs_redraw = true;

    Ok(())
}

async fn handle_mouse_up(
    app: &mut App,
    _event: MouseEvent,
    state: &mut MouseState,
) -> Result<()> {
    state.dragging = false;
    app.needs_redraw = true;
    Ok(())
}

async fn handle_notes_menu_click(app: &mut App, row: u16) -> Result<()> {
    // Notes menu starts at row 2 (row 0 = menu bar, row 1 = separator)
    // Each note is one row
    let note_index = (row - 2) as usize;

    if note_index < app.notes_list.len() {
        // Save current note before switching
        app.save_current_note()?;

        // Load selected note
        app.selected_note_index = note_index;
        let note = &app.notes_list[note_index];
        app.text_buffer = crate::text_buffer::TextBuffer::from_string(&note.content);
        app.cursor_row = 0;
        app.cursor_col = 0;
        app.viewport_row = 0;
        app.viewport_col = 0;
        app.current_note = Some(note.clone());

        // Close menu
        app.notes_menu_expanded = false;
        app.needs_redraw = true;
    }

    Ok(())
}

async fn handle_menu_bar_click(app: &mut App, col: u16) -> Result<()> {
    // Menu positions from main.rs:
    // Notes: column 0-9
    // Help: column 10-17

    if col < 10 {
        // Toggle Notes menu
        app.notes_menu_expanded = !app.notes_menu_expanded;
        app.help_menu_expanded = false;
        app.needs_redraw = true;
    } else if col >= 10 && col < 18 {
        // Toggle Help menu
        app.help_menu_expanded = !app.help_menu_expanded;
        app.notes_menu_expanded = false;
        app.needs_redraw = true;
    }

    Ok(())
}

async fn handle_sidebar_click(app: &mut App, row: u16, _col: u16) -> Result<()> {
    let row = row.saturating_sub(1);

    // Click on note in sidebar
    if (row as usize) < app.notes_list.len() {
        let note_index = app.notes_list_scroll + row as usize;
        if note_index < app.notes_list.len() {
            app.save_current_note()?;
            app.selected_note_index = note_index;
            let note = &app.notes_list[note_index];
            app.text_buffer = crate::text_buffer::TextBuffer::from_string(&note.content);
            app.cursor_row = 0;
            app.cursor_col = 0;
            app.viewport_row = 0;
            app.viewport_col = 0;
            app.current_note = Some(note.clone());

            // Expand sidebar if collapsed
            if !app.sidebar_expanded {
                app.sidebar_expanded = true;
            }
            app.needs_redraw = true;
        }
    }

    Ok(())
}
