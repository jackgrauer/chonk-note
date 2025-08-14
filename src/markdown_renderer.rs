use termimad::{MadSkin, Area};
use termimad::crossterm::style::Color;
use std::io::{self, Write};
use anyhow::Result;

pub struct MarkdownRenderer {
    skin: MadSkin,
    content: String,
    scroll_offset: usize,
}

impl MarkdownRenderer {
    pub fn new() -> Self {
        let mut skin = MadSkin::default();
        
        // Configure skin colors for dark mode using termimad's crossterm
        skin.set_headers_fg(Color::Cyan);
        skin.bold.set_fg(Color::Yellow);
        skin.italic.set_fg(Color::Magenta);
        skin.inline_code.set_fg(Color::Green);
        skin.code_block.set_bg(Color::Rgb { r: 40, g: 40, b: 40 });
        skin.quote_mark.set_fg(Color::DarkGrey);
        skin.bullet.set_fg(Color::Yellow);
        
        Self {
            skin,
            content: String::new(),
            scroll_offset: 0,
        }
    }
    
    pub fn set_content(&mut self, markdown: &str) {
        self.content = markdown.to_string();
        self.scroll_offset = 0;
    }
    
    pub fn render(&self, x: u16, y: u16, width: u16, height: u16) -> Result<()> {
        let area = Area::new(x, y, width, height);
        
        // Create text view for the area
        let text = self.skin.area_text(&self.content, &area);
        
        // Create scrollable view
        let mut view = termimad::TextView::from(&area, &text);
        view.set_scroll(self.scroll_offset);
        
        // Render to stdout
        let mut stdout = io::stdout();
        view.write_on(&mut stdout)?;
        stdout.flush()?;
        
        Ok(())
    }
    
    pub fn scroll_up(&mut self, lines: usize) {
        self.scroll_offset = self.scroll_offset.saturating_sub(lines);
    }
    
    pub fn scroll_down(&mut self, lines: usize) {
        self.scroll_offset += lines;
    }
}