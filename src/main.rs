// MINIMAL CHONKER - Just PDF text extraction to editable grid
use anyhow::Result;
use crossterm::{
    cursor::{Hide, MoveTo, Show},
    event::{self, Event, KeyModifiers, EnableMouseCapture, DisableMouseCapture},
    execute,
    style::{Print, ResetColor, SetBackgroundColor, SetForegroundColor},
    terminal::{self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
// TODO: Replace above with kitty_native::KittyTerminal
use std::{io::{self, Write}, path::PathBuf, time::{Duration, Instant}};
use image::DynamicImage;
use clap::Parser;

mod content_extractor;
mod edit_renderer;
mod pdf_renderer;
mod file_picker;
mod theme;
mod viuer_display;
mod keyboard;
mod kitty_native;

use edit_renderer::EditPanelRenderer;
use theme::ChonkerTheme;

#[cfg(target_os = "macos")]
const MOD_KEY: KeyModifiers = KeyModifiers::SUPER;
#[cfg(not(target_os = "macos"))]
const MOD_KEY: KeyModifiers = KeyModifiers::CONTROL;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ExtractionMethod {
    Segments,    // Current PDFium segments method
    PdfAlto,     // PDFAlto-style word-by-word extraction
    LeptessOCR,  // Leptess OCR extraction
}

#[derive(Parser, Debug)]
#[command(name = "chonker7", author, version, about)]
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
    pub needs_redraw: bool,
    pub open_file_picker: bool,
    // Cursor acceleration
    pub last_key_time: Option<Instant>,
    pub last_key_code: Option<crossterm::event::KeyCode>,
    pub key_repeat_count: u32,
    // Display mode
    pub dual_pane_mode: bool,
    // Extraction method
    pub extraction_method: ExtractionMethod,
    // Viewport tracking for flicker prevention
    pub last_viewport_scroll: (u16, u16),
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
            needs_redraw: true,
            open_file_picker: false,
            last_key_time: None,
            last_key_code: None,
            key_repeat_count: 0,
            dual_pane_mode: true,
            extraction_method: ExtractionMethod::Segments,
            last_viewport_scroll: (0, 0),
        })
    }

    pub async fn load_pdf_page(&mut self) -> Result<()> {
        // Render page as image for display
        self.current_page_image = Some(
            pdf_renderer::render_pdf_page(&self.pdf_path, self.current_page, 800, 1000)?
        );

        // Auto-extract text with current method
        self.extract_current_page().await?;

        Ok(())
    }

    pub async fn extract_current_page(&mut self) -> Result<()> {
        // Use current terminal size for dual pane extraction
        let (term_width, term_height) = terminal::size().unwrap_or((120, 40));  // TODO: Replace with KittyTerminal::size()
        let text_width = term_width / 2; // Always dual pane mode
        let text_height = term_height.saturating_sub(2);

        self.edit_data = Some(
            content_extractor::extract_to_matrix_with_method(
                &self.pdf_path,
                self.current_page,
                text_width.max(400) as usize,  // Minimum 400 columns
                text_height.max(200) as usize, // Minimum 200 rows
                self.extraction_method
            ).await?
        );

        if let Some(data) = &self.edit_data {
            if let Some(renderer) = &mut self.edit_display {
                renderer.update_buffer(data);
            } else {
                let mut renderer = EditPanelRenderer::new(text_width, text_height);
                renderer.update_buffer(data);
                self.edit_display = Some(renderer);
            }
        }
        let method_name = match self.extraction_method {
            ExtractionMethod::Segments => "Segments",
            ExtractionMethod::PdfAlto => "PDFAlto",
            ExtractionMethod::LeptessOCR => "OCR",
        };
        self.status_message = format!("Extracted with {} method", method_name);
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
            self.needs_redraw = true;
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
            self.needs_redraw = true;
        }
    }

    // Calculate movement speed based on key repeat
    pub fn update_key_repeat(&mut self, key_code: crossterm::event::KeyCode) -> usize {
        let now = Instant::now();

        // Check if this is the same key as last time
        if let (Some(last_time), Some(last_key)) = (self.last_key_time, self.last_key_code) {
            let time_since_last = now.duration_since(last_time);

            // If same key and within repeat threshold (200ms), increment count
            if key_code == last_key && time_since_last < Duration::from_millis(200) {
                self.key_repeat_count += 1;
            } else {
                self.key_repeat_count = 1;
            }
        } else {
            self.key_repeat_count = 1;
        }

        self.last_key_time = Some(now);
        self.last_key_code = Some(key_code);

        // Calculate movement speed: 1 for first press, then accelerate
        match self.key_repeat_count {
            1..=3 => 1,           // Normal speed for first few presses
            4..=8 => 3,           // 3x speed after holding briefly
            9..=15 => 6,          // 6x speed for sustained holding
            _ => 10,              // Max 10x speed for long holds
        }
    }

    // Force re-extraction (always dual pane mode now)
    pub async fn refresh_extraction(&mut self) -> Result<()> {
        self.extract_current_page().await?;
        self.needs_redraw = true;
        Ok(())
    }

    // Toggle between extraction methods
    pub async fn toggle_extraction_method(&mut self) -> Result<()> {
        self.extraction_method = match self.extraction_method {
            ExtractionMethod::Segments => ExtractionMethod::PdfAlto,
            ExtractionMethod::PdfAlto => ExtractionMethod::LeptessOCR,
            ExtractionMethod::LeptessOCR => ExtractionMethod::Segments,
        };

        // Show immediate feedback before processing
        let method_name = match self.extraction_method {
            ExtractionMethod::Segments => "Segments",
            ExtractionMethod::PdfAlto => "PDFAlto",
            ExtractionMethod::LeptessOCR => "OCR",
        };

        // Re-extract with new method
        if self.extraction_method == ExtractionMethod::LeptessOCR {
            // NUCLEAR: For OCR, disable redraws completely during processing
            self.status_message = "Processing OCR...".to_string();

            // Process OCR in background without updating display
            let ocr_result = content_extractor::extract_to_matrix_with_method(
                &self.pdf_path,
                self.current_page,
                400, // Fixed size for OCR
                200,
                self.extraction_method
            ).await;

            // Only update if OCR succeeded, otherwise keep current text
            if let Ok(data) = ocr_result {
                self.edit_data = Some(data);
                if let Some(edit_data) = &self.edit_data {
                    if let Some(renderer) = &mut self.edit_display {
                        renderer.update_buffer(edit_data);
                    }
                }
            }

            self.status_message = "OCR complete".to_string();
        } else {
            // For other methods, extract normally
            self.extract_current_page().await?;
        }

        self.needs_redraw = true;

        // Update final status message
        self.status_message = format!("Switched to {} extraction", method_name);

        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // NUCLEAR: Completely disable stderr and redirect to /dev/null
    unsafe {
        let dev_null = libc::open(b"/dev/null\0".as_ptr() as *const i8, libc::O_WRONLY);
        if dev_null != -1 {
            libc::dup2(dev_null, libc::STDERR_FILENO);
            libc::close(dev_null);
        }
    }

    let args = Args::parse();
    
    let pdf_path = if let Some(path) = args.pdf_file {
        path
    } else {
        // Use file picker
        if let Some(path) = file_picker::pick_pdf_file()? {
            path
        } else {
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

    // TODO: Replace with KittyTerminal::enter_fullscreen()
    // Hybrid: Kitty detection with crossterm fallback for now
    if std::env::var("KITTY_WINDOW_ID").is_ok() ||
       std::env::var("TERM_PROGRAM").unwrap_or_default().contains("kitty") {
        // Use Kitty fullscreen mode to hide banner
        execute!(io::stdout(),
            Print("\x1b[?1049h"),  // Save screen and enter alternate buffer
            Print("\x1b[2J"),      // Clear screen
            Print("\x1b[H"),       // Move to top-left
            Print("\x1b[?25l"),    // Hide cursor
            Print("\x1b[?1000h"),  // Enable mouse tracking
        )?;
    } else {
        // Standard terminal setup
        execute!(io::stdout(), EnterAlternateScreen, Hide, EnableMouseCapture)?;
    }

    Ok(())
}

fn restore_terminal() -> Result<()> {
    let _ = viuer_display::clear_graphics();

    // TODO: Replace with KittyTerminal::exit_fullscreen()
    // Hybrid: Kitty detection with crossterm fallback for now
    if std::env::var("KITTY_WINDOW_ID").is_ok() ||
       std::env::var("TERM_PROGRAM").unwrap_or_default().contains("kitty") {
        execute!(io::stdout(),
            Print("\x1b[?1000l"),  // Disable mouse tracking
            Print("\x1b[?25h"),    // Show cursor
            Print("\x1b[2J"),      // Clear screen
            Print("\x1b[H"),       // Move to top-left
            Print("\x1b[?1049l"),  // Restore screen and exit alternate buffer
        )?;
    } else {
        execute!(io::stdout(), Clear(ClearType::All), MoveTo(0, 0))?;
        execute!(io::stdout(), Show, LeaveAlternateScreen, DisableMouseCapture)?;
    }

    disable_raw_mode()?;
    Ok(())
}

async fn run_app(app: &mut App) -> Result<()> {
    let mut stdout = io::stdout();
    let mut last_term_size = (0, 0);
    let mut last_render_time = std::time::Instant::now();

    // Initial render
    app.needs_redraw = true;
    
    loop {
        let (term_width, term_height) = terminal::size()?;  // TODO: Replace with KittyTerminal::size()
        let split_x = term_width / 2;
        
        // Check if terminal was resized
        if (term_width, term_height) != last_term_size {
            app.needs_redraw = true;
            last_term_size = (term_width, term_height);
        }
        
        // Check if we need to open file picker
        if app.open_file_picker {
            app.open_file_picker = false;
            restore_terminal()?;
            
            if let Some(new_path) = file_picker::pick_pdf_file()? {
                app.pdf_path = new_path;
                app.current_page = 0;
                app.total_pages = content_extractor::get_page_count(&app.pdf_path)?;
                app.load_pdf_page().await?;
                app.needs_redraw = true;
            }
            
            setup_terminal()?;
            app.needs_redraw = true;
        }
        
        // NUCLEAR ANTI-FLICKER: Only redraw when absolutely necessary
        let now = std::time::Instant::now();
        let frame_time = now.duration_since(last_render_time);

        if app.needs_redraw && frame_time.as_millis() >= 50 { // Max 20 FPS - nuclear anti-flicker
            // TODO: Replace with KittyTerminal::move_to(0, 0)
            execute!(stdout, MoveTo(0, 0))?;
            last_render_time = now;

            // Always dual pane mode: PDF on left, text editor on right
            if let Some(image) = &app.current_page_image {
                let _ = viuer_display::display_pdf_image(
                    image, 0, 0, split_x - 1, term_height - 2, app.dark_mode
                );
            }

            // Render text editor on right
            if let Some(renderer) = &app.edit_display {
                renderer.render_with_cursor_and_selection(
                    split_x, 0, term_width - split_x, term_height - 2,
                    app.cursor,
                    app.selection_start,
                    app.selection_end
                )?;
            }

            // Status bar disabled to prevent debug flood
            // render_status_bar(&mut stdout, app, term_width, term_height)?;

            stdout.flush()?;
            app.needs_redraw = false;
        }
        
        // Handle input with reduced polling to prevent flickering
        if event::poll(Duration::from_millis(16))? { // ~60 FPS max
            if let Event::Key(key) = event::read()? {
                let old_cursor = app.cursor;
                let old_selection = (app.selection_start, app.selection_end);
                
                if !keyboard::handle_input(app, key).await? {
                    break;
                }
                if app.exit_requested {
                    break;
                }
                
                // Check if viewport has scrolled by comparing scroll positions
                let current_viewport_scroll = if let Some(renderer) = &app.edit_display {
                    (renderer.scroll_x, renderer.scroll_y)
                } else {
                    (0, 0)
                };

                let viewport_scrolled = current_viewport_scroll != app.last_viewport_scroll;
                let cursor_moved = app.cursor != old_cursor;
                let selection_changed = (app.selection_start, app.selection_end) != old_selection;

                if viewport_scrolled {
                    // Viewport scrolled - need full redraw to prevent flicker
                    app.needs_redraw = true;
                    app.last_viewport_scroll = current_viewport_scroll;
                } else if cursor_moved || selection_changed {
                    // NUCLEAR: Skip partial updates, they cause flicker
                    // Just mark for full redraw but throttled
                    app.needs_redraw = true;
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
    
    let method_name = match app.extraction_method {
        ExtractionMethod::Segments => "SEG",
        ExtractionMethod::PdfAlto => "ALTO",
        ExtractionMethod::LeptessOCR => "OCR",
    };

    let status = format!(
        " Page {}/{} | Method: {} | {} | T:Switch-Method O:Open N/P:Page ↑↓←→:Move C/V:Copy/Paste Q:Quit ",
        app.current_page + 1,
        app.total_pages,
        method_name,
        if app.status_message.is_empty() { "Ready" } else { &app.status_message }
    );
    
    let status_len = status.len();
    execute!(stdout, Print(status))?;
    execute!(stdout, Print(" ".repeat((width as usize).saturating_sub(status_len))))?;
    execute!(stdout, ResetColor)?;
    
    Ok(())
}