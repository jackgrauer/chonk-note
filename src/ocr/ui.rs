// OCR UI components for Chonker 7.58
use anyhow::Result;
use crossterm::{
    execute,
    cursor::MoveTo,
    style::{Color, Print, ResetColor, SetForegroundColor},
};
use std::io;

#[derive(Debug, Clone)]
pub enum OcrStatus {
    Idle,
    Analyzing,
    Processing(f32),  // Progress 0.0-1.0
    Complete(OcrStats),
    Error(String),
}

#[derive(Debug, Clone)]
pub struct OcrStats {
    pub blocks: usize,
    pub confidence: f32,
    pub duration_ms: u64,
}

pub struct OcrMenu {
    pub visible: bool,
    pub status: OcrStatus,
}

impl OcrMenu {
    pub fn new() -> Self {
        Self {
            visible: false,
            status: OcrStatus::Idle,
        }
    }
    
    pub fn show(&mut self) {
        self.visible = true;
        self.status = OcrStatus::Idle;
    }
    
    pub fn hide(&mut self) {
        self.visible = false;
    }
    
    pub fn set_processing(&mut self, progress: f32) {
        self.status = OcrStatus::Processing(progress);
    }
    
    pub fn set_complete(&mut self, stats: OcrStats) {
        self.status = OcrStatus::Complete(stats);
    }
    
    pub fn set_error(&mut self, msg: String) {
        self.status = OcrStatus::Error(msg);
    }
    
    pub fn render(&self, stdout: &mut io::Stdout, y: u16, width: u16) -> Result<()> {
        if !self.visible {
            return Ok(());
        }
        
        // Draw separator line
        execute!(
            stdout,
            MoveTo(0, y),
            SetForegroundColor(Color::Yellow),
            Print("─".repeat(width as usize)),
            ResetColor
        )?;
        
        // Draw status line
        execute!(
            stdout,
            MoveTo(0, y + 1),
            match &self.status {
                OcrStatus::Idle => {
                    SetForegroundColor(Color::Cyan);
                    Print("[A]uto OCR  [F]orce OCR  [R]epair Text  [ESC] Cancel");
                    ResetColor
                },
                OcrStatus::Analyzing => {
                    SetForegroundColor(Color::Yellow);
                    Print("🔍 Analyzing page for OCR needs...");
                    ResetColor
                },
                OcrStatus::Processing(p) => {
                    let bar_width = 30;
                    let filled = (bar_width as f32 * p) as usize;
                    SetForegroundColor(Color::Green);
                    Print(format!("OCR: [{}{}] {:.0}%", 
                        "█".repeat(filled),
                        "░".repeat(bar_width - filled),
                        p * 100.0
                    ));
                    ResetColor
                },
                OcrStatus::Complete(stats) => {
                    SetForegroundColor(Color::Green);
                    Print(format!("✅ OCR Complete: {} blocks, {:.1}% confidence, {}ms",
                        stats.blocks, stats.confidence * 100.0, stats.duration_ms
                    ));
                    ResetColor
                },
                OcrStatus::Error(msg) => {
                    SetForegroundColor(Color::Red);
                    Print(format!("❌ OCR Error: {}", msg));
                    ResetColor
                },
            }
        )?;
        
        Ok(())
    }
}