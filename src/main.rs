// Pure crossterm implementation - no ratatui, no tearing!
use anyhow::Result;
use crossterm::{
    cursor::{Hide, MoveTo, Show},
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
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
};
use image::DynamicImage;
use clap::Parser;

// Existing modules
mod text_matrix;
mod ferrules_extractor;
mod renderer;
mod pdf_renderer;
mod pdf_to_grid;
mod file_picker;
mod theme;
mod terminal_image;

use renderer::TextRenderer;
use theme::ChonkerTheme;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// PDF file to view (opens file dialog if not provided)
    pdf_file: Option<PathBuf>,
    
    /// Starting page number (1-indexed)
    #[arg(short, long, default_value_t = 1)]
    page: usize,
    
    /// Display mode: image, text, or split
    #[arg(short, long, default_value = "split")]
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
enum DisplayMode {
    Image,
    TextMatrix,
    Split,
    Options,
}

#[derive(Clone, Debug)]
pub struct AppSettings {
    use_sophisticated_extraction: bool,
    use_vision_model: bool,
}

pub struct App {
    pdf_path: PathBuf,
    current_page: usize,
    total_pages: usize,
    display_mode: DisplayMode,
    text_matrix: Option<Vec<Vec<char>>>,
    text_renderer: Option<TextRenderer>,
    current_page_image: Option<DynamicImage>,
    settings: AppSettings,
    should_quit: bool,
    status_message: String,
    term_width: u16,
    term_height: u16,
    image_protocol: terminal_image::ImageProtocol,
    options_cursor: usize, // Track which option is selected
    dark_mode: bool, // Dark mode toggle
}

impl App {
    pub fn new(pdf_path: PathBuf, starting_page: usize, mode: &str) -> Result<Self> {
        let (width, height) = terminal::size()?;
        let total_pages = ferrules_extractor::get_page_count(&pdf_path)?;
        
        let display_mode = match mode {
            "image" => DisplayMode::Image,
            "text" => DisplayMode::TextMatrix,
            _ => DisplayMode::Split,
        };
        
        let protocol = terminal_image::detect_image_support();
        
        Ok(Self {
            pdf_path,
            current_page: starting_page.saturating_sub(1),
            total_pages,
            display_mode,
            text_matrix: None,
            text_renderer: None,
            current_page_image: None,
            settings: AppSettings {
                use_sophisticated_extraction: false,
                use_vision_model: true,
            },
            should_quit: false,
            status_message: format!("Page {}/{}", starting_page, total_pages),
            term_width: width,
            term_height: height,
            image_protocol: protocol,
            options_cursor: 0,
            dark_mode: true, // Default to dark mode
        })
    }
    
    fn update_terminal_size(&mut self) -> Result<()> {
        let (width, height) = terminal::size()?;
        self.term_width = width;
        self.term_height = height;
        
        // Update renderer size if it exists
        if let Some(renderer) = &mut self.text_renderer {
            let renderer_width = if self.display_mode == DisplayMode::Split {
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
        
        // Calculate PDF size based on display mode - much larger for better readability
        let (image_width, image_height) = match self.display_mode {
            DisplayMode::Split => {
                // For split mode, use almost full half screen
                let width = ((self.term_width / 2) - 4).max(40) as u32;
                let height = (self.term_height - 4).max(20) as u32;
                // Scale up for better quality (will be downscaled by terminal)
                (width * 10, height * 20)
            }
            DisplayMode::Image => {
                // For full PDF mode, use most of the screen
                let width = (self.term_width - 8).max(40) as u32;
                let height = (self.term_height - 4).max(20) as u32;
                // Scale up for better quality
                (width * 10, height * 20)
            }
            _ => {
                // Default for other modes
                let width = ((self.term_width / 2) - 4).max(40) as u32;
                let height = (self.term_height - 4).max(20) as u32;
                (width * 10, height * 20)
            }
        };
        
        match pdf_renderer::render_pdf_page(&self.pdf_path, self.current_page, image_width, image_height) {
            Ok(image) => {
                self.current_page_image = Some(image);
                self.status_message = format!("Page {}/{} - Press Ctrl+E to extract text", self.current_page + 1, self.total_pages);
            }
            Err(e) => {
                eprintln!("Failed to render PDF page: {}", e);
                self.current_page_image = None;
                self.status_message = format!("Failed to load page {}", self.current_page + 1);
            }
        }
        
        Ok(())
    }
    
    pub async fn extract_current_page(&mut self) -> Result<()> {
        self.status_message = "Extracting text...".to_string();
        
        // Calculate dimensions
        let matrix_width = if self.display_mode == DisplayMode::Split {
            ((self.term_width / 2) - 2).min(100) as usize
        } else {
            (self.term_width - 4).min(200) as usize
        };
        let matrix_height = (self.term_height - 6).min(100) as usize;
        
        // Extract text
        self.status_message = if self.settings.use_vision_model {
            "Extracting text with AI...".to_string()
        } else {
            "Extracting text...".to_string()
        };
        
        let matrix = if self.settings.use_sophisticated_extraction {
            ferrules_extractor::extract_to_matrix_sophisticated(
                &self.pdf_path,
                self.current_page,
                matrix_width,
                matrix_height,
                self.settings.use_vision_model,
            ).await?
        } else {
            ferrules_extractor::extract_to_matrix(
                &self.pdf_path,
                self.current_page,
                matrix_width,
                matrix_height,
            ).await?
        };
        
        // Create or update renderer
        let renderer_width = if self.display_mode == DisplayMode::Split {
            self.term_width / 2 - 2
        } else {
            self.term_width - 2
        };
        
        if self.text_renderer.is_none() {
            let mut renderer = TextRenderer::new(renderer_width, matrix_height as u16);
            renderer.update_buffer(&matrix);
            self.text_renderer = Some(renderer);
        } else {
            if let Some(renderer) = &mut self.text_renderer {
                renderer.resize(renderer_width, matrix_height as u16);
                renderer.update_buffer(&matrix);
            }
        }
        
        self.text_matrix = Some(matrix);
        self.status_message = format!("Page {}/{} - Text extracted", self.current_page + 1, self.total_pages);
        Ok(())
    }
    
    pub fn next_page(&mut self) {
        if self.current_page < self.total_pages - 1 {
            let _ = terminal_image::clear_graphics();
            self.current_page += 1;
            self.text_matrix = None;
            self.current_page_image = None;
            self.text_renderer = None; // Clear text renderer too
            self.status_message = format!("Page {}/{} - Press Ctrl+E to extract text", self.current_page + 1, self.total_pages);
        }
    }
    
    pub fn prev_page(&mut self) {
        if self.current_page > 0 {
            let _ = terminal_image::clear_graphics();
            self.current_page -= 1;
            self.text_matrix = None;
            self.current_page_image = None;
            self.text_renderer = None; // Clear text renderer too
            self.status_message = format!("Page {}/{} - Press Ctrl+E to extract text", self.current_page + 1, self.total_pages);
        }
    }
    
    pub fn toggle_mode(&mut self) {
        // DON'T clear graphics - let the new mode render over the old one
        // This prevents the flicker entirely
        
        self.display_mode = match self.display_mode {
            DisplayMode::Image => DisplayMode::TextMatrix,
            DisplayMode::TextMatrix => DisplayMode::Split,
            DisplayMode::Split => DisplayMode::Options,
            DisplayMode::Options => DisplayMode::Image,
        };
        
        // Keep the current_page_image loaded - no clearing, no reloading
        // The rendering loop will display it appropriately for the new mode
        
        // Update renderer size for the new mode (if it exists)
        if let Some(renderer) = &mut self.text_renderer {
            let renderer_width = if self.display_mode == DisplayMode::Split {
                self.term_width / 2 - 2
            } else {
                self.term_width - 2
            };
            let renderer_height = (self.term_height - 5).min(100);
            renderer.resize(renderer_width, renderer_height);
        }
        
        self.status_message = format!("Mode: {:?}", self.display_mode);
    }
}

/// Layout structure for organizing screen regions
struct Layout {
    main: Rect,
    left: Option<Rect>,
    right: Option<Rect>,
    header: Rect,
    status: Rect,
}

/// Main entry point
fn main() -> Result<()> {
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
    
    // Create tokio runtime
    let runtime = tokio::runtime::Runtime::new()?;
    
    // Initial load - just the PDF, not text extraction
    runtime.block_on(app.load_pdf_page())?;
    
    // Run the app
    let result = run_app(&mut app, &runtime);
    
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
        Clear(ClearType::All)
    )?;
    Ok(())
}

/// Restore terminal to normal mode
fn restore_terminal() -> Result<()> {
    // Clear any remaining graphics
    let _ = terminal_image::clear_graphics();
    
    // Clear the screen before leaving
    execute!(
        io::stdout(),
        Clear(ClearType::All),
        MoveTo(0, 0),
        ResetColor,
        Show,
        LeaveAlternateScreen,
        Clear(ClearType::All)  // Extra clear after leaving alternate screen
    )?;
    disable_raw_mode()?;
    
    // Final clear to ensure clean exit
    println!("\x1b[2J\x1b[H");
    
    Ok(())
}

/// Main application loop with SYNCHRONIZED RENDERING
fn run_app(app: &mut App, runtime: &tokio::runtime::Runtime) -> Result<()> {
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
        draw_status_bar(&mut stdout, app, layout.status)?;
        
        // Draw main content based on mode
        match app.display_mode {
            DisplayMode::Split => {
                // Draw panels cleanly without borders
                if let Some(left) = layout.left {
                    // PDF panel - clean background
                    let bg = if app.dark_mode { ChonkerTheme::bg_secondary() } else { ChonkerTheme::bg_secondary_light() };
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
                
                // Draw text panel
                if let Some(right) = layout.right {
                    if let Some(renderer) = &app.text_renderer {
                        renderer.render(
                            right.x + 1,
                            right.y,
                            right.width - 2,
                            right.height,
                        )?;
                    } else {
                        // Centered message
                        let msg = "Press Ctrl+E to extract";
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
                }
            }
            DisplayMode::TextMatrix => {
                // Clean background
                let bg = if app.dark_mode { ChonkerTheme::bg_secondary() } else { ChonkerTheme::bg_secondary_light() };
                draw_panel_background(&mut stdout, layout.main, bg)?;
                
                // Full screen text matrix - centered
                if let Some(renderer) = &app.text_renderer {
                    // Center the text content
                    let padding = 4;
                    renderer.render(
                        layout.main.x + padding,
                        layout.main.y + 1,
                        layout.main.width - (padding * 2),
                        layout.main.height - 2,
                    )?;
                } else {
                    // Simple centered message
                    let msg = "Press Ctrl+E to extract text";
                    let msg_x = layout.main.x + (layout.main.width - msg.len() as u16) / 2;
                    let msg_y = layout.main.y + layout.main.height / 2;
                    execute!(
                        stdout,
                        MoveTo(msg_x, msg_y),
                        SetForegroundColor(ChonkerTheme::text_dim()),
                        Print(msg),
                        ResetColor
                    )?;
                }
            }
            DisplayMode::Image => {
                // Clean background
                let bg = if app.dark_mode { ChonkerTheme::bg_secondary() } else { ChonkerTheme::bg_secondary_light() };
                draw_panel_background(&mut stdout, layout.main, bg)?;
                
                // Show centered loading message if no image
                if app.current_page_image.is_none() {
                    let msg = "Loading PDF page...";
                    let msg_x = layout.main.x + (layout.main.width - msg.len() as u16) / 2;
                    let msg_y = layout.main.y + layout.main.height / 2;
                    execute!(
                        stdout,
                        MoveTo(msg_x, msg_y),
                        SetForegroundColor(ChonkerTheme::accent_pdf()),
                        Print(msg),
                        ResetColor
                    )?;
                }
                // Image will be displayed after sync block
            }
            DisplayMode::Options => {
                draw_options_panel(&mut stdout, app, layout.main)?;
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
        
        match app.display_mode {
            DisplayMode::Split => {
                if let Some(left) = layout.left {
                    if let Some(ref image) = app.current_page_image {
                        // Use full left panel for PDF
                        let _ = terminal_image::display_image(
                            image,
                            left.x,
                            left.y,
                            left.width,
                            left.height,
                        );
                    }
                }
            }
            DisplayMode::Image => {
                if let Some(ref image) = app.current_page_image {
                    // Center the image with some padding
                    let padding = 2;
                    let _ = terminal_image::display_image(
                        image,
                        layout.main.x + padding,
                        layout.main.y,
                        layout.main.width - (padding * 2),
                        layout.main.height,
                    );
                }
            }
            _ => {}
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
            if let Event::Key(key) = event::read()? {
                if handle_input(app, key, runtime)? {
                    if app.should_quit {
                        // Clear graphics before quitting
                        let _ = terminal_image::clear_graphics();
                        break;
                    }
                } else {
                    break;
                }
            }
        }
    }
    
    Ok(())
}

/// Calculate layout based on terminal size and display mode
fn calculate_layout(width: u16, height: u16, mode: DisplayMode) -> Layout {
    let header_height = 2;
    let status_height = 2;
    let content_height = height.saturating_sub(header_height + status_height);
    
    match mode {
        DisplayMode::Split => {
            let half_width = width / 2;
            Layout {
                main: Rect::new(0, header_height, width, content_height),
                left: Some(Rect::new(0, header_height, half_width, content_height)),
                right: Some(Rect::new(half_width, header_height, half_width, content_height)),
                header: Rect::new(0, 0, width, header_height),
                status: Rect::new(0, height - status_height, width, status_height),
            }
        }
        _ => {
            Layout {
                main: Rect::new(0, header_height, width, content_height),
                left: None,
                right: None,
                header: Rect::new(0, 0, width, header_height),
                status: Rect::new(0, height - status_height, width, status_height),
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


/// Draw headers with Ghostty-inspired theme
fn draw_headers(stdout: &mut io::Stdout, layout: &Layout, mode: DisplayMode) -> Result<()> {
    match mode {
        DisplayMode::Split => {
            // Left header
            if let Some(left) = layout.left {
                draw_header_section(
                    stdout,
                    "PDF VIEW",
                    left.x,
                    0,
                    left.width,
                    ChonkerTheme::accent_pdf(),
                )?;
            }
            // Right header  
            if let Some(right) = layout.right {
                draw_header_section(
                    stdout,
                    "FERRULES AI TEXT",
                    right.x,
                    0,
                    right.width,
                    ChonkerTheme::accent_text(),
                )?;
            }
        }
        DisplayMode::TextMatrix => {
            draw_header_section(
                stdout,
                "TEXT MATRIX [hjkl to scroll]",
                0,
                0,
                layout.main.width,
                ChonkerTheme::accent_text(),
            )?;
        }
        DisplayMode::Image => {
            draw_header_section(
                stdout,
                "PDF IMAGE VIEW",
                0,
                0,
                layout.main.width,
                ChonkerTheme::accent_pdf(),
            )?;
        }
        DisplayMode::Options => {
            draw_header_section(
                stdout,
                "OPTIONS",
                0,
                0,
                layout.main.width,
                ChonkerTheme::accent_options(),
            )?;
        }
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
    // Draw first header line
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
    
    // Draw second header line to ensure full header coverage
    execute!(
        stdout,
        MoveTo(x, y + 1),
        SetBackgroundColor(bg_color),
        Print(" ".repeat(width as usize)),
        ResetColor
    )?;
    
    Ok(())
}

/// Draw status bar
fn draw_status_bar(stdout: &mut io::Stdout, app: &App, area: Rect) -> Result<()> {
    let status = format!(
        " {} | Mode: {:?} | [o]pen [n]ext [p]rev [m]ode [q]uit ",
        app.status_message,
        app.display_mode
    );
    
    // Draw both lines of status bar to ensure full coverage
    for y_offset in 0..area.height {
        let line_content = if y_offset == 0 {
            format!("{:<width$}", status, width = area.width as usize)
        } else {
            " ".repeat(area.width as usize)
        };
        
        execute!(
            stdout,
            MoveTo(area.x, area.y + y_offset),
            SetBackgroundColor(ChonkerTheme::bg_status()),
            SetForegroundColor(ChonkerTheme::text_secondary()),
            Print(line_content),
            ResetColor
        )?;
    }
    
    Ok(())
}

/// Draw options panel with cursor
fn draw_options_panel(stdout: &mut io::Stdout, app: &App, area: Rect) -> Result<()> {
    // Clean background
    let bg = if app.dark_mode { ChonkerTheme::bg_secondary() } else { ChonkerTheme::bg_secondary_light() };
    draw_panel_background(stdout, area, bg)?;
    
    // Center content horizontally
    let content_width = 50.min(area.width - 4); // Max width of 50 chars
    let start_x = area.x + (area.width - content_width) / 2;
    
    // Define the selectable options
    let toggleable_options = vec![
        (0, "Sophisticated Spatial Extraction", app.settings.use_sophisticated_extraction),
        (1, "AI Vision Analysis", app.settings.use_vision_model),
    ];
    
    // Start content vertically centered
    let total_lines = 20; // Approximate total lines of content
    let start_y = area.y + (area.height - total_lines) / 2;
    
    // Title
    let title = "SETTINGS";
    let title_x = area.x + (area.width - title.len() as u16) / 2;
    execute!(
        stdout,
        MoveTo(title_x, start_y),
        SetForegroundColor(ChonkerTheme::accent_options()),
        SetAttribute(Attribute::Bold),
        Print(title),
        SetAttribute(Attribute::Reset),
        ResetColor
    )?;
    
    let mut y_offset = 3;
    
    // TEXT EXTRACTION section
    execute!(
        stdout,
        MoveTo(start_x, start_y + y_offset),
        SetForegroundColor(ChonkerTheme::text_secondary()),
        Print("TEXT EXTRACTION"),
        ResetColor
    )?;
    y_offset += 2;
    
    // First toggleable option with cursor
    let (cursor_char, highlight_color) = if app.options_cursor == 0 {
        (">", ChonkerTheme::accent_text())
    } else {
        (" ", ChonkerTheme::text_primary())
    };
    
    let status = if toggleable_options[0].2 { "ON" } else { "OFF" };
    let status_color = if toggleable_options[0].2 { 
        ChonkerTheme::success() 
    } else { 
        ChonkerTheme::error() 
    };
    
    execute!(
        stdout,
        MoveTo(start_x, start_y + y_offset),
        SetForegroundColor(highlight_color),
        Print(format!("{} Sophisticated Extraction", cursor_char)),
        ResetColor,
        MoveTo(start_x + 35, start_y + y_offset),
        SetForegroundColor(status_color),
        Print(status),
        ResetColor
    )?;
    y_offset += 2;
    
    // VISION MODEL section
    execute!(
        stdout,
        MoveTo(start_x, start_y + y_offset),
        SetForegroundColor(ChonkerTheme::text_secondary()),
        Print("VISION MODEL"),
        ResetColor
    )?;
    y_offset += 2;
    
    // Second toggleable option with cursor
    let (cursor_char, highlight_color) = if app.options_cursor == 1 {
        (">", ChonkerTheme::accent_text())
    } else {
        (" ", ChonkerTheme::text_primary())
    };
    
    let status = if toggleable_options[1].2 { "ON" } else { "OFF" };
    let status_color = if toggleable_options[1].2 { 
        ChonkerTheme::success() 
    } else { 
        ChonkerTheme::error() 
    };
    
    execute!(
        stdout,
        MoveTo(start_x, start_y + y_offset),
        SetForegroundColor(highlight_color),
        Print(format!("{} AI Vision Analysis", cursor_char)),
        ResetColor,
        MoveTo(start_x + 35, start_y + y_offset),
        SetForegroundColor(status_color),
        Print(status),
        ResetColor
    )?;
    y_offset += 4;
    
    // Footer with controls - centered at bottom
    let controls = "↑/↓ Navigate   Space Toggle   M Return";
    let controls_x = area.x + (area.width - controls.len() as u16) / 2;
    execute!(
        stdout,
        MoveTo(controls_x, start_y + y_offset + 2),
        SetForegroundColor(ChonkerTheme::text_dim()),
        Print(controls),
        ResetColor
    )?;
    
    Ok(())
}

/// Handle keyboard input
fn handle_input(app: &mut App, key: KeyEvent, runtime: &tokio::runtime::Runtime) -> Result<bool> {
    match key.code {
        KeyCode::Char('q') | KeyCode::Char('Q') => {
            app.should_quit = true;
            return Ok(true);
        }
        
        KeyCode::Char('n') | KeyCode::Right => {
            app.next_page();
            runtime.block_on(app.load_pdf_page())?;
        }
        
        KeyCode::Char('p') | KeyCode::Left => {
            app.prev_page();
            runtime.block_on(app.load_pdf_page())?;
        }
        
        KeyCode::Char('m') => {
            app.toggle_mode();
            // DON'T reload PDF - this was causing the flicker!
            // The existing image will be displayed in the new mode automatically
        }
        
        KeyCode::Char('d') | KeyCode::Char('D') => {
            app.dark_mode = !app.dark_mode;
            app.status_message = format!("Mode: {}", if app.dark_mode { "Dark" } else { "Light" });
        }
        
        KeyCode::Char('o') | KeyCode::Char('O') => {
            // Open new file
            disable_raw_mode()?;
            println!("\r\n🐹 Opening file picker...\r");
            
            let new_file = file_picker::pick_pdf_file()?;
            
            enable_raw_mode()?;
            
            if let Some(new_file) = new_file {
                if let Ok(new_app) = App::new(new_file.clone(), 1, "split") {
                    *app = new_app;
                    app.status_message = format!("Loaded: {}", new_file.display());
                    runtime.block_on(app.load_pdf_page())?;
                }
            }
        }
        
        KeyCode::Char('e') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            runtime.block_on(app.extract_current_page())?;
        }
        
        // Options mode controls with cursor navigation
        KeyCode::Up if app.display_mode == DisplayMode::Options => {
            if app.options_cursor > 0 {
                app.options_cursor -= 1;
            }
        }
        
        KeyCode::Down if app.display_mode == DisplayMode::Options => {
            if app.options_cursor < 1 { // We have 2 options (0 and 1)
                app.options_cursor += 1;
            }
        }
        
        KeyCode::Char(' ') | KeyCode::Enter if app.display_mode == DisplayMode::Options => {
            match app.options_cursor {
                0 => {
                    app.settings.use_sophisticated_extraction = !app.settings.use_sophisticated_extraction;
                    app.status_message = format!("Sophisticated extraction: {}",
                        if app.settings.use_sophisticated_extraction { "ON" } else { "OFF" });
                    runtime.block_on(app.extract_current_page())?;
                }
                1 => {
                    app.settings.use_vision_model = !app.settings.use_vision_model;
                    app.status_message = format!("Vision model: {}",
                        if app.settings.use_vision_model { "ON" } else { "OFF" });
                    runtime.block_on(app.extract_current_page())?;
                }
                _ => {}
            }
        }
        
        // Keep the old shortcuts for backward compatibility
        KeyCode::Char('s') | KeyCode::Char('S') if app.display_mode == DisplayMode::Options => {
            app.settings.use_sophisticated_extraction = !app.settings.use_sophisticated_extraction;
            app.status_message = format!("Sophisticated extraction: {}",
                if app.settings.use_sophisticated_extraction { "ON" } else { "OFF" });
            runtime.block_on(app.extract_current_page())?;
        }
        
        KeyCode::Char('v') | KeyCode::Char('V') if app.display_mode == DisplayMode::Options => {
            app.settings.use_vision_model = !app.settings.use_vision_model;
            app.status_message = format!("Vision model: {}",
                if app.settings.use_vision_model { "ON" } else { "OFF" });
            runtime.block_on(app.extract_current_page())?;
        }
        
        // Scrolling for text matrix (not in Options mode)
        KeyCode::Char('j') => {
            if let Some(renderer) = &mut app.text_renderer {
                renderer.scroll_down(1);
            }
        }
        KeyCode::Down if app.display_mode != DisplayMode::Options => {
            if let Some(renderer) = &mut app.text_renderer {
                renderer.scroll_down(1);
            }
        }
        KeyCode::Char('k') => {
            if let Some(renderer) = &mut app.text_renderer {
                renderer.scroll_up(1);
            }
        }
        KeyCode::Up if app.display_mode != DisplayMode::Options => {
            if let Some(renderer) = &mut app.text_renderer {
                renderer.scroll_up(1);
            }
        }
        KeyCode::Char('h') => {
            if let Some(renderer) = &mut app.text_renderer {
                renderer.scroll_left(1);
            }
        }
        KeyCode::Char('l') => {
            if let Some(renderer) = &mut app.text_renderer {
                renderer.scroll_right(1);
            }
        }
        KeyCode::PageDown => {
            if let Some(renderer) = &mut app.text_renderer {
                renderer.scroll_down(10);
            }
        }
        KeyCode::PageUp => {
            if let Some(renderer) = &mut app.text_renderer {
                renderer.scroll_up(10);
            }
        }
        
        _ => {}
    }
    
    Ok(true)
}