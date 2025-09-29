// Visual debugging that shows EVERYTHING on screen at once
use crate::App;

pub fn render_debug_panel(app: &App, x: u16, y: u16) -> String {
    let mut output = String::new();

    // Create a floating debug panel in the corner
    let debug_info = vec![
        format!("🎯 Mouse: ({}, {})", app.last_mouse_pos.0, app.last_mouse_pos.1),
        format!("📍 Cursor: ({}, {})",
            if app.active_pane == crate::ActivePane::Left {
                app.notes_cursor.row
            } else {
                app.extraction_cursor.row
            },
            if app.active_pane == crate::ActivePane::Left {
                app.notes_cursor.col
            } else {
                app.extraction_cursor.col
            }),
        format!("📜 Viewport: ({}, {})",
            app.edit_display.as_ref().map(|r| r.viewport_x).unwrap_or(0),
            app.edit_display.as_ref().map(|r| r.viewport_y).unwrap_or(0)),
        format!("🔲 Pane: {:?}", app.active_pane),
        "─────────────────".to_string(),
        "Press 'd' to toggle".to_string(),
    ];

    // Draw box with info
    for (i, line) in debug_info.iter().enumerate() {
        output.push_str(&format!("\x1b[{};{}H\x1b[48;5;234m\x1b[38;5;226m{:20}\x1b[0m",
            y + i as u16, x, line));
    }

    output
}

// Even better: Draw cursor crosshairs
pub fn render_cursor_debug(app: &App, term_width: u16, term_height: u16) -> String {
    if !app.debug_mode {
        return String::new();
    }

    let mut output = String::new();

    // Get cursor screen position
    let (cursor_x, cursor_y) = calculate_cursor_screen_pos(app);

    // Draw crosshairs (subtle dotted lines)
    // Horizontal line
    for x in 0..term_width {
        if x != cursor_x && x % 2 == 0 {
            output.push_str(&format!("\x1b[{};{}H\x1b[38;5;236m·\x1b[0m", cursor_y + 1, x + 1));
        }
    }

    // Vertical line
    for y in 0..term_height {
        if y != cursor_y && y % 2 == 0 {
            output.push_str(&format!("\x1b[{};{}H\x1b[38;5;236m·\x1b[0m", y + 1, cursor_x + 1));
        }
    }

    // Highlight cursor position with colored box
    output.push_str(&format!("\x1b[{};{}H\x1b[48;5;202m \x1b[0m", cursor_y + 1, cursor_x + 1));

    output
}

fn calculate_cursor_screen_pos(app: &App) -> (u16, u16) {
    // This is where we can visually debug coordinate calculations
    let (row, col) = if app.active_pane == crate::ActivePane::Left {
        (app.notes_cursor.row, app.notes_cursor.col)
    } else {
        (app.extraction_cursor.row, app.extraction_cursor.col)
    };

    // Account for pane offset and viewport
    let screen_x = if app.active_pane == crate::ActivePane::Left {
        4 + col as u16  // Notes start at x=4
    } else {
        // Calculate extraction pane start
        let term_width = 100; // Would get from terminal
        let extraction_start = 4 + (term_width - 4) / 2;
        extraction_start + col as u16
    };

    let screen_y = row as u16;

    // Subtract viewport offset to get screen position
    let final_x = screen_x.saturating_sub(
        if app.active_pane == crate::ActivePane::Left {
            app.notes_display.as_ref().map(|r| r.viewport_x as u16).unwrap_or(0)
        } else {
            app.edit_display.as_ref().map(|r| r.viewport_x as u16).unwrap_or(0)
        }
    );

    let final_y = screen_y.saturating_sub(
        if app.active_pane == crate::ActivePane::Left {
            app.notes_display.as_ref().map(|r| r.viewport_y as u16).unwrap_or(0)
        } else {
            app.edit_display.as_ref().map(|r| r.viewport_y as u16).unwrap_or(0)
        }
    );

    (final_x, final_y)
}