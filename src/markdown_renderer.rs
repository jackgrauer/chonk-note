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
        
        // Modern, beautiful styling for markdown
        
        // Headers - gradient from bright to medium blue
        skin.set_headers_fg(Color::Rgb { r: 100, g: 200, b: 255 }); // Bright blue
        skin.headers[0].set_fg(Color::Rgb { r: 120, g: 220, b: 255 }); // H1 - Brightest
        skin.headers[0].set_bg(Color::Rgb { r: 20, g: 30, b: 50 });   // Subtle bg
        skin.headers[1].set_fg(Color::Rgb { r: 100, g: 180, b: 240 }); // H2
        skin.headers[2].set_fg(Color::Rgb { r: 80, g: 160, b: 220 });  // H3
        
        // Text styling - clean and modern
        skin.bold.set_fg(Color::Rgb { r: 255, g: 200, b: 100 }); // Warm gold
        skin.italic.set_fg(Color::Rgb { r: 200, g: 150, b: 255 }); // Soft purple
        
        // Code - modern IDE style
        skin.inline_code.set_fg(Color::Rgb { r: 150, g: 255, b: 150 }); // Bright green
        skin.inline_code.set_bg(Color::Rgb { r: 30, g: 35, b: 40 }); // Dark background
        skin.code_block.set_fg(Color::Rgb { r: 200, g: 200, b: 200 }); // Light gray text
        skin.code_block.set_bg(Color::Rgb { r: 25, g: 30, b: 35 }); // Very dark bg
        
        // Lists and quotes - elegant
        skin.bullet.set_fg(Color::Rgb { r: 255, g: 150, b: 100 }); // Coral/orange
        skin.quote_mark.set_fg(Color::Rgb { r: 100, g: 100, b: 150 }); // Muted purple
        
        // Tables - clean borders
        skin.table.set_fg(Color::Rgb { r: 150, g: 150, b: 180 }); // Soft borders
        
        // Strikeout (termimad uses strikeout not strikethrough)
        skin.strikeout.set_fg(Color::Rgb { r: 120, g: 120, b: 120 });
        skin.strikeout.add_attr(termimad::crossterm::style::Attribute::CrossedOut);
        
        // Paragraph text - clean and readable
        skin.paragraph.set_fg(Color::Rgb { r: 220, g: 220, b: 230 }); // Soft white
        
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