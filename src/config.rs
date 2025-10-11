/// Configuration constants for chonk-note editor

/// UI Layout Constants
pub mod layout {
    pub const SIDEBAR_WIDTH_EXPANDED: u16 = 30;
    pub const SIDEBAR_WIDTH_COLLAPSED: u16 = 0;
    pub const SETTINGS_PANEL_WIDTH: u16 = 35;
    pub const GRID_VERTICAL_SPACING: usize = 8;
    pub const GRID_HORIZONTAL_SPACING: usize = 4;
    pub const VISIBLE_NOTE_COUNT_APPROX: usize = 30;
}

/// Timing and Performance Constants
pub mod timing {
    pub const FRAME_TIME_MS: u128 = 8; // 120 FPS for responsive cursor movement
    pub const SAVE_INTERVAL_MS: u128 = 2000; // 2 seconds auto-save debounce
}

/// Navigation Constants
pub mod navigation {
    pub const PAGE_JUMP_ROWS: usize = 20; // Number of rows to jump for Page Up/Down
    pub const PAGE_JUMP_COLS: usize = 20; // Number of columns to jump for horizontal scrolling
}

/// Color Theme (RGB values)
pub mod colors {
    /// Title bar colors
    pub const TITLE_BAR_BG: (u8, u8, u8) = (0, 128, 128); // Teal
    pub const TITLE_BAR_FG: (u8, u8, u8) = (255, 255, 255); // White

    /// Selection colors
    pub const SELECTION_BG: (u8, u8, u8) = (255, 20, 147); // Deep pink
    pub const SELECTION_FG: (u8, u8, u8) = (255, 255, 255); // White

    /// Grid line colors
    pub const GRID_LINE_FG: (u8, u8, u8) = (60, 60, 60); // Dark gray

    /// Sidebar colors
    pub const SIDEBAR_BG: (u8, u8, u8) = (30, 60, 100); // Dark blue
    pub const SIDEBAR_FG: (u8, u8, u8) = (220, 220, 220); // Light gray
    pub const SIDEBAR_ICON_FG: (u8, u8, u8) = (200, 200, 200); // Slightly darker gray
    pub const SIDEBAR_SCROLL_FG: (u8, u8, u8) = (76, 175, 80); // Green

    /// Selected item colors
    pub const SELECTED_ITEM_BG: (u8, u8, u8) = (255, 193, 7); // Amber/Gold
    pub const SELECTED_ITEM_FG: (u8, u8, u8) = (0, 0, 0); // Black
}

/// Helper function to format RGB color for terminal escape code
pub fn rgb_bg(r: u8, g: u8, b: u8) -> String {
    format!("\x1b[48;2;{};{};{}m", r, g, b)
}

/// Helper function to format RGB foreground color for terminal escape code
pub fn rgb_fg(r: u8, g: u8, b: u8) -> String {
    format!("\x1b[38;2;{};{};{}m", r, g, b)
}
