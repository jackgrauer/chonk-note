// Pure crossterm implementation - no ratatui, no tearing!
use anyhow::Result;
use crossterm::{
    cursor::{Hide, MoveTo, Show},
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers, MouseEvent, EnableMouseCapture, DisableMouseCapture},
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

// Cross-platform modifier key constant for EDIT mode
#[cfg(target_os = "macos")]
const MOD_KEY: KeyModifiers = KeyModifiers::SUPER; // Cmd key on macOS
#[cfg(not(target_os = "macos"))]
const MOD_KEY: KeyModifiers = KeyModifiers::CONTROL; // Ctrl key elsewhere

// Existing modules
mod content_extractor;
mod renderer;
mod pdf_renderer;
mod pdf_to_grid;
mod file_picker;
mod theme;
mod viuer_display;
mod markdown_renderer;
mod keyboard;

use renderer::EditPanelRenderer;
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
    
    /// Display mode: edit, markdown, or options
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
enum DisplayMode {
    PdfEdit,
    PdfMarkdown,
    Options,
}

#[derive(Clone, Debug)]
pub struct AppSettings {
    pub spatial_recognition_enabled: bool,
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
    pub term_width: u16,
    pub term_height: u16,
    pub dark_mode: bool, // Dark mode toggle
    // EDIT mode cursor and selection
    pub cursor: (usize, usize),  // (x, y) in grid
    pub selection_start: Option<(usize, usize)>,
    pub selection_end: Option<(usize, usize)>,
    pub is_selecting: bool,
}

impl App {
    pub fn new(pdf_path: PathBuf, starting_page: usize, mode: &str) -> Result<Self> {
        let (width, height) = terminal::size()?;
        let total_pages = content_extractor::get_page_count(&pdf_path)?;
        
        let display_mode = match mode {
            "edit" => DisplayMode::PdfEdit,
            "markdown" => DisplayMode::PdfMarkdown,
            "options" => DisplayMode::Options,
            _ => DisplayMode::PdfEdit,
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
                spatial_recognition_enabled: true,
            },
            exit_requested: false,
            status_message: format!("Page {}/{}", starting_page, total_pages),
            term_width: width,
            term_height: height,
            dark_mode: true, // Default to dark mode
            cursor: (0, 0),
            selection_start: None,
            selection_end: None,
            is_selecting: false,
        })
    }
    
    fn update_terminal_size(&mut self) -> Result<()> {
        let (width, height) = terminal::size()?;
        self.term_width = width;
        self.term_height = height;
        
        // Update renderer size if it exists
        if let Some(renderer) = &mut self.edit_display {
            let renderer_width = if matches!(self.display_mode, DisplayMode::PdfEdit | DisplayMode::PdfMarkdown) {
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
            DisplayMode::PdfEdit | DisplayMode::PdfMarkdown => {
                // For split modes - use actual panel size
                let width = ((self.term_width / 2) - 4).max(40) as u32;
                let height = (self.term_height - 4).max(20) as u32;
                // Ultra-high resolution with aspect ratio correction
                // Terminal cells are ~2:1 (height:width), so we scale height by 1.8x
                (width * 14, (height * 14 * 18) / 10)  // 1.8x height for better aspect ratio
            }
            _ => {
                // Default for other modes
                let width = ((self.term_width / 2) - 4).max(40) as u32;
                let height = (self.term_height - 4).max(20) as u32;
                // Same aspect ratio correction
                (width * 14, (height * 14 * 18) / 10)  // 1.8x height for better aspect ratio
            }
        };
        
        match pdf_renderer::render_pdf_page(&self.pdf_path, self.current_page, image_width, image_height) {
            Ok(image) => {
                self.current_page_image = Some(image);
                self.status_message = format!("Page {}/{} - Ctrl+E: Extract text", self.current_page + 1, self.total_pages);
                
                #[cfg(debug_assertions)]
                {
                    let protocol = viuer_display::get_protocol_info();
                    self.status_message = format!("Page {}/{} [{}]", self.current_page + 1, self.total_pages, protocol);
                }
            }
            Err(e) => {
                // Log error for debugging but show user-friendly message
                #[cfg(debug_assertions)]
                eprintln!("Failed to render PDF page: {}", e);
                
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
        
        // Calculate dimensions
        let matrix_width = if matches!(self.display_mode, DisplayMode::PdfEdit | DisplayMode::PdfMarkdown) {
            ((self.term_width / 2) - 2).min(100) as usize
        } else {
            (self.term_width - 4).min(200) as usize
        };
        let matrix_height = (self.term_height - 6).min(100) as usize;
        
        // Extract text
        self.status_message = if self.settings.spatial_recognition_enabled {
            "Extracting content with spatial recognition...".to_string()
        } else {
            "Extracting content...".to_string()
        };
        
        let matrix = if self.settings.spatial_recognition_enabled {
            content_extractor::extract_to_matrix_sophisticated(
                &self.pdf_path,
                self.current_page,
                matrix_width,
                matrix_height,
                true,
            ).await?
        } else {
            content_extractor::extract_to_matrix(
                &self.pdf_path,
                self.current_page,
                matrix_width,
                matrix_height,
            ).await?
        };
        
        // Create or update renderer
        let renderer_width = if matches!(self.display_mode, DisplayMode::PdfEdit | DisplayMode::PdfMarkdown) {
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
        
        // Extract markdown if spatial recognition is enabled
        if self.settings.spatial_recognition_enabled {
            let markdown = content_extractor::get_markdown_content(&self.pdf_path, self.current_page).await?;
            
            // Create or update markdown renderer
            if self.markdown_renderer.is_none() {
                self.markdown_renderer = Some(MarkdownRenderer::new());
            }
            
            if let Some(renderer) = &mut self.markdown_renderer {
                renderer.set_content(&markdown);
            }
            
            self.markdown_data = Some(markdown);
        }
        
        self.status_message = format!("Page {}/{} - Content extracted to EDIT panel", self.current_page + 1, self.total_pages);
        Ok(())
    }
    
    pub fn next_page(&mut self) {
        if self.current_page < self.total_pages - 1 {
            let _ = viuer_display::clear_graphics();
            self.current_page += 1;
            self.edit_data = None;
            self.current_page_image = None;
            self.edit_display = None; // Clear EDIT renderer
            self.markdown_renderer = None; // Clear MARKDOWN renderer
            self.markdown_data = None;
            self.status_message = format!("Page {}/{} - Press Ctrl+E to extract to EDIT panel", self.current_page + 1, self.total_pages);
        }
    }
    
    pub fn prev_page(&mut self) {
        if self.current_page > 0 {
            let _ = viuer_display::clear_graphics();
            self.current_page -= 1;
            self.edit_data = None;
            self.current_page_image = None;
            self.edit_display = None; // Clear EDIT renderer
            self.markdown_renderer = None; // Clear MARKDOWN renderer
            self.markdown_data = None;
            self.status_message = format!("Page {}/{} - Press Ctrl+E to extract to EDIT panel", self.current_page + 1, self.total_pages);
        }
    }
    
    pub fn toggle_mode(&mut self) {
        self.display_mode = match self.display_mode {
            DisplayMode::PdfEdit => {
                if self.settings.spatial_recognition_enabled && self.markdown_data.is_some() {
                    DisplayMode::PdfMarkdown
                } else {
                    DisplayMode::Options
                }
            }
            DisplayMode::PdfMarkdown => DisplayMode::Options,
            DisplayMode::Options => DisplayMode::PdfEdit,
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
            DisplayMode::PdfEdit | DisplayMode::PdfMarkdown => {
                // PDF panel - where the pdf image displays
                if let Some(left) = layout.left {
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
                
                // Right panel - EDIT or MARKDOWN
                if let Some(right) = layout.right {
                    if app.display_mode == DisplayMode::PdfEdit {
                        // Show EDIT panel - unformatted gridlike layout
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
                            let msg = "Press Ctrl+E to extract to EDIT";
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
                        // Show MARKDOWN panel - nicely formatted, non-editable
                        render_markdown(&mut stdout, &app.markdown_renderer, right)?;
                    }
                }
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
            DisplayMode::PdfEdit | DisplayMode::PdfMarkdown => {
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
                        );
                    }
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
            match event::read()? {
                Event::Key(key) => {
                    // Use the keyboard module to handle input
                    if keyboard::handle_input(app, key, runtime)? {
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
        DisplayMode::Options => {
            Layout {
                main: Rect::new(0, 2, width, height - 4),
                left: None,
                right: None,
                status: Rect::new(0, height - 2, width, 2),
            }
        }
        _ => {
            // Always return split layout for PdfEdit and PdfMarkdown
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
    match mode {
        DisplayMode::PdfEdit => {
            if let Some(left) = layout.left {
                draw_header_section(stdout, "PDF", left.x, 0, left.width, ChonkerTheme::accent_pdf())?;
            }
            if let Some(right) = layout.right {
                draw_header_section(stdout, "EDIT", right.x, 0, right.width, ChonkerTheme::accent_text())?;
            }
        }
        DisplayMode::PdfMarkdown => {
            if let Some(left) = layout.left {
                draw_header_section(stdout, "PDF", left.x, 0, left.width, ChonkerTheme::accent_pdf())?;
            }
            if let Some(right) = layout.right {
                draw_header_section(stdout, "MARKDOWN", right.x, 0, right.width, ChonkerTheme::accent_options())?;
            }
        }
        DisplayMode::Options => {
            draw_header_section(stdout, "OPTIONS", 0, 0, layout.main.width, ChonkerTheme::accent_options())?;
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
    
    // Start near top for more content
    let mut y = area.y + 2;
    
    // Title
    execute!(
        stdout,
        MoveTo(area.x + 2, y),
        SetForegroundColor(ChonkerTheme::accent_text()),
        SetAttribute(Attribute::Bold),
        Print("OPTIONS & HELP"),
        SetAttribute(Attribute::Reset),
        ResetColor
    )?;
    y += 2;
    
    // Keyboard shortcuts section
    execute!(
        stdout,
        MoveTo(area.x + 2, y),
        SetForegroundColor(ChonkerTheme::accent_text()),
        SetAttribute(Attribute::Bold),
        Print("KEYBOARD SHORTCUTS"),
        SetAttribute(Attribute::Reset),
        ResetColor
    )?;
    y += 2;
    
    // List shortcuts
    let shortcuts = [
        ("Ctrl+O", "Open PDF file"),
        ("Ctrl+E", "Extract text from current page"),
        ("Ctrl+D", "Toggle dark/light mode"),
        ("Ctrl+Q", "Quit application"),
        ("Tab", "Switch between PDF/EDIT/MARKDOWN/OPTIONS"),
        ("", ""),
        ("EDIT Mode:", ""),
        ("↑↓←→", "Scroll text"),
        ("Type", "Edit text"),
        ("Backspace", "Delete before cursor"),
        ("Delete", "Delete at cursor"),
        ("Enter", "New line"),
        ("Cmd+C", "Copy selection"),
        ("Cmd+V", "Paste"),
        ("", ""),
        ("MARKDOWN Mode:", ""),
        ("↑↓", "Scroll content"),
    ];
    
    for (key, desc) in shortcuts {
        if key.is_empty() && desc.is_empty() {
            y += 1;
            continue;
        }
        
        if desc.is_empty() {
            // Section header
            execute!(
                stdout,
                MoveTo(area.x + 2, y),
                SetForegroundColor(ChonkerTheme::accent_text()),
                Print(key),
                ResetColor
            )?;
        } else if key.is_empty() {
            // Description only
            execute!(
                stdout,
                MoveTo(area.x + 4, y),
                SetForegroundColor(ChonkerTheme::text_dim()),
                Print(desc),
                ResetColor
            )?;
        } else {
            // Key + description
            execute!(
                stdout,
                MoveTo(area.x + 4, y),
                SetForegroundColor(ChonkerTheme::text_primary()),
                Print(format!("{:12}", key)),
                SetForegroundColor(ChonkerTheme::text_secondary()),
                Print(desc),
                ResetColor
            )?;
        }
        y += 1;
        
        // Stop if we're running out of space
        if y >= area.y + area.height - 2 {
            break;
        }
    }
    
    // Footer
    let footer = "Tab to return";
    execute!(
        stdout,
        MoveTo(area.x + (area.width - footer.len() as u16) / 2, area.y + area.height - 2),
        SetForegroundColor(ChonkerTheme::text_dim()),
        Print(footer),
        ResetColor
    )?;
    
    Ok(())
}

/// Handle mouse input for EDIT and MARKDOWN modes
fn handle_mouse_input(app: &mut App, mouse: MouseEvent, layout: &Layout) -> Result<()> {
    use crossterm::event::{MouseEventKind, MouseButton};
    
    // Handle MARKDOWN mode scrolling
    if app.display_mode == DisplayMode::PdfMarkdown && app.markdown_data.is_some() {
        if let Some(right) = layout.right {
            let panel_x = right.x + 1;
            let panel_y = right.y;
            let panel_width = right.width - 2;
            let panel_height = right.height;
            
            // Check if mouse is within the MARKDOWN panel
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
    
    // Handle EDIT mode
    if app.display_mode != DisplayMode::PdfEdit || app.edit_data.is_none() {
        return Ok(());
    }
    
    // Get the EDIT panel bounds
    if let Some(right) = layout.right {
        let panel_x = right.x + 1; // Account for border
        let panel_y = right.y;
        let panel_width = right.width - 2;
        let panel_height = right.height;
        
        // Check if mouse is within the EDIT panel
        if mouse.column >= panel_x && 
           mouse.column < panel_x + panel_width && 
           mouse.row >= panel_y && 
           mouse.row < panel_y + panel_height {
            
            match mouse.kind {
                MouseEventKind::Down(MouseButton::Left) => {
                    // Start selection
                    let grid_x = (mouse.column - panel_x) as usize;
                    let grid_y = (mouse.row - panel_y) as usize;
                    
                    if let Some(renderer) = &app.edit_display {
                        // Adjust for scroll offset
                        let (scroll_x, scroll_y) = renderer.get_scroll();
                        let actual_x = grid_x + scroll_x as usize;
                        let actual_y = grid_y + scroll_y as usize;
                        
                        app.cursor = (actual_x, actual_y);
                        app.selection_start = Some((actual_x, actual_y));
                        app.selection_end = None;
                        app.is_selecting = true;
                    }
                }
                MouseEventKind::Drag(MouseButton::Left) => {
                    // Update selection end
                    if app.is_selecting {
                        let grid_x = (mouse.column - panel_x) as usize;
                        let grid_y = (mouse.row - panel_y) as usize;
                        
                        if let Some(renderer) = &app.edit_display {
                            // Adjust for scroll offset
                            let (scroll_x, scroll_y) = renderer.get_scroll();
                            let actual_x = grid_x + scroll_x as usize;
                            let actual_y = grid_y + scroll_y as usize;
                            
                            app.selection_end = Some((actual_x, actual_y));
                        }
                    }
                }
                MouseEventKind::Up(MouseButton::Left) => {
                    // End selection
                    if app.is_selecting {
                        let grid_x = (mouse.column - panel_x) as usize;
                        let grid_y = (mouse.row - panel_y) as usize;
                        
                        if let Some(renderer) = &app.edit_display {
                            // Adjust for scroll offset
                            let (scroll_x, scroll_y) = renderer.get_scroll();
                            let actual_x = grid_x + scroll_x as usize;
                            let actual_y = grid_y + scroll_y as usize;
                            
                            app.selection_end = Some((actual_x, actual_y));
                            app.is_selecting = false;
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
