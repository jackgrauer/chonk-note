// MINIMAL CHONKER - Just PDF text extraction to editable grid
use anyhow::Result;
use crossterm::{
    cursor::{Hide, MoveTo, Show},
    event::{self, Event, KeyModifiers, EnableMouseCapture, DisableMouseCapture},
    execute,
    style::{Color, Print, ResetColor, SetBackgroundColor, SetForegroundColor},
    terminal::{self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use std::{io::{self, Write}, path::PathBuf, time::Duration};
use image::DynamicImage;
use clap::Parser;

mod content_extractor;
mod edit_renderer;
mod pdf_renderer;
mod file_picker;
mod theme;
mod viuer_display;
mod keyboard;

use edit_renderer::EditPanelRenderer;
use theme::ChonkerTheme;

#[cfg(target_os = "macos")]
const MOD_KEY: KeyModifiers = KeyModifiers::SUPER;
#[cfg(not(target_os = "macos"))]
const MOD_KEY: KeyModifiers = KeyModifiers::CONTROL;

#[derive(Parser, Debug)]
#[command(author, version, about)]
struct Args {
    pdf_file: Option<PathBuf>,
    #[arg(short, long, default_value_t = 1)]
    page: usize,
}

pub struct App {
    pub pdf_path: PathBuf,
    pub current_page: usize,
    pub total_pages: usize,
    pub edit_data: Option<Vec<Vec<char>>>,
    pub edit_display: Option<EditPanelRenderer>,
    pub current_page_image: Option<DynamicImage>,
    pub cursor: (usize, usize),
    pub selection_start: Option<(usize, usize)>,
    pub selection_end: Option<(usize, usize)>,
    pub is_selecting: bool,
    pub status_message: String,
    pub dark_mode: bool,
    pub exit_requested: bool,
}

impl App {
    pub fn new(pdf_path: PathBuf, start_page: usize) -> Result<Self> {
        let total_pages = content_extractor::get_page_count(&pdf_path)?;
        Ok(Self {
            pdf_path,
            current_page: start_page.saturating_sub(1),
            total_pages,
            edit_data: None,
            edit_display: None,
            current_page_image: None,
            cursor: (0, 0),
            selection_start: None,
            selection_end: None,
            is_selecting: false,
            status_message: String::new(),
            dark_mode: true,
            exit_requested: false,
        })
    }

    pub async fn load_pdf_page(&mut self) -> Result<()> {
        // Render page as image for display
        self.current_page_image = Some(
            pdf_renderer::render_pdf_page(&self.pdf_path, self.current_page, 800, 1000)?
        );
        
        // Extract text to grid
        self.edit_data = Some(
            content_extractor::extract_to_matrix(&self.pdf_path, self.current_page, 200, 100).await?
        );
        
        if let Some(data) = &self.edit_data {
            let mut renderer = EditPanelRenderer::new();
            renderer.update_buffer(data);
            self.edit_display = Some(renderer);
        }
        
        Ok(())
    }

    pub async fn extract_current_page(&mut self) -> Result<()> {
        self.edit_data = Some(
            content_extractor::extract_to_matrix(&self.pdf_path, self.current_page, 200, 100).await?
        );
        
        if let Some(data) = &self.edit_data {
            if let Some(renderer) = &mut self.edit_display {
                renderer.update_buffer(data);
            } else {
                let mut renderer = EditPanelRenderer::new();
                renderer.update_buffer(data);
                self.edit_display = Some(renderer);
            }
        }
        self.status_message = "Text extracted".to_string();
        Ok(())
    }

    pub fn next_page(&mut self) {
        if self.current_page < self.total_pages - 1 {
            let _ = viuer_display::clear_graphics();
            self.current_page += 1;
            self.edit_data = None;
            self.edit_display = None;
            self.current_page_image = None;
            self.cursor = (0, 0);
            self.selection_start = None;
            self.selection_end = None;
        }
    }

    pub fn prev_page(&mut self) {
        if self.current_page > 0 {
            let _ = viuer_display::clear_graphics();
            self.current_page -= 1;
            self.edit_data = None;
            self.edit_display = None;
            self.current_page_image = None;
            self.cursor = (0, 0);
            self.selection_start = None;
            self.selection_end = None;
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    
    let pdf_path = if let Some(path) = args.pdf_file {
        path
    } else {
        // Use file picker
        println!("🐹 Launching Chonker7 file picker...");
        if let Some(path) = file_picker::pick_pdf_file()? {
            println!("Selected: {}", path.display());
            path
        } else {
            println!("No file selected");
            return Ok(());
        }
    };

    let mut app = App::new(pdf_path, args.page)?;
    app.load_pdf_page().await?;
    
    setup_terminal()?;
    let result = run_app(&mut app).await;
    restore_terminal()?;
    
    result
}

fn setup_terminal() -> Result<()> {
    enable_raw_mode()?;
    execute!(io::stdout(), EnterAlternateScreen, Hide, EnableMouseCapture)?;
    Ok(())
}

fn restore_terminal() -> Result<()> {
    let _ = viuer_display::clear_graphics();
    execute!(io::stdout(), Clear(ClearType::All), MoveTo(0, 0))?;
    execute!(io::stdout(), Show, LeaveAlternateScreen, DisableMouseCapture)?;
    disable_raw_mode()?;
    Ok(())
}

async fn run_app(app: &mut App) -> Result<()> {
    let mut stdout = io::stdout();
    
    loop {
        // Clear and render
        execute!(stdout, Clear(ClearType::All), MoveTo(0, 0))?;
        
        let (term_width, term_height) = terminal::size()?;
        
        // Simple split view: left for image, right for text
        let split_x = term_width / 2;
        
        // Render PDF image on left
        if let Some(image) = &app.current_page_image {
            let _ = viuer_display::display_pdf_image(
                image, 0, 0, split_x - 1, term_height - 2, app.dark_mode
            );
        }
        
        // Render text editor on right
        if let Some(renderer) = &mut app.edit_display {
            renderer.render(
                &mut stdout,
                split_x, 0, term_width - split_x, term_height - 2,
                app.cursor,
                app.selection_start,
                app.selection_end,
                app.dark_mode
            )?;
        }
        
        // Status bar
        render_status_bar(&mut stdout, app, term_width, term_height)?;
        
        stdout.flush()?;
        
        // Handle input
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if !keyboard::handle_input(app, key).await? {
                    break;
                }
                if app.exit_requested {
                    break;
                }
            }
        }
    }
    
    Ok(())
}

fn render_status_bar(stdout: &mut io::Stdout, app: &App, width: u16, height: u16) -> Result<()> {
    execute!(stdout, MoveTo(0, height - 1))?;
    execute!(stdout, SetBackgroundColor(ChonkerTheme::bg_status_dark()))?;
    execute!(stdout, SetForegroundColor(ChonkerTheme::text_status_dark()))?;
    
    let status = format!(
        " Page {}/{} | {} | Ctrl+C/V: Copy/Paste | Ctrl+Q: Quit ",
        app.current_page + 1,
        app.total_pages,
        if app.status_message.is_empty() { "Ready" } else { &app.status_message }
    );
    
    execute!(stdout, Print(status))?;
    execute!(stdout, Print(" ".repeat((width as usize).saturating_sub(status.len()))))?;
    execute!(stdout, ResetColor)?;
    
    Ok(())
}