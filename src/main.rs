// Pure crossterm implementation - no ratatui, no tearing!
use anyhow::Result;
use crossterm::{
    cursor::{Hide, MoveTo, Show},
    event::{self, Event, KeyModifiers, MouseEvent, EnableMouseCapture, DisableMouseCapture},
    execute,
    style::{Color, Print, ResetColor, SetBackgroundColor, SetForegroundColor, Attribute, SetAttribute},
    terminal::{
        self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen,
        disable_raw_mode, enable_raw_mode,
    },
};
use std::{
    io::{self, Write},
    path::PathBuf,
    time::Duration,
    sync::{Arc, Mutex},
};
use image::DynamicImage;
use clap::Parser;

// Cross-platform modifier key constant for TEXT mode
#[cfg(target_os = "macos")]
const MOD_KEY: KeyModifiers = KeyModifiers::SUPER; // Cmd key on macOS
#[cfg(not(target_os = "macos"))]
const MOD_KEY: KeyModifiers = KeyModifiers::CONTROL; // Ctrl key elsewhere

// Existing modules
mod content_extractor;
use content_extractor::MlProcessingStats;
mod edit_renderer;
mod pdf_renderer;
mod file_picker;
mod theme;
mod viuer_display;
mod markdown_renderer;
mod keyboard;
mod two_pass;
mod debug_capture;

#[cfg(feature = "ml")]
mod ml;

#[cfg(feature = "ocr")]
mod ocr;

use edit_renderer::EditPanelRenderer;
use theme::ChonkerTheme;
use markdown_renderer::MarkdownRenderer;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// PDF file to view (opens file dialog if not provided)
    pdf_file: Option<PathBuf>,
    
    /// Starting page number (1-indexed)
    #[arg(short, long, default_value_t = 1)]
    page: usize,
    
    /// Display mode: edit or markdown
    #[arg(short, long, default_value = "edit")]
    mode: String,
}

/// Simple rectangle for layout calculations
#[derive(Clone, Copy, Debug)]
struct Rect {
    x: u16,
    y: u16,
    width: u16,
    height: u16,
}

impl Rect {
    fn new(x: u16, y: u16, width: u16, height: u16) -> Self {
        Self { x, y, width, height }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DisplayMode {
    PdfText,     // Raw text extraction mode (editable)
    PdfReader,   // Formatted reader mode (markdown)
    Debug,       // Debug console mode
}

#[derive(Clone, Debug)]
pub struct AppSettings {
    // Vision mode is now always enabled
}

pub struct App {
    pub pdf_path: PathBuf,
    pub current_page: usize,
    pub total_pages: usize,
    pub display_mode: DisplayMode,
    pub edit_data: Option<Vec<Vec<char>>>,
    pub edit_display: Option<EditPanelRenderer>,
    pub current_page_image: Option<DynamicImage>,
    pub markdown_data: Option<String>,
    pub markdown_renderer: Option<MarkdownRenderer>,
    pub settings: AppSettings,
    pub exit_requested: bool,
    pub status_message: String,
    pub ml_stats: Option<MlProcessingStats>,
    pub ml_debug_visible: bool,
    pub debug_console: Vec<String>,  // Store all debug output
    pub debug_scroll_offset: usize,  // For scrolling through debug
    pub term_width: u16,
    pub term_height: u16,
    pub dark_mode: bool, // Dark mode toggle
    // TEXT mode cursor and selection
    pub cursor: (usize, usize),  // (x, y) in grid
    pub selection_start: Option<(usize, usize)>,
    pub selection_end: Option<(usize, usize)>,
    pub is_selecting: bool,
    // OCR support
    #[cfg(feature = "ocr")]
    pub ocr_layer: ocr::OcrLayer,
    #[cfg(feature = "ocr")]
    pub ocr_menu: ocr::OcrMenu,
}

impl App {
    pub fn new(pdf_path: PathBuf, starting_page: usize, mode: &str) -> Result<Self> {
        let (width, height) = terminal::size()?;
        let total_pages = content_extractor::get_page_count(&pdf_path)?;
        
        let display_mode = match mode {
            "markdown" | "reader" => DisplayMode::PdfReader,
            _ => DisplayMode::PdfText,
        };
        
        Ok(Self {
            pdf_path,
            current_page: starting_page.saturating_sub(1),
            total_pages,
            display_mode,
            edit_data: None,
            edit_display: None,
            current_page_image: None,
            markdown_data: None,
            markdown_renderer: None,
            settings: AppSettings {
                // Vision mode is always enabled
            },
            exit_requested: false,
            status_message: format!("Page {}/{} - Ctrl+O: Open | Ctrl+E: Extract | Tab: Switch Mode", starting_page, total_pages),
            ml_stats: None,
            ml_debug_visible: false,
            debug_console: Vec::new(),
            debug_scroll_offset: 0,
            term_width: width,
            term_height: height,
            dark_mode: true, // Default to dark mode
            cursor: (0, 0),
            selection_start: None,
            selection_end: None,
            is_selecting: false,
            #[cfg(feature = "ocr")]
            ocr_layer: ocr::OcrLayer::new(),
            #[cfg(feature = "ocr")]
            ocr_menu: ocr::OcrMenu::new(),
        })
    }
    
    fn update_terminal_size(&mut self) -> Result<()> {
        let (width, height) = terminal::size()?;
        self.term_width = width;
        self.term_height = height;
        
        // Update renderer size if it exists
        if let Some(renderer) = &mut self.edit_display {
            let renderer_width = if matches!(self.display_mode, DisplayMode::PdfText | DisplayMode::PdfReader) {
                width / 2 - 2
            } else {
                width - 2
            };
            renderer.resize(renderer_width, height - 5);
        }
        
        Ok(())
    }
    
    pub async fn load_pdf_page(&mut self) -> Result<()> {
        self.status_message = "Loading PDF...".to_string();
        
        // Calculate PDF size based on display mode
        let (image_width, image_height) = match self.display_mode {
            DisplayMode::PdfText | DisplayMode::PdfReader => {
                // For split modes - use actual panel size
                let width = ((self.term_width / 2) - 4).max(40) as u32;
                let height = (self.term_height - 4).max(20) as u32;
                // Ultra-high resolution with aspect ratio correction
                // Terminal cells are ~2:1 (height:width), so we scale height by 1.8x
                (width * 14, (height * 14 * 18) / 10)  // 1.8x height for better aspect ratio
            }
            DisplayMode::Debug => {
                // Debug mode doesn't show PDF, but we need some values
                (800, 600)
            }
        };
        
        match pdf_renderer::render_pdf_page(&self.pdf_path, self.current_page, image_width, image_height) {
            Ok(image) => {
                self.current_page_image = Some(image);
                self.status_message = format!("Page {}/{} - Ctrl+O: Open | Ctrl+E: Extract | Ctrl+R: OCR", self.current_page + 1, self.total_pages);
            }
            Err(e) => {
                self.current_page_image = None;
                self.status_message = format!("Failed to load page {} - {}", 
                    self.current_page + 1, 
                    match e.to_string().as_str() {
                        s if s.contains("not found") => "File not found",
                        s if s.contains("permission") => "Permission denied",
                        s if s.contains("memory") => "Out of memory",
                        _ => "Unable to render page"
                    }
                );
            }
        }
        
        Ok(())
    }
    
    pub async fn extract_current_page(&mut self) -> Result<()> {
        self.status_message = "Extracting content...".to_string();
        
        // Clear debug console for new extraction
        debug_capture::clear_debug_buffer();
        self.debug_console.clear();
        self.debug_console.push(format!("=== Extracting Page {} ===", self.current_page));
        
        // Much larger dimensions to capture full table width
        let matrix_width = 400;  // Wide enough for tables
        let matrix_height = 200; // Tall enough for full content
        
        // Use two-pass mode (PDFium + ML)
        self.status_message = "Extracting content with PDFium + ML...".to_string();
        let (matrix, ml_stats) = content_extractor::extract_to_matrix(
            &self.pdf_path,
            self.current_page,
            matrix_width,
            matrix_height,
        ).await?;
        
        // Store ML stats for status display
        self.ml_stats = Some(ml_stats.clone());
        
        // Add extraction summary to debug console
        self.debug_console.push(format!("Extraction Mode: Two-Pass (PDFium + ML)"));
        self.debug_console.push(format!("ML Active: {}", ml_stats.ml_active));
        self.debug_console.push(format!("Processing Method: {}", ml_stats.processing_method));
        self.debug_console.push(format!("Entities Detected: {}", ml_stats.entities_detected));
        self.debug_console.push(format!("Columns Detected: {}", ml_stats.columns_detected));
        self.debug_console.push(format!("Matrix Size: {}x{}", matrix_width, matrix_height));
        
        // Sync debug messages from extraction process
        let captured_debug = debug_capture::get_debug_messages();
        if !captured_debug.is_empty() {
            self.debug_console.push("--- Console Output ---".to_string());
            self.debug_console.extend(captured_debug);
        }
        
        // Create or update renderer
        let renderer_width = if matches!(self.display_mode, DisplayMode::PdfText | DisplayMode::PdfReader) {
            self.term_width / 2 - 2
        } else {
            self.term_width - 2
        };
        
        if self.edit_display.is_none() {
            let mut renderer = EditPanelRenderer::new(renderer_width, matrix_height as u16);
            renderer.update_buffer(&matrix);
            self.edit_display = Some(renderer);
        } else {
            if let Some(renderer) = &mut self.edit_display {
                renderer.resize(renderer_width, matrix_height as u16);
                renderer.update_buffer(&matrix);
            }
        }
        
        self.edit_data = Some(matrix);
        
        // Initialize cursor at top-left when new content is loaded
        self.cursor = (0, 0);
        self.selection_start = None;
        self.selection_end = None;
        self.is_selecting = false;
        
        // Extract markdown
        let markdown = content_extractor::get_markdown_content(&self.pdf_path, self.current_page).await?;
        
        // Create or update markdown renderer
        if self.markdown_renderer.is_none() {
            self.markdown_renderer = Some(MarkdownRenderer::new());
        }
        
        if let Some(renderer) = &mut self.markdown_renderer {
            renderer.set_content(&markdown);
        }
        
        self.markdown_data = Some(markdown);
        
        // Create enhanced status message with ML info
        let base_status = format!("Page {}/{} - Content extracted | Ctrl+R: OCR", self.current_page + 1, self.total_pages);
        self.status_message = if let Some(ref stats) = self.ml_stats {
            if stats.ml_active {
                format!("{} | 🧠 {} | {}s {}c", 
                    base_status, 
                    stats.processing_method,
                    stats.superscripts_merged,
                    stats.columns_detected
                )
            } else {
                format!("{} | 📄 PDFium Raw", base_status)
            }
        } else {
            base_status
        };
        Ok(())
    }
    
    pub fn next_page(&mut self) {
        if self.current_page < self.total_pages - 1 {
            let _ = viuer_display::clear_graphics();
            self.current_page += 1;
            self.edit_data = None;
            self.current_page_image = None;
            self.edit_display = None; // Clear TEXT renderer
            self.markdown_renderer = None; // Clear READER renderer
            self.markdown_data = None;
            self.status_message = format!("Page {}/{} - Ctrl+O: Open | Ctrl+E: Extract | Ctrl+R: OCR", self.current_page + 1, self.total_pages);
        }
    }
    
    pub fn prev_page(&mut self) {
        if self.current_page > 0 {
            let _ = viuer_display::clear_graphics();
            self.current_page -= 1;
            self.edit_data = None;
            self.current_page_image = None;
            self.edit_display = None; // Clear TEXT renderer
            self.markdown_renderer = None; // Clear READER renderer
            self.markdown_data = None;
            self.status_message = format!("Page {}/{} - Ctrl+O: Open | Ctrl+E: Extract | Ctrl+R: OCR", self.current_page + 1, self.total_pages);
        }
    }
    
    pub fn toggle_mode(&mut self) {
        self.display_mode = match self.display_mode {
            DisplayMode::PdfText => {
                if self.markdown_data.is_some() {
                    DisplayMode::PdfReader
                } else {
                    DisplayMode::Debug  // Go to debug if no markdown
                }
            }
            DisplayMode::PdfReader => DisplayMode::Debug,
            DisplayMode::Debug => DisplayMode::PdfText,
        };
        
        self.status_message = format!("Mode: {:?}", self.display_mode);
    }
}

/// Layout structure for organizing screen regions
struct Layout {
    main: Rect,
    left: Option<Rect>,
    right: Option<Rect>,
    status: Rect,
}

/// Main entry point
#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    
    // Get PDF file
    let pdf_file = match args.pdf_file {
        Some(path) => path,
        None => {
            println!("🐹 Launching Chonker7 file picker...");
            
            match file_picker::pick_pdf_file()? {
                Some(path) => {
                    println!("Selected: {}", path.display());
                    path
                },
                None => {
                    println!("No file selected.");
                    return Ok(());
                }
            }
        }
    };
    
    // Initialize app
    let mut app = App::new(pdf_file, args.page, &args.mode)?;
    
    // Setup terminal
    setup_terminal()?;
    
    // Initial load - just the PDF, not text extraction
    app.load_pdf_page().await?;
    
    // Run the app
    let result = run_app(&mut app).await;
    
    // Always restore terminal
    restore_terminal()?;
    
    if let Err(err) = result {
        eprintln!("Error: {}", err);
    }
    
    Ok(())
}

/// Setup terminal for TUI mode
fn setup_terminal() -> Result<()> {
    enable_raw_mode()?;
    execute!(
        io::stdout(),
        EnterAlternateScreen,
        Hide,
        Clear(ClearType::All),
        EnableMouseCapture
    )?;
    Ok(())
}

/// Restore terminal to normal mode
fn restore_terminal() -> Result<()> {
    // Clear any remaining graphics
    let _ = viuer_display::clear_graphics();
    
    // Clear the screen before leaving
    execute!(
        io::stdout(),
        Clear(ClearType::All),
        MoveTo(0, 0),
        ResetColor,
        Show,
        DisableMouseCapture,
        LeaveAlternateScreen,
        Clear(ClearType::All)  // Extra clear after leaving alternate screen
    )?;
    disable_raw_mode()?;
    
    // Final clear to ensure clean exit
    println!("\x1b[2J\x1b[H");
    
    Ok(())
}

/// Main application loop with SYNCHRONIZED RENDERING
async fn run_app(app: &mut App) -> Result<()> {
    let mut stdout = io::stdout();
    
    loop {
        // Update terminal size
        let _ = app.update_terminal_size();
        
        // ████████████████████████████████████████████████████████████████████████
        // ██                  SYNCHRONIZED RENDER BLOCK                    ██
        // ██                                                                ██
        // ██ This entire block renders atomically to prevent tearing.      ██
        // ██ ALL images must be drawn INSIDE this block, not after!        ██  
        // ██                                                                ██
        // ██ \x1b[?2026h = Begin synchronized update (transaction start)    ██
        // ██ \x1b[?2026l = End synchronized update (transaction commit)     ██
        // ████████████████████████████████████████████████████████████████████████
        
        write!(stdout, "\x1b[?2026h")?; // Begin synchronized update
        
        // Clear screen normally - we need this for proper rendering
        execute!(stdout, Clear(ClearType::All))?;
        
        // Calculate layout
        let layout = calculate_layout(app.term_width, app.term_height, app.display_mode);
        
        // Draw UI chrome
        draw_headers(&mut stdout, &layout, app.display_mode)?;
        
        // Draw OCR menu if visible
        #[cfg(feature = "ocr")]
        {
            if app.ocr_menu.visible {
                app.ocr_menu.render(&mut stdout, app.term_height - 4, app.term_width)?;
            }
        }
        
        draw_status_bar(&mut stdout, app, layout.status)?;
        
        // Draw main content based on mode
        if app.display_mode == DisplayMode::Debug {
            // Show debug console in full screen
            draw_debug_console(&mut stdout, app, layout.main)?;
        } else if let Some(left) = layout.left {
            // Use a dark gray for PDF panel in dark mode for better contrast with black PDFs
            let bg = if app.dark_mode { 
                Color::Rgb { r: 30, g: 30, b: 30 }  // Dark gray instead of pure black
            } else { 
                ChonkerTheme::bg_secondary_light() 
            };
            draw_panel_background(&mut stdout, left, bg)?;
            
            if app.current_page_image.is_none() {
                // Centered loading message
                let msg = "Loading PDF...";
                let msg_x = left.x + (left.width - msg.len() as u16) / 2;
                let msg_y = left.y + left.height / 2;
                execute!(
                    stdout,
                    MoveTo(msg_x, msg_y),
                    SetForegroundColor(ChonkerTheme::accent_pdf()),
                    Print(msg),
                    ResetColor
                )?;
            }
        }
        
        // Right panel - TEXT or READER
        if let Some(right) = layout.right {
            if app.display_mode == DisplayMode::PdfText {
                // Show TEXT panel - unformatted gridlike layout
                if let Some(renderer) = &app.edit_display {
                    renderer.render_with_cursor_and_selection(
                        right.x + 1,
                        right.y,
                        right.width - 2,
                        right.height,
                        app.cursor,
                        app.selection_start,
                        app.selection_end,
                    )?;
                } else {
                    // Centered message
                    let msg = "Press Ctrl+E to extract text";
                    let msg_x = right.x + (right.width - msg.len() as u16) / 2;
                    let msg_y = right.y + right.height / 2;
                    execute!(
                        stdout,
                        MoveTo(msg_x, msg_y),
                        SetForegroundColor(ChonkerTheme::text_dim()),
                        Print(msg),
                        ResetColor
                    )?;
                }
            } else {
                // Show READER panel - nicely formatted, non-editable
                render_markdown(&mut stdout, &app.markdown_renderer, right)?;
            }
        }
        
        // ████████████████████████████████████████████████████████████████████████
        // ██ CRITICAL: DISPLAY IMAGES INSIDE SYNC BLOCK TO PREVENT FLICKER! ██
        // ██                                                                ██
        // ██ Moving display_image() outside the synchronized block causes   ██
        // ██ visible flicker during mode switches. Images MUST be rendered  ██
        // ██ atomically with the UI inside the \x1b[?2026h transaction.     ██
        // ██                                                                ██
        // ██ DO NOT MOVE THIS CODE OUTSIDE THE SYNC BLOCK!                  ██
        // ████████████████████████████████████████████████████████████████████████
        
        if let Some(left) = layout.left {
            if let Some(ref image) = app.current_page_image {
                // Use full left panel for PDF with proper padding
                // Add padding to avoid overlapping with borders
                let padding = 1;
                let display_x = left.x + padding;
                let display_y = left.y + padding;
                let display_width = left.width.saturating_sub(padding * 2);
                let display_height = left.height.saturating_sub(padding * 2);
                
                let _ = viuer_display::display_pdf_image(
                    image,
                    display_x,
                    display_y,
                    display_width,
                    display_height,
                    app.dark_mode,
                );
            }
        }
        
        // ████████████████████████████████████████████████████████████████████████
        // ██                    END SYNCHRONIZED BLOCK                     ██
        // ██                                                                ██
        // ██ Everything above (UI + images) now appears instantly as one   ██
        // ██ atomic operation. This prevents all flicker and tearing.      ██
        // ████████████████████████████████████████████████████████████████████████
        
        write!(stdout, "\x1b[?2026l")?; // End synchronized update
        stdout.flush()?;
        
        // Handle events
        if event::poll(Duration::from_millis(100))? {
            match event::read()? {
                Event::Key(key) => {
                    // Use the keyboard module to handle input
                    if keyboard::handle_input(app, key).await? {
                        if app.exit_requested {
                            // Clear graphics before quitting
                            let _ = viuer_display::clear_graphics();
                            break;
                        }
                    } else {
                        break;
                    }
                }
                Event::Mouse(mouse) => {
                    handle_mouse_input(app, mouse, &layout)?;
                }
                _ => {}
            }
        }
    }
    
    Ok(())
}

/// Calculate layout based on terminal size and display mode
fn calculate_layout(width: u16, height: u16, mode: DisplayMode) -> Layout {
    match mode {
        DisplayMode::Debug => {
            // Full screen for debug console
            Layout {
                main: Rect::new(0, 2, width, height - 4),
                left: None,
                right: None,
                status: Rect::new(0, height - 2, width, 2),
            }
        }
        _ => {
            // Split layout for PdfText and PdfReader
            Layout {
                main: Rect::new(0, 2, width, height - 4),
                left: Some(Rect::new(0, 2, width/2, height - 4)),
                right: Some(Rect::new(width/2, 2, width/2, height - 4)),
                status: Rect::new(0, height - 2, width, 2),
            }
        }
    }
}

/// Draw a panel background with optional border
fn draw_panel_background(stdout: &mut io::Stdout, area: Rect, bg_color: Color) -> Result<()> {
    // Fill the area with background color
    for y in area.y..(area.y + area.height) {
        execute!(
            stdout,
            MoveTo(area.x, y),
            SetBackgroundColor(bg_color),
            Print(" ".repeat(area.width as usize)),
            ResetColor
        )?;
    }
    Ok(())
}



/// Draw a single header section
fn draw_header_section(
    stdout: &mut io::Stdout,
    title: &str,
    x: u16,
    y: u16,
    width: u16,
    bg_color: Color,
) -> Result<()> {
    // Draw header line (only one line now since title is separate)
    execute!(
        stdout,
        MoveTo(x, y),
        SetBackgroundColor(bg_color),
        SetForegroundColor(ChonkerTheme::text_header()),
        SetAttribute(Attribute::Bold),
        Print(format!("  {:<width$}", title, width = (width - 2) as usize)),
        ResetColor,
        SetAttribute(Attribute::Reset)
    )?;
    
    Ok(())
}

/// Render markdown content to the specified area
fn render_markdown(stdout: &mut io::Stdout, renderer: &Option<MarkdownRenderer>, area: Rect) -> Result<()> {
    if let Some(r) = renderer {
        r.render(area.x, area.y, area.width, area.height)?;
    } else {
        // Show placeholder message
        let msg = "Enable spatial recognition to see markdown";
        let msg_x = area.x + (area.width - msg.len() as u16) / 2;
        let msg_y = area.y + area.height / 2;
        execute!(
            stdout,
            MoveTo(msg_x, msg_y),
            SetForegroundColor(ChonkerTheme::text_dim()),
            Print(msg),
            ResetColor
        )?;
    }
    Ok(())
}

/// Draw headers with new mode names
fn draw_headers(stdout: &mut io::Stdout, layout: &Layout, mode: DisplayMode) -> Result<()> {
    // Get terminal width for centering
    let (term_width, _) = terminal::size()?;
    
    // No title display in main interface - version only shows in file picker
    
    // Draw panel headers on line 1
    match mode {
        DisplayMode::PdfText => {
            if let Some(left) = layout.left {
                draw_header_section(stdout, "PDF", left.x, 1, left.width, ChonkerTheme::accent_pdf())?;
            }
            if let Some(right) = layout.right {
                draw_header_section(stdout, "TEXT", right.x, 1, right.width, ChonkerTheme::accent_text())?;
            }
        }
        DisplayMode::PdfReader => {
            if let Some(left) = layout.left {
                draw_header_section(stdout, "PDF", left.x, 1, left.width, ChonkerTheme::accent_pdf())?;
            }
            if let Some(right) = layout.right {
                draw_header_section(stdout, "READER", right.x, 1, right.width, ChonkerTheme::accent_options())?;
            }
        }
        DisplayMode::Debug => {
            draw_header_section(stdout, "DEBUG CONSOLE", 0, 1, term_width, ChonkerTheme::accent_load_file())?;
        }
    }
    
    Ok(())
}

/// Draw status bar
fn draw_status_bar(stdout: &mut io::Stdout, app: &App, area: Rect) -> Result<()> {
    let status = format!(
        " {} | [Ctrl+O]pen [Ctrl+E]xtract [Tab]Mode [Ctrl+Q]uit ",
        app.status_message
    );
    
    // Normal status display (single line)
    let line_content = format!("{:<width$}", status, width = area.width as usize);
    
    execute!(
        stdout,
        MoveTo(area.x, area.y),
        SetBackgroundColor(ChonkerTheme::bg_status()),
        SetForegroundColor(ChonkerTheme::text_secondary()),
        Print(line_content),
        ResetColor
    )?;
    
    Ok(())
}

/// Draw debug console in its own tab
fn draw_debug_console(stdout: &mut io::Stdout, app: &App, area: Rect) -> Result<()> {
    if true {
        // Use entire area for debug console
        let debug_height = area.height; // Use full height
        
        // Draw header
        execute!(
            stdout,
            MoveTo(area.x, area.y),
            SetBackgroundColor(Color::DarkGrey),
            SetForegroundColor(Color::White),
            SetAttribute(Attribute::Bold),
            Print(format!(" 📋 DEBUG CONSOLE - [↑/↓] Scroll | [Cmd+A] Select All | [Cmd+C] Copy {:<width$}", 
                "", width = (area.width as usize).saturating_sub(70))),
            SetAttribute(Attribute::Reset)
        )?;
        
        // Show debug messages
        let start_idx = app.debug_scroll_offset;
        let visible_lines = (debug_height - 1) as usize;
        
        for i in 0..visible_lines {
            let y = area.y + i as u16 + 1;
            if y >= area.y + area.height {
                break;
            }
            
            execute!(stdout, MoveTo(area.x, y))?;
            
            let line_idx = start_idx + i;
            if line_idx < app.debug_console.len() {
                let line = &app.debug_console[line_idx];
                // Color code based on content
                let color = if line.contains("ERROR") || line.contains("❌") {
                    Color::Red
                } else if line.contains("WARNING") || line.contains("⚠️") {
                    Color::Yellow
                } else if line.contains("✅") || line.contains("SUCCESS") {
                    Color::Green
                } else if line.starts_with("===") {
                    Color::Cyan
                } else {
                    Color::Grey
                };
                
                execute!(
                    stdout,
                    SetBackgroundColor(Color::Black),
                    SetForegroundColor(color),
                    Print(format!("{:<width$}", 
                        if line.len() > area.width as usize { 
                            &line[..area.width as usize]
                        } else { 
                            line 
                        }, 
                        width = area.width as usize))
                )?;
            } else {
                execute!(
                    stdout,
                    SetBackgroundColor(Color::Black),
                    Print(" ".repeat(area.width as usize))
                )?;
            }
        }
        
        execute!(stdout, ResetColor)?;
    }
    
    Ok(())
}


/// Handle mouse input for TEXT and READER modes
fn handle_mouse_input(app: &mut App, mouse: MouseEvent, layout: &Layout) -> Result<()> {
    use crossterm::event::{MouseEventKind, MouseButton};
    
    // Handle DEBUG mode scrolling
    if app.display_mode == DisplayMode::Debug {
        match mouse.kind {
            MouseEventKind::ScrollDown => {
                if app.debug_scroll_offset + 20 < app.debug_console.len() {
                    app.debug_scroll_offset += 3;
                }
            }
            MouseEventKind::ScrollUp => {
                if app.debug_scroll_offset > 0 {
                    app.debug_scroll_offset = app.debug_scroll_offset.saturating_sub(3);
                }
            }
            MouseEventKind::Down(MouseButton::Left) => {
                // Click to select line in debug console
                if mouse.row >= 3 && mouse.row < app.term_height - 2 {
                    let clicked_line = app.debug_scroll_offset + (mouse.row - 3) as usize;
                    if clicked_line < app.debug_console.len() {
                        app.status_message = format!("Line {}: {}", clicked_line + 1, 
                            app.debug_console[clicked_line].chars().take(80).collect::<String>());
                    }
                }
            }
            _ => {}
        }
        return Ok(());
    }
    
    // Handle READER mode scrolling
    if app.display_mode == DisplayMode::PdfReader && app.markdown_data.is_some() {
        if let Some(right) = layout.right {
            let panel_x = right.x + 1;
            let panel_y = right.y;
            let panel_width = right.width - 2;
            let panel_height = right.height;
            
            // Check if mouse is within the READER panel
            if mouse.column >= panel_x && 
               mouse.column < panel_x + panel_width && 
               mouse.row >= panel_y && 
               mouse.row < panel_y + panel_height {
                
                match mouse.kind {
                    MouseEventKind::ScrollDown => {
                        if let Some(renderer) = &mut app.markdown_renderer {
                            renderer.scroll_down(3);
                        }
                    }
                    MouseEventKind::ScrollUp => {
                        if let Some(renderer) = &mut app.markdown_renderer {
                            renderer.scroll_up(3);
                        }
                    }
                    _ => {}
                }
            }
        }
        return Ok(());
    }
    
    // Handle TEXT mode - improved mouse tracking
    if app.display_mode != DisplayMode::PdfText || app.edit_data.is_none() {
        return Ok(());
    }
    
    // Get the TEXT panel bounds
    if let Some(right) = layout.right {
        let panel_x = right.x + 1; // Account for border
        let panel_y = right.y;
        let panel_width = right.width - 2;
        let panel_height = right.height;
        
        // Check if mouse is within the TEXT panel
        if mouse.column >= panel_x && 
           mouse.column < panel_x + panel_width && 
           mouse.row >= panel_y && 
           mouse.row < panel_y + panel_height {
            
            match mouse.kind {
                MouseEventKind::Down(MouseButton::Left) => {
                    // Start selection - improved cursor alignment
                    let grid_x = (mouse.column - panel_x) as usize;
                    let grid_y = (mouse.row - panel_y) as usize;
                    
                    if let Some(renderer) = &mut app.edit_display {
                        // Adjust for scroll offset
                        let (scroll_x, scroll_y) = renderer.get_scroll();
                        let actual_x = grid_x + scroll_x as usize;
                        let actual_y = grid_y + scroll_y as usize;
                        
                        // Ensure cursor is within bounds of actual data
                        if let Some(data) = &app.edit_data {
                            let bounded_y = actual_y.min(data.len().saturating_sub(1));
                            let bounded_x = if bounded_y < data.len() {
                                actual_x.min(data[bounded_y].len())
                            } else {
                                0
                            };
                            
                            app.cursor = (bounded_x, bounded_y);
                            app.selection_start = Some((bounded_x, bounded_y));
                            app.selection_end = None;
                            app.is_selecting = true;
                            
                            // Auto-scroll to make cursor visible
                            let (viewport_width, viewport_height) = renderer.get_viewport_size();
                            if bounded_y < scroll_y as usize {
                                renderer.scroll_to_y(bounded_y as u16);
                            } else if bounded_y >= (scroll_y + viewport_height) as usize {
                                renderer.scroll_to_y((bounded_y as u16).saturating_sub(viewport_height - 1));
                            }
                            if bounded_x < scroll_x as usize {
                                renderer.scroll_to_x(bounded_x as u16);
                            } else if bounded_x >= (scroll_x + viewport_width) as usize {
                                renderer.scroll_to_x((bounded_x as u16).saturating_sub(viewport_width - 1));
                            }
                        }
                    }
                }
                MouseEventKind::Drag(MouseButton::Left) => {
                    // Update selection end with improved tracking
                    if app.is_selecting {
                        let grid_x = (mouse.column.saturating_sub(panel_x)) as usize;
                        let grid_y = (mouse.row.saturating_sub(panel_y)) as usize;
                        
                        if let Some(renderer) = &mut app.edit_display {
                            // Adjust for scroll offset
                            let (scroll_x, scroll_y) = renderer.get_scroll();
                            let actual_x = grid_x + scroll_x as usize;
                            let actual_y = grid_y + scroll_y as usize;
                            
                            // Ensure selection end is within bounds
                            if let Some(data) = &app.edit_data {
                                let bounded_y = actual_y.min(data.len().saturating_sub(1));
                                let bounded_x = if bounded_y < data.len() {
                                    actual_x.min(data[bounded_y].len())
                                } else {
                                    0
                                };
                                
                                app.selection_end = Some((bounded_x, bounded_y));
                                app.cursor = (bounded_x, bounded_y);
                                
                                // Auto-scroll during drag if near edges
                                if grid_y == 0 && scroll_y > 0 {
                                    renderer.scroll_up(1);
                                } else if grid_y >= panel_height as usize - 1 {
                                    renderer.scroll_down(1);
                                }
                                if grid_x == 0 && scroll_x > 0 {
                                    renderer.scroll_left(1);
                                } else if grid_x >= panel_width as usize - 1 {
                                    renderer.scroll_right(1);
                                }
                            }
                        }
                    }
                }
                MouseEventKind::Up(MouseButton::Left) => {
                    // End selection
                    if app.is_selecting {
                        let grid_x = (mouse.column.saturating_sub(panel_x)) as usize;
                        let grid_y = (mouse.row.saturating_sub(panel_y)) as usize;
                        
                        if let Some(renderer) = &app.edit_display {
                            // Adjust for scroll offset
                            let (scroll_x, scroll_y) = renderer.get_scroll();
                            let actual_x = grid_x + scroll_x as usize;
                            let actual_y = grid_y + scroll_y as usize;
                            
                            // Ensure selection end is within bounds
                            if let Some(data) = &app.edit_data {
                                let bounded_y = actual_y.min(data.len().saturating_sub(1));
                                let bounded_x = if bounded_y < data.len() {
                                    actual_x.min(data[bounded_y].len())
                                } else {
                                    0
                                };
                                
                                app.selection_end = Some((bounded_x, bounded_y));
                                app.cursor = (bounded_x, bounded_y);
                                app.is_selecting = false;
                                
                                // If selection start and end are the same, clear selection
                                if app.selection_start == app.selection_end {
                                    app.selection_start = None;
                                    app.selection_end = None;
                                }
                            }
                        }
                    }
                }
                MouseEventKind::ScrollDown => {
                    // Scroll down with mouse wheel/trackpad
                    if let Some(renderer) = &mut app.edit_display {
                        renderer.scroll_down(3); // Scroll 3 lines at a time
                    }
                }
                MouseEventKind::ScrollUp => {
                    // Scroll up with mouse wheel/trackpad
                    if let Some(renderer) = &mut app.edit_display {
                        renderer.scroll_up(3); // Scroll 3 lines at a time
                    }
                }
                MouseEventKind::ScrollLeft => {
                    // Horizontal scroll left
                    if let Some(renderer) = &mut app.edit_display {
                        renderer.scroll_left(3);
                    }
                }
                MouseEventKind::ScrollRight => {
                    // Horizontal scroll right
                    if let Some(renderer) = &mut app.edit_display {
                        renderer.scroll_right(3);
                    }
                }
                _ => {}
            }
        }
    }
    
    Ok(())
}

// Keyboard handling moved to keyboard.rs module
