// Viewport with auto-hiding scrollbars for chonk-note
use std::io::{self, Write};

pub struct Viewport {
    // Viewport dimensions (terminal size)
    pub view_width: usize,
    pub view_height: usize,

    // Content dimensions (total size of scrollable content)
    pub content_width: usize,
    pub content_height: usize,

    // Scroll position (top-left corner of visible area)
    pub scroll_x: usize,
    pub scroll_y: usize,

    // Offset for positioning (e.g., skip title bar)
    pub y_offset: usize,
}

impl Viewport {
    pub fn new(view_width: u16, view_height: u16) -> Self {
        Self {
            view_width: view_width as usize,
            view_height: view_height as usize,
            content_width: 0,
            content_height: 0,
            scroll_x: 0,
            scroll_y: 0,
            y_offset: 1, // Default: skip 1 row for title bar
        }
    }

    pub fn set_y_offset(&mut self, offset: usize) {
        self.y_offset = offset;
    }

    pub fn resize(&mut self, width: u16, height: u16) {
        self.view_width = width as usize;
        self.view_height = height as usize;
        self.clamp_scroll();
    }

    pub fn set_content_size(&mut self, width: usize, height: usize) {
        self.content_width = width;
        self.content_height = height;
        self.clamp_scroll();
    }

    pub fn set_scroll(&mut self, x: usize, y: usize) {
        self.scroll_x = x;
        self.scroll_y = y;
        self.clamp_scroll();
    }

    fn clamp_scroll(&mut self) {
        // Allow unlimited scrolling - no clamping needed
        // Scrollbars can now move into virtual space beyond content
    }

    pub fn needs_vertical_scrollbar(&self) -> bool {
        true  // Always show vertical scrollbar
    }

    pub fn needs_horizontal_scrollbar(&self) -> bool {
        true  // Always show horizontal scrollbar
    }

    pub fn draw_scrollbars(&self) -> io::Result<()> {
        let mut stdout = io::stdout();

        let _ = (|| -> std::io::Result<()> {
            use std::io::Write;
            let mut f = std::fs::OpenOptions::new().create(true).append(true).open("/tmp/chonk-debug.log")?;
            writeln!(f, "DRAW_SCROLLBARS called: needs_v={}, needs_h={}",
                self.needs_vertical_scrollbar(), self.needs_horizontal_scrollbar())?;
            f.flush()
        })();

        // Draw vertical scrollbar
        if self.needs_vertical_scrollbar() {
            let scrollbar_height = self.view_height;

            // Calculate thumb size and position
            // When content <= viewport, thumb fills entire scrollbar and doesn't move
            let (thumb_height, thumb_position) = if self.content_height <= self.view_height {
                (scrollbar_height, 0)
            } else {
                let thumb_h = ((self.view_height as f64 / self.content_height as f64) * scrollbar_height as f64).max(1.0) as usize;
                let thumb_pos = ((self.scroll_y as f64 / self.content_height as f64) * scrollbar_height as f64) as usize;
                (thumb_h, thumb_pos)
            };

            let _ = (|| -> std::io::Result<()> {
                use std::io::Write;
                let mut f = std::fs::OpenOptions::new().create(true).append(true).open("/tmp/chonk-debug.log")?;
                writeln!(f, "V_SCROLLBAR: height={}, content_h={}, thumb_h={}, thumb_pos={}, scroll_y={}",
                    scrollbar_height, self.content_height, thumb_height, thumb_position, self.scroll_y)?;
                f.flush()
            })();

            for row in 0..scrollbar_height {
                // Terminal coords are 1-based, mouse coords are 0-based
                let screen_row = row + self.y_offset + 1;
                let screen_col = self.view_width; // 1-based position for rightmost column

                if row >= thumb_position && row < thumb_position + thumb_height {
                    // Scrollbar thumb (bright)
                    print!("\x1b[{};{}H\x1b[48;2;100;100;100m \x1b[0m", screen_row, screen_col);
                } else {
                    // Scrollbar track (dim)
                    print!("\x1b[{};{}H\x1b[48;2;40;40;40m \x1b[0m", screen_row, screen_col);
                }
            }
        }

        // Draw horizontal scrollbar
        if self.needs_horizontal_scrollbar() {
            let scrollbar_width = self.view_width;

            // Calculate thumb size and position
            // When content <= viewport, thumb fills entire scrollbar and doesn't move
            let (thumb_width, thumb_position) = if self.content_width <= self.view_width {
                (scrollbar_width, 0)
            } else {
                let thumb_w = ((self.view_width as f64 / self.content_width as f64) * scrollbar_width as f64).max(1.0) as usize;
                let thumb_pos = ((self.scroll_x as f64 / self.content_width as f64) * scrollbar_width as f64) as usize;
                (thumb_w, thumb_pos)
            };

            // Terminal coords are 1-based, mouse coords are 0-based
            let screen_row = self.view_height + self.y_offset;

            for col in 0..scrollbar_width {
                let screen_col = col + 1; // 1-based terminal position

                if col >= thumb_position && col < thumb_position + thumb_width {
                    // Scrollbar thumb
                    print!("\x1b[{};{}H\x1b[48;2;100;100;100m \x1b[0m", screen_row, screen_col);
                } else {
                    // Scrollbar track
                    print!("\x1b[{};{}H\x1b[48;2;40;40;40m \x1b[0m", screen_row, screen_col);
                }
            }
        }

        stdout.flush()
    }

    pub fn visible_range(&self) -> (usize, usize, usize, usize) {
        let start_x = self.scroll_x;
        let start_y = self.scroll_y;
        let end_x = (self.scroll_x + self.view_width).min(self.content_width);
        let end_y = (self.scroll_y + self.view_height).min(self.content_height);

        (start_x, start_y, end_x, end_y)
    }

    /// Check if a click is on the vertical scrollbar
    pub fn is_click_on_vertical_scrollbar(&self, x: u16, y: u16) -> bool {
        if !self.needs_vertical_scrollbar() {
            return false;
        }

        // Mouse coords are 0-indexed, so rightmost column is view_width - 1
        let scrollbar_col = (self.view_width - 1) as u16;
        let scrollbar_start_row = self.y_offset as u16;
        let scrollbar_end_row = scrollbar_start_row + self.view_height as u16;

        x == scrollbar_col && y >= scrollbar_start_row && y < scrollbar_end_row
    }

    /// Check if a click is on the horizontal scrollbar
    pub fn is_click_on_horizontal_scrollbar(&self, x: u16, y: u16) -> bool {
        if !self.needs_horizontal_scrollbar() {
            return false;
        }

        // Mouse coords are 0-indexed, so bottom row is view_height + y_offset - 1
        let scrollbar_row = (self.view_height + self.y_offset - 1) as u16;

        y == scrollbar_row && x < self.view_width as u16
    }

    /// Handle a click on the vertical scrollbar - returns true if position changed
    pub fn handle_vertical_scrollbar_click(&mut self, y: u16) -> bool {
        if !self.needs_vertical_scrollbar() {
            return false;
        }

        let scrollbar_start_row = self.y_offset as u16;
        let click_offset = (y.saturating_sub(scrollbar_start_row)) as usize;

        // Calculate new scroll position based on click
        let ratio = click_offset as f64 / self.view_height as f64;
        let new_scroll_y = (ratio * self.content_height as f64) as usize;

        if new_scroll_y != self.scroll_y {
            self.scroll_y = new_scroll_y;
            self.clamp_scroll();
            return true;
        }
        false
    }

    /// Handle a click on the horizontal scrollbar - returns true if position changed
    pub fn handle_horizontal_scrollbar_click(&mut self, x: u16) -> bool {
        if !self.needs_horizontal_scrollbar() {
            return false;
        }

        let click_offset = (x.saturating_sub(1)) as usize;

        // Calculate new scroll position based on click
        let ratio = click_offset as f64 / self.view_width as f64;
        let new_scroll_x = (ratio * self.content_width as f64) as usize;

        if new_scroll_x != self.scroll_x {
            self.scroll_x = new_scroll_x;
            self.clamp_scroll();
            return true;
        }
        false
    }
}
