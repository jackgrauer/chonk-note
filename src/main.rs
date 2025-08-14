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
mod kitty_graphics;
mod markdown_renderer;

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
    spatial_recognition_enabled: bool,
}

pub struct App {
    pdf_path: PathBuf,
    current_page: usize,
    total_pages: usize,
    display_mode: DisplayMode,
    edit_data: Option<Vec<Vec<char>>>,
    edit_display: Option<EditPanelRenderer>,
    current_page_image: Option<DynamicImage>,
    markdown_data: Option<String>,
    markdown_renderer: Option<MarkdownRenderer>,
    settings: AppSettings,
    exit_requested: bool,
    status_message: String,
    term_width: u16,
    term_height: u16,
    dark_mode: bool, // Dark mode toggle
    // EDIT mode cursor and selection
    cursor: (usize, usize),  // (x, y) in grid
    selection_start: Option<(usize, usize)>,
    selection_end: Option<(usize, usize)>,
    is_selecting: bool,
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
        
        // Calculate PDF size based on display mode - much larger for better readability
        let (image_width, image_height) = match self.display_mode {
            DisplayMode::PdfEdit | DisplayMode::PdfMarkdown => {
                // For split modes, use almost full half screen
                let width = ((self.term_width / 2) - 4).max(40) as u32;
                let height = (self.term_height - 4).max(20) as u32;
                // Scale up for better quality (will be downscaled by terminal)
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
                self.status_message = format!("Page {}/{} - Press Ctrl+E to extract to EDIT panel", self.current_page + 1, self.total_pages);
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
            let _ = kitty_graphics::clear_graphics();
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
            let _ = kitty_graphics::clear_graphics();
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

/// Extract selected text from the edit buffer
fn extract_selection_text(app: &App) -> Option<String> {
    if let (Some(start), Some(end), Some(data)) = 
        (&app.selection_start, &app.selection_end, &app.edit_data) {
        
        let (start_row, start_col) = *start;
        let (end_row, end_col) = *end;
        
        // Normalize selection (ensure start comes before end)
        let ((start_row, start_col), (end_row, end_col)) = 
            if start_row < end_row || (start_row == end_row && start_col < end_col) {
                ((start_row, start_col), (end_row, end_col))
            } else {
                ((end_row, end_col), (start_row, start_col))
            };
        
        let mut selected_text = String::new();
        
        if start_row == end_row {
            // Single line selection
            if let Some(row) = data.get(start_row) {
                let start_idx = start_col.min(row.len());
                let end_idx = end_col.min(row.len());
                for i in start_idx..end_idx {
                    if let Some(&ch) = row.get(i) {
                        selected_text.push(ch);
                    }
                }
            }
        } else {
            // Multi-line selection
            for row_idx in start_row..=end_row {
                if let Some(row) = data.get(row_idx) {
                    if row_idx == start_row {
                        // First line: from start_col to end of line
                        for i in start_col..row.len() {
                            selected_text.push(row[i]);
                        }
                    } else if row_idx == end_row {
                        // Last line: from start of line to end_col
                        for i in 0..end_col.min(row.len()) {
                            selected_text.push(row[i]);
                        }
                    } else {
                        // Middle lines: entire line
                        for &ch in row {
                            selected_text.push(ch);
                        }
                    }
                    
                    // Add newline except for last row
                    if row_idx < end_row {
                        selected_text.push('\n');
                    }
                }
            }
        }
        
        if selected_text.is_empty() {
            None
        } else {
            Some(selected_text)
        }
    } else {
        None
    }
}

/// Paste text at cursor position
fn paste_at_cursor(app: &mut App, text: &str) {
    if let Some(data) = &mut app.edit_data {
        // Ensure we have a row to paste into
        while data.len() <= app.cursor.1 {
            data.push(vec![]);
        }
        
        let lines: Vec<&str> = text.lines().collect();
        
        if lines.is_empty() {
            return;
        }
        
        if lines.len() == 1 {
            // Single line paste
            let row = &mut data[app.cursor.1];
            let insert_pos = app.cursor.0.min(row.len());
            
            for (i, ch) in lines[0].chars().enumerate() {
                row.insert(insert_pos + i, ch);
            }
            app.cursor.0 += lines[0].len();
        } else {
            // Multi-line paste
            let current_row = &mut data[app.cursor.1];
            let insert_pos = app.cursor.0.min(current_row.len());
            
            // Split current line at cursor
            let remaining_chars: Vec<char> = current_row.drain(insert_pos..).collect();
            
            // Insert first line
            for ch in lines[0].chars() {
                current_row.push(ch);
            }
            
            // Insert middle lines
            for line in &lines[1..lines.len()-1] {
                let new_line: Vec<char> = line.chars().collect();
                data.insert(app.cursor.1 + 1, new_line);
                app.cursor.1 += 1;
            }
            
            // Insert last line and remaining chars
            if lines.len() > 1 {
                let mut last_line: Vec<char> = lines[lines.len()-1].chars().collect();
                app.cursor.0 = last_line.len();
                last_line.extend(remaining_chars);
                data.insert(app.cursor.1 + 1, last_line);
                app.cursor.1 += 1;
            }
        }
        
        // Update renderer
        if let Some(renderer) = &mut app.edit_display {
            renderer.update_buffer(data);
        }
    }
}

/// Copy text to clipboard
fn copy_to_clipboard(text: &str) -> Result<()> {
    use cli_clipboard::{ClipboardContext, ClipboardProvider};
    
    let mut ctx: ClipboardContext = ClipboardProvider::new()
        .map_err(|e| anyhow::anyhow!("Failed to create clipboard context: {}", e))?;
    
    ctx.set_contents(text.to_owned())
        .map_err(|e| anyhow::anyhow!("Failed to set clipboard contents: {}", e))?;
    
    Ok(())
}

/// Paste text from clipboard
fn paste_from_clipboard() -> Result<String> {
    use cli_clipboard::{ClipboardContext, ClipboardProvider};
    
    let mut ctx: ClipboardContext = ClipboardProvider::new()
        .map_err(|e| anyhow::anyhow!("Failed to create clipboard context: {}", e))?;
    
    ctx.get_contents()
        .map_err(|e| anyhow::anyhow!("Failed to get clipboard contents: {}", e))
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
    let _ = kitty_graphics::clear_graphics();
    
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
                        // Use full left panel for PDF
                        let _ = kitty_graphics::display_image(
                            image,
                            left.x,
                            left.y,
                            left.width,
                            left.height,
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
                    if handle_input(app, key, runtime)? {
                        if app.exit_requested {
                            // Clear graphics before quitting
                            let _ = kitty_graphics::clear_graphics();
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
        " {} | [Ctrl+O]pen [Ctrl+N]ext [Ctrl+P]rev [Ctrl+E]xtract [Tab]Mode [Ctrl+Q]uit ",
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
    
    // Center content horizontally
    let content_width = 50.min(area.width - 4); // Max width of 50 chars
    let start_x = area.x + (area.width - content_width) / 2;
    
    // Start content vertically centered
    let total_lines = 10; // Reduced for single option
    let start_y = area.y + (area.height - total_lines) / 2;
    
    let mut y_offset = 3;
    
    // Single toggle option
    let (cursor_char, highlight_color) = (">", ChonkerTheme::accent_text());
    
    let status = if app.settings.spatial_recognition_enabled { "ON" } else { "OFF" };
    let status_color = if app.settings.spatial_recognition_enabled { 
        ChonkerTheme::success() 
    } else { 
        ChonkerTheme::error() 
    };
    
    execute!(
        stdout,
        MoveTo(start_x, start_y + y_offset),
        SetForegroundColor(highlight_color),
        Print(format!("{} ENABLE SPATIAL RECOGNITION WITH MARKDOWN", cursor_char)),
        ResetColor,
        MoveTo(start_x + 45, start_y + y_offset),
        SetForegroundColor(status_color),
        Print(status),
        ResetColor
    )?;
    y_offset += 4;
    
    // Footer with controls - centered at bottom
    let controls = "Space Toggle   Tab Return";
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

/// Handle mouse input for EDIT mode
fn handle_mouse_input(app: &mut App, mouse: MouseEvent, layout: &Layout) -> Result<()> {
    use crossterm::event::{MouseEventKind, MouseButton};
    
    // Only handle mouse events in EDIT mode and when we have content
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
                _ => {}
            }
        }
    }
    
    Ok(())
}

/// Handle keyboard input
fn handle_input(app: &mut App, key: KeyEvent, runtime: &tokio::runtime::Runtime) -> Result<bool> {
    match key.code {
        KeyCode::Char('q') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.exit_requested = true;
            return Ok(true);
        }
        
        KeyCode::Char('n') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.next_page();
            runtime.block_on(app.load_pdf_page())?;
        }
        
        KeyCode::Right => {
            app.next_page();
            runtime.block_on(app.load_pdf_page())?;
        }
        
        KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.prev_page();
            runtime.block_on(app.load_pdf_page())?;
        }
        
        KeyCode::Left => {
            app.prev_page();
            runtime.block_on(app.load_pdf_page())?;
        }
        
        KeyCode::Tab => {
            app.toggle_mode();
            // DON'T reload PDF - this was causing the flicker!
            // The existing image will be displayed in the new mode automatically
        }
        
        KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.dark_mode = !app.dark_mode;
            app.status_message = format!("Mode: {}", if app.dark_mode { "Dark" } else { "Light" });
        }
        
        KeyCode::Char('o') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            // Open new file
            disable_raw_mode()?;
            println!("\r\n🐹 Opening file picker...\r");
            
            let new_file = file_picker::pick_pdf_file()?;
            
            enable_raw_mode()?;
            
            if let Some(new_file) = new_file {
                if let Ok(new_app) = App::new(new_file.clone(), 1, "edit") {
                    *app = new_app;
                    app.status_message = format!("Loaded: {}", new_file.display());
                    runtime.block_on(app.load_pdf_page())?;
                }
            }
        }
        
        KeyCode::Char('e') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            runtime.block_on(app.extract_current_page())?;
        }
        
        // EDIT mode keyboard handlers - only active when in EDIT mode with content
        _ if app.display_mode == DisplayMode::PdfEdit && app.edit_data.is_some() => {
            match key.code {
                // Copy selection
                KeyCode::Char('c') if key.modifiers.contains(MOD_KEY) => {
                    if let Some(text) = extract_selection_text(app) {
                        if let Err(e) = copy_to_clipboard(&text) {
                            app.status_message = format!("Copy failed: {}", e);
                        } else {
                            app.status_message = "Text copied to clipboard".to_string();
                        }
                    }
                }
                
                // Paste at cursor
                KeyCode::Char('v') if key.modifiers.contains(MOD_KEY) => {
                    match paste_from_clipboard() {
                        Ok(text) => {
                            paste_at_cursor(app, &text);
                            app.status_message = "Text pasted".to_string();
                        }
                        Err(e) => {
                            app.status_message = format!("Paste failed: {}", e);
                        }
                    }
                }
                
                // Arrow key navigation (move cursor)
                KeyCode::Up => {
                    if app.cursor.1 > 0 {
                        app.cursor.1 -= 1;
                    }
                }
                KeyCode::Down => {
                    if let Some(data) = &app.edit_data {
                        if app.cursor.1 < data.len().saturating_sub(1) {
                            app.cursor.1 += 1;
                        }
                    }
                }
                KeyCode::Left => {
                    if app.cursor.0 > 0 {
                        app.cursor.0 -= 1;
                    }
                }
                KeyCode::Right => {
                    if let Some(data) = &app.edit_data {
                        if app.cursor.1 < data.len() {
                            if let Some(row) = data.get(app.cursor.1) {
                                if app.cursor.0 < row.len().saturating_sub(1) {
                                    app.cursor.0 += 1;
                                }
                            }
                        }
                    }
                }
                
                // Backspace - delete character before cursor
                KeyCode::Backspace => {
                    if let Some(data) = &mut app.edit_data {
                        if app.cursor.0 > 0 {
                            // Delete character in current row
                            if app.cursor.1 < data.len() {
                                if app.cursor.0 <= data[app.cursor.1].len() {
                                    data[app.cursor.1].remove(app.cursor.0 - 1);
                                    app.cursor.0 -= 1;
                                    
                                    // Update renderer
                                    if let Some(renderer) = &mut app.edit_display {
                                        renderer.update_buffer(data);
                                    }
                                }
                            }
                        } else if app.cursor.1 > 0 {
                            // Join current line with previous line
                            let current_line = if app.cursor.1 < data.len() {
                                data.remove(app.cursor.1)
                            } else {
                                vec![]
                            };
                            
                            if app.cursor.1 > 0 {
                                app.cursor.0 = data[app.cursor.1 - 1].len();
                                data[app.cursor.1 - 1].extend(current_line);
                                app.cursor.1 -= 1;
                                
                                // Update renderer
                                if let Some(renderer) = &mut app.edit_display {
                                    renderer.update_buffer(data);
                                }
                            }
                        }
                    }
                }
                
                // Delete - delete character at cursor
                KeyCode::Delete => {
                    if let Some(data) = &mut app.edit_data {
                        if app.cursor.1 < data.len() {
                            let row_len = data[app.cursor.1].len();
                            if app.cursor.0 < row_len {
                                // Delete character at cursor position
                                data[app.cursor.1].remove(app.cursor.0);
                                
                                // Update renderer
                                if let Some(renderer) = &mut app.edit_display {
                                    renderer.update_buffer(data);
                                }
                            } else if app.cursor.1 + 1 < data.len() {
                                // Join next line with current line
                                let next_line = data.remove(app.cursor.1 + 1);
                                data[app.cursor.1].extend(next_line);
                                
                                // Update renderer
                                if let Some(renderer) = &mut app.edit_display {
                                    renderer.update_buffer(data);
                                }
                            }
                        }
                    }
                }
                
                // Enter - insert new line
                KeyCode::Enter => {
                    if let Some(data) = &mut app.edit_data {
                        if app.cursor.1 < data.len() {
                            // Split current line at cursor position
                            let current_row = &mut data[app.cursor.1];
                            let split_point = app.cursor.0.min(current_row.len());
                            let new_line: Vec<char> = current_row.drain(split_point..).collect();
                            
                            // Insert new line
                            data.insert(app.cursor.1 + 1, new_line);
                            app.cursor.1 += 1;
                            app.cursor.0 = 0;
                            
                            // Update renderer
                            if let Some(renderer) = &mut app.edit_display {
                                renderer.update_buffer(data);
                            }
                        }
                    }
                }
                
                // Regular character input
                KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) && !key.modifiers.contains(KeyModifiers::ALT) => {
                    if let Some(data) = &mut app.edit_data {
                        // Ensure we have a row to insert into
                        while data.len() <= app.cursor.1 {
                            data.push(vec![]);
                        }
                        
                        // Insert character at cursor position
                        let row = &mut data[app.cursor.1];
                        let insert_pos = app.cursor.0.min(row.len());
                        row.insert(insert_pos, c);
                        app.cursor.0 += 1;
                        
                        // Update renderer
                        if let Some(renderer) = &mut app.edit_display {
                            renderer.update_buffer(data);
                        }
                        
                        // Clear any selection when typing
                        app.selection_start = None;
                        app.selection_end = None;
                        app.is_selecting = false;
                    }
                }
                
                _ => {}
            }
        }
        
        KeyCode::Char(' ') | KeyCode::Enter if app.display_mode == DisplayMode::Options => {
            app.settings.spatial_recognition_enabled = !app.settings.spatial_recognition_enabled;
            app.status_message = format!("Spatial recognition with MARKDOWN: {}",
                if app.settings.spatial_recognition_enabled { "ON" } else { "OFF" });
            runtime.block_on(app.extract_current_page())?;
        }
        
        // Scrolling for EDIT panel and MARKDOWN panel (not in OPTIONS mode)
        KeyCode::Char('j') => {
            if app.display_mode == DisplayMode::PdfEdit {
                if let Some(renderer) = &mut app.edit_display {
                    renderer.scroll_down(1);
                }
            } else if app.display_mode == DisplayMode::PdfMarkdown {
                if let Some(renderer) = &mut app.markdown_renderer {
                    renderer.scroll_down(1);
                }
            }
        }
        KeyCode::Down if app.display_mode != DisplayMode::Options => {
            if app.display_mode == DisplayMode::PdfEdit {
                if let Some(renderer) = &mut app.edit_display {
                    renderer.scroll_down(1);
                }
            } else if app.display_mode == DisplayMode::PdfMarkdown {
                if let Some(renderer) = &mut app.markdown_renderer {
                    renderer.scroll_down(1);
                }
            }
        }
        KeyCode::Char('k') => {
            if app.display_mode == DisplayMode::PdfEdit {
                if let Some(renderer) = &mut app.edit_display {
                    renderer.scroll_up(1);
                }
            } else if app.display_mode == DisplayMode::PdfMarkdown {
                if let Some(renderer) = &mut app.markdown_renderer {
                    renderer.scroll_up(1);
                }
            }
        }
        KeyCode::Up if app.display_mode != DisplayMode::Options => {
            if app.display_mode == DisplayMode::PdfEdit {
                if let Some(renderer) = &mut app.edit_display {
                    renderer.scroll_up(1);
                }
            } else if app.display_mode == DisplayMode::PdfMarkdown {
                if let Some(renderer) = &mut app.markdown_renderer {
                    renderer.scroll_up(1);
                }
            }
        }
        KeyCode::Char('h') => {
            if let Some(renderer) = &mut app.edit_display {
                renderer.scroll_left(1);
            }
        }
        KeyCode::Char('l') => {
            if let Some(renderer) = &mut app.edit_display {
                renderer.scroll_right(1);
            }
        }
        KeyCode::PageDown => {
            if app.display_mode == DisplayMode::PdfEdit {
                if let Some(renderer) = &mut app.edit_display {
                    renderer.scroll_down(10);
                }
            } else if app.display_mode == DisplayMode::PdfMarkdown {
                if let Some(renderer) = &mut app.markdown_renderer {
                    renderer.scroll_down(10);
                }
            }
        }
        KeyCode::PageUp => {
            if app.display_mode == DisplayMode::PdfEdit {
                if let Some(renderer) = &mut app.edit_display {
                    renderer.scroll_up(10);
                }
            } else if app.display_mode == DisplayMode::PdfMarkdown {
                if let Some(renderer) = &mut app.markdown_renderer {
                    renderer.scroll_up(10);
                }
            }
        }
        
        _ => {}
    }
    
    Ok(true)
}