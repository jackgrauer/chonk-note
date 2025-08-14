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
        
        // Clean, modern, READABLE styling
        
        // Headers - NO BACKGROUNDS, just clean colored text
        skin.set_headers_fg(Color::Rgb { r: 100, g: 180, b: 255 }); // Clean blue
        skin.headers[0].set_fg(Color::Rgb { r: 100, g: 180, b: 255 }); // H1 - Clean blue
        // Remove background - headers[0].set_bg() removed
        skin.headers[1].set_fg(Color::Rgb { r: 80, g: 160, b: 240 }); // H2 - Slightly darker
        skin.headers[2].set_fg(Color::Rgb { r: 60, g: 140, b: 220 });  // H3 - Even darker
        
        // Text styling - subtle and readable
        skin.bold.set_fg(Color::Rgb { r: 255, g: 255, b: 255 }); // Pure white for emphasis
        skin.bold.add_attr(termimad::crossterm::style::Attribute::Bold);
        skin.italic.set_fg(Color::Rgb { r: 180, g: 180, b: 200 }); // Soft gray-blue
        skin.italic.add_attr(termimad::crossterm::style::Attribute::Italic);
        
        // Code - subtle highlighting
        skin.inline_code.set_fg(Color::Rgb { r: 100, g: 220, b: 100 }); // Soft green
        // Remove aggressive background for inline code
        skin.code_block.set_fg(Color::Rgb { r: 150, g: 150, b: 170 }); // Light gray text
        skin.code_block.set_bg(Color::Rgb { r: 30, g: 30, b: 35 }); // Very subtle dark bg
        
        // Lists and quotes - subtle
        skin.bullet.set_fg(Color::Rgb { r: 100, g: 180, b: 255 }); // Match header blue
        skin.quote_mark.set_fg(Color::Rgb { r: 100, g: 100, b: 120 }); // Subtle gray
        
        // Tables - subtle borders
        skin.table.set_fg(Color::Rgb { r: 80, g: 80, b: 100 }); // Dark gray borders
        
        // Strikeout
        skin.strikeout.set_fg(Color::Rgb { r: 100, g: 100, b: 100 });
        skin.strikeout.add_attr(termimad::crossterm::style::Attribute::CrossedOut);
        
        // Paragraph text - clean white, not too bright
        skin.paragraph.set_fg(Color::Rgb { r: 200, g: 200, b: 210 }); // Soft white
        
        // Horizontal rules - subtle
        skin.horizontal_rule.set_fg(Color::Rgb { r: 60, g: 60, b: 80 });
        
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