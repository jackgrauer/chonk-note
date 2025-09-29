// Debug overlay that shows coordinate info without disrupting the UI
use std::fmt::Write as _;

pub struct DebugInfo {
    pub mouse_screen: (u16, u16),
    pub mouse_document: (usize, usize),
    pub cursor_grid: (usize, usize),
    pub viewport: (usize, usize),
    pub active_pane: String,
}

impl DebugInfo {
    pub fn render(&self) -> String {
        let mut output = String::new();

        // Render in the terminal's title bar (non-intrusive!)
        write!(
            output,
            "\x1b]0;Mouse:({},{})→Doc:({},{}) Cursor:({},{}) Viewport:({},{}) Pane:{}\x07",
            self.mouse_screen.0, self.mouse_screen.1,
            self.mouse_document.0, self.mouse_document.1,
            self.cursor_grid.0, self.cursor_grid.1,
            self.viewport.0, self.viewport.1,
            self.active_pane
        ).unwrap();

        output
    }
}

// Alternative: Status line at bottom
pub fn render_status_line(info: &DebugInfo, width: u16) -> String {
    let status = format!(
        " M:({},{}) C:({},{}) V:({},{}) {}",
        info.mouse_screen.0, info.mouse_screen.1,
        info.cursor_grid.0, info.cursor_grid.1,
        info.viewport.0, info.viewport.1,
        info.active_pane
    );

    format!("\x1b[7m{:width$}\x1b[0m", status, width = width as usize)
}