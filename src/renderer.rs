use crossterm::{
    cursor::MoveTo,
    execute,
    style::{Color, Print, SetForegroundColor, SetBackgroundColor, ResetColor},
    terminal,
};
use std::io::{self, Write};

pub struct TextRenderer {
    buffer: Vec<Vec<char>>,
    viewport_width: u16,
    viewport_height: u16,
    scroll_x: u16,
    scroll_y: u16,
}

impl TextRenderer {
    pub fn new(width: u16, height: u16) -> Self {
        Self {
            buffer: vec![vec![' '; width as usize]; height as usize],
            viewport_width: width,
            viewport_height: height,
            scroll_x: 0,
            scroll_y: 0,
        }
    }
    
    pub fn update_buffer(&mut self, matrix: &[Vec<char>]) {
        self.buffer.clear();
        for row in matrix {
            self.buffer.push(row.clone());
        }
    }
    
    pub fn scroll_up(&mut self, lines: u16) {
        self.scroll_y = self.scroll_y.saturating_sub(lines);
    }
    
    pub fn scroll_down(&mut self, lines: u16) {
        let max_scroll = self.buffer.len().saturating_sub(self.viewport_height as usize) as u16;
        self.scroll_y = (self.scroll_y + lines).min(max_scroll);
    }
    
    pub fn scroll_left(&mut self, cols: u16) {
        self.scroll_x = self.scroll_x.saturating_sub(cols);
    }
    
    pub fn scroll_right(&mut self, cols: u16) {
        let max_width = self.buffer.get(0).map(|r| r.len()).unwrap_or(0);
        let max_scroll = max_width.saturating_sub(self.viewport_width as usize) as u16;
        self.scroll_x = (self.scroll_x + cols).min(max_scroll);
    }
    
    /// Efficiently render the text buffer to the terminal within bounds
    pub fn render(&self, start_x: u16, start_y: u16, max_width: u16, max_height: u16) -> io::Result<()> {
        let mut stdout = io::stdout();
        
        // Clamp rendering to the specified bounds
        let render_width = self.viewport_width.min(max_width);
        let render_height = self.viewport_height.min(max_height);
        
        // Build the entire screen content in one go
        let mut screen_buffer = String::with_capacity(
            (render_width * render_height * 2) as usize
        );
        
        for y in 0..render_height {
            let buffer_y = (self.scroll_y + y) as usize;
            
            // Move cursor to start of line
            execute!(stdout, MoveTo(start_x, start_y + y))?;
            
            if buffer_y < self.buffer.len() {
                let row = &self.buffer[buffer_y];
                let start_col = self.scroll_x as usize;
                let end_col = (start_col + render_width as usize).min(row.len());
                
                // Build the entire line at once, but truncate to render_width
                screen_buffer.clear();
                for x in start_col..end_col {
                    screen_buffer.push(row[x]);
                }
                
                // Pad with spaces if needed
                let chars_written = end_col - start_col;
                if chars_written < render_width as usize {
                    for _ in chars_written..render_width as usize {
                        screen_buffer.push(' ');
                    }
                }
                
                // Write the entire line in one go
                write!(stdout, "{}", screen_buffer)?;
            } else {
                // Clear the rest of the viewport
                write!(stdout, "{:width$}", "", width = render_width as usize)?;
            }
        }
        
        stdout.flush()?;
        Ok(())
    }
    
    /// Render with highlighting for search results or selections
    pub fn render_with_highlights(
        &self,
        start_x: u16,
        start_y: u16,
        highlights: &[(usize, usize, usize, usize)], // (start_y, start_x, end_y, end_x)
    ) -> io::Result<()> {
        let mut stdout = io::stdout();
        
        for y in 0..self.viewport_height {
            let buffer_y = (self.scroll_y + y) as usize;
            execute!(stdout, MoveTo(start_x, start_y + y))?;
            
            if buffer_y < self.buffer.len() {
                let row = &self.buffer[buffer_y];
                let start_col = self.scroll_x as usize;
                let end_col = (start_col + self.viewport_width as usize).min(row.len());
                
                for x in start_col..end_col {
                    let is_highlighted = highlights.iter().any(|(sy, sx, ey, ex)| {
                        (buffer_y > *sy || (buffer_y == *sy && x >= *sx)) &&
                        (buffer_y < *ey || (buffer_y == *ey && x <= *ex))
                    });
                    
                    if is_highlighted {
                        execute!(
                            stdout,
                            SetBackgroundColor(Color::Yellow),
                            SetForegroundColor(Color::Black),
                            Print(row[x]),
                            ResetColor
                        )?;
                    } else {
                        write!(stdout, "{}", row[x])?;
                    }
                }
                
                // Clear rest of line
                let chars_written = end_col - start_col;
                if chars_written < self.viewport_width as usize {
                    write!(stdout, "{:width$}", "", width = (self.viewport_width as usize - chars_written))?;
                }
            } else {
                write!(stdout, "{:width$}", "", width = self.viewport_width as usize)?;
            }
        }
        
        stdout.flush()?;
        Ok(())
    }
    
    pub fn resize(&mut self, width: u16, height: u16) {
        self.viewport_width = width;
        self.viewport_height = height;
    }
}