// MINIMAL CHONKER - Just PDF text extraction to editable grid
use anyhow::Result;
// CROSSTERM ELIMINATED! Pure Kitty-native PDF viewer
use kitty_native::{KittyTerminal, KeyCode};
// All crossterm eliminated - pure Kitty ANSI
use std::{io::{self, Write}, path::PathBuf};
use image::DynamicImage;
use clap::Parser;

// HELIX-CORE INTEGRATION! Professional text editing
use helix_core::{
    Rope, Selection,
    history::History,
};

mod content_extractor;
mod edit_renderer;
mod pdf_renderer;
mod kitty_file_picker;
mod viuer_display;
mod keyboard;
mod dual_pane_keyboard;
mod kitty_native;
mod mouse;
mod block_selection;
mod notes_database;
mod notes_mode;

use edit_renderer::EditPanelRenderer;
use mouse::MouseState;
use block_selection::BlockSelection;
// Theme eliminated - using direct ANSI


#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ExtractionMethod {
    Segments,    // Current PDFium segments method
    PdfAlto,     // PDFAlto-style word-by-word extraction
    LeptessOCR,  // Leptess OCR extraction
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AppMode {
    PdfViewer,
    NotesEditor,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ActivePane {
    Left,   // Notes in NotesEditor mode, PDF in PdfViewer mode
    Right,  // Always extraction text
}

#[derive(Parser, Debug)]
#[command(name = "chonker7", author, version, about)]
struct Args {
    /// PDF file to open, or "notes" to enter notes mode
    file_or_mode: Option<String>,

    #[arg(short, long, default_value_t = 1)]
    page: usize,
}

pub struct App {
    // Mode
    pub app_mode: AppMode,
    pub notes_mode: Option<notes_mode::NotesMode>,

    // PDF-related fields (keep unchanged)
    pub pdf_path: PathBuf,
    pub current_page: usize,
    pub total_pages: usize,
    pub current_page_image: Option<DynamicImage>,
    pub extraction_method: ExtractionMethod,
    pub dual_pane_mode: bool,

    // HELIX-CORE INTEGRATION! Professional text editing
    pub rope: Rope,                    // Main text buffer (used for current active pane)
    pub extraction_rope: Rope,         // Extracted text from PDF (right pane)
    pub notes_rope: Rope,              // Notes text (left pane when in notes mode)
    pub selection: Selection,          // Current pane cursor + selections
    pub extraction_selection: Selection,  // Right pane selection state
    pub notes_selection: Selection,    // Left pane (notes) selection state
    pub history: History,              // Undo/redo for free!
    pub block_selection: Option<BlockSelection>,  // Proper block selection with visual columns
    pub virtual_cursor_col: Option<usize>,  // Virtual cursor column for navigating past line ends
    pub active_pane: ActivePane,       // Which pane has focus

    // Rendering
    pub edit_display: Option<EditPanelRenderer>,
    pub notes_display: Option<EditPanelRenderer>,  // Separate renderer for notes pane

    // App state (keep unchanged)
    pub status_message: String,
    pub dark_mode: bool,
    pub exit_requested: bool,
    pub needs_redraw: bool,
    pub open_file_picker: bool,


    // Viewport tracking for flicker prevention
    pub last_viewport_scroll: (u16, u16),

    // Pane split position (x coordinate where the divider is)
    pub split_position: Option<u16>,  // None means use default (50/50 split)
    pub is_dragging_divider: bool,    // Track if user is dragging the divider

    // PDF viewport scrolling
    pub pdf_scroll_x: u16,            // Horizontal scroll position for PDF
    pub pdf_scroll_y: u16,            // Vertical scroll position for PDF
    pub pdf_full_width: u16,          // Full width of PDF image
    pub pdf_full_height: u16,         // Full height of PDF image

    // Zoom levels for each pane (1.0 = 100%)
    pub pdf_zoom: f32,                // Zoom level for PDF pane
    pub text_zoom: f32,               // Zoom level for text editor pane

}

impl App {
    pub fn new(pdf_path: PathBuf, start_page: usize) -> Result<Self> {
        let total_pages = content_extractor::get_page_count(&pdf_path)?;
        Ok(Self {
            // PDF-related fields
            pdf_path: pdf_path.clone(),
            current_page: start_page.saturating_sub(1),
            total_pages,
            current_page_image: None,
            extraction_method: ExtractionMethod::Segments,
            dual_pane_mode: true,

            // HELIX-CORE INTEGRATION!
            rope: Rope::from(""),                    // Main active buffer
            extraction_rope: Rope::from(""),         // Extraction text (right pane)
            notes_rope: Rope::from(""),              // Notes text (left pane in notes mode)
            selection: Selection::point(0),          // Cursor at position 0
            extraction_selection: Selection::point(0),
            notes_selection: Selection::point(0),
            history: History::default(),             // Empty history
            block_selection: None,                   // No block selection initially
            virtual_cursor_col: None,                // No virtual cursor position initially
            active_pane: ActivePane::Right,          // Start with extraction pane active

            // Rendering
            edit_display: None,
            notes_display: None,

            // App state
            status_message: "Ctrl+J/K: Pages | Ctrl+T: Extraction | Ctrl+E: Notes | Ctrl+Q: Quit".to_string(),
            dark_mode: true,
            exit_requested: false,
            needs_redraw: true,
            open_file_picker: false,


            // Viewport tracking
            last_viewport_scroll: (0, 0),

            // Pane split
            split_position: None,  // Start with default 50/50 split
            is_dragging_divider: false,

            // PDF viewport scrolling
            pdf_scroll_x: 0,
            pdf_scroll_y: 0,
            pdf_full_width: 800,   // Will be updated when image loads
            pdf_full_height: 1000, // Will be updated when image loads

            // Zoom levels
            pdf_zoom: 1.0,
            text_zoom: 1.0,

            // Mode
            app_mode: AppMode::PdfViewer,
            notes_mode: None,
        })
    }

    pub fn new_notes_mode() -> Result<Self> {
        Ok(Self {
            // Mode
            app_mode: AppMode::NotesEditor,
            notes_mode: Some(notes_mode::NotesMode::new()?),

            // PDF-related fields (empty until PDF is loaded)
            pdf_path: PathBuf::new(),  // Empty path, will be set when opening PDF
            current_page: 0,
            total_pages: 0,
            current_page_image: None,
            extraction_method: ExtractionMethod::Segments,
            dual_pane_mode: false,

            // HELIX-CORE INTEGRATION!
            rope: Rope::from("# Notes\n\nPress Ctrl+N to create a new note, Ctrl+L to list notes\n"),
            extraction_rope: Rope::from(""),
            notes_rope: Rope::from("# Notes\n\nPress Ctrl+N to create a new note, Ctrl+L to list notes\n"),
            selection: Selection::point(0),
            extraction_selection: Selection::point(0),
            notes_selection: Selection::point(0),
            history: History::default(),
            block_selection: None,
            virtual_cursor_col: None,
            active_pane: ActivePane::Left,  // Notes mode starts with left pane active

            // Rendering
            edit_display: None,
            notes_display: None,

            // App state
            status_message: "Notes mode - Ctrl+N for new, Ctrl+L to list".to_string(),
            dark_mode: true,
            exit_requested: false,
            needs_redraw: true,
            open_file_picker: false,

            // Viewport tracking
            last_viewport_scroll: (0, 0),

            // Pane split (not used in notes mode)
            split_position: None,
            is_dragging_divider: false,

            // PDF viewport scrolling (not used)
            pdf_scroll_x: 0,
            pdf_scroll_y: 0,
            pdf_full_width: 800,
            pdf_full_height: 1000,

            // Zoom levels
            pdf_zoom: 1.0,
            text_zoom: 1.0,
        })
    }

    pub async fn load_pdf_page(&mut self) -> Result<()> {
        // Apply zoom to the base size when rendering (reduced by 25% for better text alignment)
        let base_width = 900;  // Was 1200, reduced by 25%
        let base_height = 1200; // Was 1600, reduced by 25%
        let zoomed_width = (base_width as f32 * self.pdf_zoom) as u32;
        let zoomed_height = (base_height as f32 * self.pdf_zoom) as u32;

        // Render page at zoomed size
        let image = pdf_renderer::render_pdf_page(&self.pdf_path, self.current_page, zoomed_width, zoomed_height)?;

        // Update PDF dimensions - convert from pixels to terminal cells
        // Typical terminal cell is about 7x14 pixels
        let cell_width = 7;
        let cell_height = 14;
        self.pdf_full_width = (image.width() / cell_width) as u16;
        self.pdf_full_height = (image.height() / cell_height) as u16;

        // Reset scroll position when loading new page
        self.pdf_scroll_x = 0;
        self.pdf_scroll_y = 0;

        self.current_page_image = Some(image);

        // Auto-extract text with current method
        self.extract_current_page().await?;

        Ok(())
    }

    pub async fn extract_current_page(&mut self) -> Result<()> {
        // Use current terminal size for dual pane extraction
        let (term_width, term_height) = KittyTerminal::size().unwrap_or((120, 40));
        let text_width = term_width / 2; // Always dual pane mode
        let text_height = term_height.saturating_sub(2);

        // HELIX-CORE INTEGRATION! Extract to Rope
        let matrix = content_extractor::extract_to_matrix_with_method(
            &self.pdf_path,
            self.current_page,
            text_width.max(400) as usize,  // Minimum 400 columns
            text_height.max(200) as usize, // Minimum 200 rows
            self.extraction_method
        ).await?;

        // Convert matrix to Rope
        let text = matrix.iter()
            .map(|row| row.iter().collect::<String>())
            .collect::<Vec<_>>()
            .join("\n");

        self.extraction_rope = Rope::from_str(&text);
        // If in PDF mode, update the main rope for editing
        if self.app_mode == AppMode::PdfViewer {
            self.rope = Rope::from_str(&text);
        }
        self.selection = Selection::point(0);  // Reset cursor to top-left
        self.virtual_cursor_col = Some(0);  // Reset virtual cursor position

        // Update renderer from rope and reset viewport to top-left
        if let Some(renderer) = &mut self.edit_display {
            renderer.update_from_rope(&self.rope);
            // Reset viewport to show top-left of extracted text
            renderer.scroll_x = 0;
            renderer.scroll_y = 0;
            renderer.viewport_x = 0;
            renderer.viewport_y = 0;
        } else {
            let mut renderer = EditPanelRenderer::new(text_width, text_height);
            renderer.update_from_rope(&self.rope);
            self.edit_display = Some(renderer);
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
            // HELIX-CORE: Clear extraction rope and reset
            self.extraction_rope = Rope::from("");
            if self.app_mode == AppMode::PdfViewer {
                self.rope = Rope::from("");
            }
            self.selection = Selection::point(0);
            self.virtual_cursor_col = Some(0);  // Reset virtual cursor
            self.edit_display = None;
            self.current_page_image = None;
            self.needs_redraw = true;
        }
    }

    pub fn prev_page(&mut self) {
        if self.current_page > 0 {
            let _ = viuer_display::clear_graphics();
            self.current_page -= 1;
            // HELIX-CORE: Clear extraction rope and reset
            self.extraction_rope = Rope::from("");
            if self.app_mode == AppMode::PdfViewer {
                self.rope = Rope::from("");
            }
            self.selection = Selection::point(0);
            self.virtual_cursor_col = Some(0);  // Reset virtual cursor
            self.edit_display = None;
            self.current_page_image = None;
            self.needs_redraw = true;
        }
    }

    // Key repeat system eliminated - helix-core handles movement

    // Force re-extraction (always dual pane mode now)
    pub async fn refresh_extraction(&mut self) -> Result<()> {
        self.extract_current_page().await?;
        self.needs_redraw = true;
        Ok(())
    }

    // Toggle between PDF and Notes modes
    // Switch active pane and save/restore appropriate selection states
    pub fn switch_active_pane(&mut self, pane: ActivePane) {
        // Save current selection to the appropriate pane
        match self.active_pane {
            ActivePane::Left => self.notes_selection = self.selection.clone(),
            ActivePane::Right => self.extraction_selection = self.selection.clone(),
        }

        // Switch to new pane and restore its selection
        self.active_pane = pane;
        match pane {
            ActivePane::Left => {
                self.rope = self.notes_rope.clone();
                self.selection = self.notes_selection.clone();
            }
            ActivePane::Right => {
                self.rope = self.extraction_rope.clone();
                self.selection = self.extraction_selection.clone();
            }
        }
        self.needs_redraw = true;
    }

    pub fn toggle_notes_mode(&mut self) -> Result<()> {
        match self.app_mode {
            AppMode::PdfViewer => {
                // Switch to notes mode
                if self.notes_mode.is_none() {
                    self.notes_mode = Some(notes_mode::NotesMode::new()?);
                }
                self.app_mode = AppMode::NotesEditor;

                // Load notes into notes_rope and switch active rope
                if self.notes_rope.len_chars() == 0 {
                    self.notes_rope = Rope::from("# Notes\n\nCtrl+N: New note | Ctrl+S: Save | Ctrl+L: List | Ctrl+F: Search\nCtrl+E: Back to PDF\n");
                }
                self.rope = self.notes_rope.clone();  // Make notes the active buffer
                self.selection = Selection::point(0);
                self.status_message = "Notes Mode - Ctrl+E to return to PDF".to_string();

                // Keep dual pane mode - notes on left, extraction on right
                self.dual_pane_mode = true;
                self.needs_redraw = true;
            }
            AppMode::NotesEditor => {
                // Check if we have a valid PDF path before switching
                if self.pdf_path.as_os_str().is_empty() || !self.pdf_path.exists() {
                    // No PDF loaded yet, open file picker
                    self.status_message = "No PDF loaded - opening file picker...".to_string();
                    self.open_file_picker = true;
                    return Ok(());
                }

                // Save notes before switching back
                self.notes_rope = self.rope.clone();

                // Switch back to PDF mode
                self.app_mode = AppMode::PdfViewer;
                self.dual_pane_mode = true;
                self.status_message = "PDF Mode - Ctrl+J/K: Pages | Ctrl+T: Extraction | Ctrl+E: Notes".to_string();

                // Restore extraction text as active buffer
                self.rope = self.extraction_rope.clone();
                self.selection = Selection::point(0);
                self.needs_redraw = true;
            }
        }
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
                // HELIX-CORE: Convert OCR result to rope
                let text = data.iter()
                    .map(|row| row.iter().collect::<String>())
                    .collect::<Vec<_>>()
                    .join("\n");
                self.extraction_rope = Rope::from_str(&text);
                // If in PDF mode, update the main rope for editing
                if self.app_mode == AppMode::PdfViewer {
                    self.rope = Rope::from_str(&text);
                }
                self.selection = Selection::point(0);  // Reset cursor to top-left
                self.virtual_cursor_col = Some(0);  // Reset virtual cursor position
                // HELIX-CORE: Update renderer from rope and reset viewport
                if let Some(renderer) = &mut self.edit_display {
                    renderer.update_from_rope(&self.rope);
                    // Reset viewport to show top-left of extracted text
                    renderer.scroll_x = 0;
                    renderer.scroll_y = 0;
                    renderer.viewport_x = 0;
                    renderer.viewport_y = 0;
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
    // Commented out: stderr redirection causes issues with file picker
    // unsafe {
    //     let dev_null = libc::open(b"/dev/null\0".as_ptr() as *const i8, libc::O_WRONLY);
    //     if dev_null != -1 {
    //         libc::dup2(dev_null, libc::STDERR_FILENO);
    //         libc::close(dev_null);
    //     }
    // }

    let args = Args::parse();

    // Check if user wants to start in notes mode or open a PDF
    let mut app = if let Some(path_str) = args.file_or_mode {
        if path_str == "notes" {
            // Start directly in notes mode
            App::new_notes_mode()?
        } else {
            // Open the specified PDF file
            let pdf_path = PathBuf::from(path_str);
            let mut app = App::new(pdf_path, args.page)?;
            app.load_pdf_page().await?;
            app
        }
    } else {
        // No argument provided - show file picker for PDF
        if let Some(path) = kitty_file_picker::pick_pdf_file()? {
            let mut app = App::new(path, args.page)?;
            app.load_pdf_page().await?;
            app
        } else {
            // User cancelled file picker
            std::process::exit(0);
        }
    };
    
    setup_terminal()?;
    let result = run_app(&mut app).await;
    restore_terminal()?;
    
    result
}


fn setup_terminal() -> Result<()> {
    // CROSSTERM ELIMINATED! Pure Kitty-native
    KittyTerminal::enable_raw_mode().map_err(|e| anyhow::anyhow!("Terminal setup failed: {}", e))?;
    KittyTerminal::enter_fullscreen().map_err(|e| anyhow::anyhow!("Fullscreen failed: {}", e))?;
    Ok(())
}

fn restore_terminal() -> Result<()> {
    let _ = viuer_display::clear_graphics();

    // CROSSTERM ELIMINATED! Pure Kitty-native
    KittyTerminal::exit_fullscreen().map_err(|e| anyhow::anyhow!("Exit fullscreen failed: {}", e))?;
    KittyTerminal::disable_raw_mode().map_err(|e| anyhow::anyhow!("Disable raw mode failed: {}", e))?;
    Ok(())
}

async fn run_app(app: &mut App) -> Result<()> {
    let mut stdout = io::stdout();
    let mut last_term_size = (0, 0);
    let mut last_render_time = std::time::Instant::now();
    let mut mouse_state = MouseState::default();


    // Initial render
    app.needs_redraw = true;
    
    loop {
        let (term_width, term_height) = KittyTerminal::size()?;
        // Use custom split position or default to 50/50
        let split_x = app.split_position.unwrap_or(term_width / 2);
        
        // Check if terminal was resized
        if (term_width, term_height) != last_term_size {
            app.needs_redraw = true;
            last_term_size = (term_width, term_height);
        }
        
        // Check if we need to open file picker
        if app.open_file_picker {
            app.open_file_picker = false;
            restore_terminal()?;

            use kitty_file_picker::FilePickerAction;
            match kitty_file_picker::pick_pdf_with_action()? {
                FilePickerAction::Open(new_path) => {
                    app.pdf_path = new_path;
                    app.current_page = 0;
                    app.total_pages = content_extractor::get_page_count(&app.pdf_path)?;
                    app.load_pdf_page().await?;

                    // If we were in notes mode, switch to PDF mode now
                    if app.app_mode == AppMode::NotesEditor {
                        app.app_mode = AppMode::PdfViewer;
                        app.dual_pane_mode = true;
                        app.status_message = "PDF Mode - Ctrl+J/K: Pages | Ctrl+T: Extraction | Ctrl+E: Notes".to_string();
                    }

                    app.needs_redraw = true;
                }
                FilePickerAction::Cancel => {
                    // User cancelled - stay in current mode
                }
            }

            setup_terminal()?;
            app.needs_redraw = true;
        }
        
        // NUCLEAR ANTI-FLICKER: Only redraw when absolutely necessary
        let now = std::time::Instant::now();
        let frame_time = now.duration_since(last_render_time);

        if app.needs_redraw && frame_time.as_millis() >= 50 { // Max 20 FPS - nuclear anti-flicker
            // CROSSTERM ELIMINATED! Direct Kitty positioning
            KittyTerminal::move_to(0, 0)?;
            last_render_time = now;

            // Clear the entire screen to prevent artifacts when resizing
            print!("\x1b[2J");
            stdout.flush()?;

            // Save cursor position
            print!("\x1b[s]");

            // Always render split view - left pane changes based on mode
            if app.app_mode == AppMode::NotesEditor {
                // In notes mode, render notes on left, extraction text on right
                render_notes_pane(app, 0, 0, split_x - 1, term_height)?;
            } else {
                // In PDF mode, render PDF on left
                render_pdf_pane(app, 0, 0, split_x - 1, term_height)?;
            }

            // Draw draggable divider between panes (always visible)
            {
                let divider_color = if app.is_dragging_divider {
                    "\x1b[38;2;100;150;255m"  // Bright blue when dragging
                } else {
                    "\x1b[38;2;80;80;80m"     // Medium gray normally
                };

                for y in 0..term_height {
                    print!("\x1b[{};{}H{}┃\x1b[0m",
                        y + 1, split_x, divider_color);
                }
            }

            // Always render extraction text on the right
            render_text_pane(app, split_x + 1, 0, term_width - split_x - 1, term_height)?;

            // Restore cursor position
            print!("\x1b[u]");
            stdout.flush()?;

            // Status bar disabled to prevent debug flood
            // render_status_bar(&mut stdout, app, term_width, term_height)?;

            stdout.flush()?;
            app.needs_redraw = false;
        }

        // DISABLED: Text pane trackpad gestures are not active yet
        // Momentum-based scrolling is disabled for the text pane
        // if mouse_state.scroll_momentum.velocity_y.abs() > 0.1 ||
        //    mouse_state.scroll_momentum.velocity_x.abs() > 0.1 {
        //     mouse::apply_smooth_scroll(app, &mut mouse_state);
        // }

        // CROSSTERM ELIMINATED! Direct Kitty input
        if KittyTerminal::poll_input()? {
            // Use new unified input API
            if let Some(input) = KittyTerminal::read_input()? {
                match input {
                    kitty_native::InputEvent::Key(key) => {
                        // Debug log key event
                        if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open("/Users/jack/chonker7_debug.log") {
                            use std::io::Write;
                            writeln!(file, "[MAIN] Received key event: {:?}", key).ok();
                        }

                        // HELIX-CORE: Track selection changes
                        let old_selection = app.selection.clone();

                        if !keyboard::handle_input(app, key).await? {
                            break;
                        }
                        if app.exit_requested {
                            break;
                        }

                        // HELIX-CORE: Check for changes
                        let selection_changed = app.selection != old_selection;

                        if selection_changed {
                            // Any selection change triggers redraw
                            app.needs_redraw = true;
                        }
                    }
                    kitty_native::InputEvent::Mouse(mouse_event) => {
                        // Debug log mouse event received
                        if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open("/Users/jack/chonker7_debug.log") {
                            use std::io::Write;
                            writeln!(file, "[MAIN] Received mouse event").ok();
                        }
                        // Handle mouse events
                        mouse::handle_mouse(app, mouse_event, &mut mouse_state).await?;
                    }
                }
            }
        }
    }
    
    Ok(())
}

/// Render the PDF pane with zoom controls
fn render_pdf_pane(app: &mut App, x: u16, y: u16, width: u16, height: u16) -> Result<()> {
    // Draw container border
    print!("\x1b[{};{}H\x1b[38;2;60;60;60m╭{}╮\x1b[0m",
        y + 1, x + 1, "─".repeat((width - 2) as usize));

    // Draw zoom controls in top right corner of PDF pane
    let zoom_text = format!(" {:.0}% ", app.pdf_zoom * 100.0);
    let button_x = x + width - 12;
    print!("\x1b[{};{}H\x1b[38;2;100;100;100m[\x1b[38;2;200;200;200m-\x1b[38;2;100;100;100m]{} [\x1b[38;2;200;200;200m+\x1b[38;2;100;100;100m]\x1b[0m",
        y + 1, button_x, zoom_text);

    // Draw side borders
    for row in 1..height - 1 {
        print!("\x1b[{};{}H\x1b[38;2;60;60;60m│\x1b[0m", y + row + 1, x + 1);
        print!("\x1b[{};{}H\x1b[38;2;60;60;60m│\x1b[0m", y + row + 1, x + width);
    }

    // Draw bottom border
    print!("\x1b[{};{}H\x1b[38;2;60;60;60m╰{}╯\x1b[0m",
        y + height, x + 1, "─".repeat((width - 2) as usize));

    // Render PDF content inside the container (with 1-cell padding for borders)
    if let Some(image) = &app.current_page_image {
        let content_x = x + 2;
        let content_y = y + 1;
        let content_width = width.saturating_sub(4);  // -2 for borders, -2 for scrollbar
        let content_height = height.saturating_sub(3); // -2 for borders, -1 for scrollbar

        // Apply zoom to PDF display
        let _ = viuer_display::display_pdf_viewport(
            image, content_x, content_y, content_width, content_height,
            app.pdf_scroll_x, app.pdf_scroll_y, app.dark_mode
        );

        // Draw scrollbars inside the container
        if app.pdf_full_width > content_width {
            let scrollbar_y = y + height - 2;
            let thumb_width = ((content_width as f32 / app.pdf_full_width as f32) * content_width as f32) as u16;
            let thumb_pos = ((app.pdf_scroll_x as f32 / (app.pdf_full_width - content_width) as f32) * (content_width - thumb_width) as f32) as u16;

            print!("\x1b[{};{}H\x1b[38;2;40;40;40m{} \x1b[0m",
                scrollbar_y, content_x, "─".repeat(content_width as usize));
            print!("\x1b[{};{}H\x1b[38;2;100;100;100m{}\x1b[0m",
                scrollbar_y, content_x + thumb_pos, "═".repeat(thumb_width.max(2) as usize));
        }

        if app.pdf_full_height > content_height {
            let scrollbar_x = x + width - 2;
            let thumb_height = ((content_height as f32 / app.pdf_full_height as f32) * content_height as f32) as u16;
            let thumb_pos = ((app.pdf_scroll_y as f32 / (app.pdf_full_height - content_height) as f32) * (content_height - thumb_height) as f32) as u16;

            for row in 0..content_height {
                if row >= thumb_pos && row < thumb_pos + thumb_height {
                    print!("\x1b[{};{}H\x1b[38;2;100;100;100m║\x1b[0m", y + row + 2, scrollbar_x);
                } else {
                    print!("\x1b[{};{}H\x1b[38;2;40;40;40m│\x1b[0m", y + row + 2, scrollbar_x);
                }
            }
        }
    }

    Ok(())
}

/// Render the notes pane (left side in notes mode)
fn render_notes_pane(app: &mut App, x: u16, y: u16, width: u16, height: u16) -> Result<()> {
    // Draw container border - highlight if active
    let border_color = if app.active_pane == ActivePane::Left {
        "\x1b[38;2;100;150;255m"  // Bright blue when active
    } else {
        "\x1b[38;2;60;60;60m"     // Dim gray when inactive
    };
    print!("{}╭{}╮\x1b[0m",
        format!("\x1b[{};{}H{}", y + 1, x + 1, border_color),
        "─".repeat((width - 2) as usize));

    // Draw label for notes pane
    let label_text = " Notes - Ctrl+E for PDF ";
    print!("\x1b[{};{}H\x1b[38;2;150;150;150m{}\x1b[0m", y + 1, x + 2, label_text);

    // Draw side borders
    for row in 1..height - 1 {
        print!("\x1b[{};{}H{}│\x1b[0m", y + row + 1, x + 1, border_color);
        print!("\x1b[{};{}H{}│\x1b[0m", y + row + 1, x + width, border_color);
    }

    // Draw bottom border
    print!("\x1b[{};{}H{}╰{}╯\x1b[0m",
        y + height, x + 1, border_color, "─".repeat((width - 2) as usize));

    // Render notes content inside the container
    // Create notes renderer if needed
    if app.notes_display.is_none() {
        let mut renderer = EditPanelRenderer::new(width, height);
        renderer.update_from_rope(&app.notes_rope);
        app.notes_display = Some(renderer);
    }

    if let Some(renderer) = &mut app.notes_display {
        renderer.update_from_rope(&app.notes_rope);

        // Only show cursor if this pane is active
        let show_cursor = app.active_pane == ActivePane::Left;

        let cursor_pos = app.notes_selection.primary().head;
        let cursor_line = app.notes_rope.char_to_line(cursor_pos);
        let line_start = app.notes_rope.line_to_char(cursor_line);
        let cursor_col = cursor_pos - line_start;

        renderer.follow_cursor(cursor_col, cursor_line, 3);

        let viewport_relative_cursor = (cursor_col, cursor_line);

        let content_x = x + 2;
        let content_y = y + 1;
        let display_width = width.saturating_sub(2);
        let display_height = height.saturating_sub(2);

        // Render notes with standard selection
        let (sel_start, sel_end) = if app.notes_selection.primary().len() > 0 && show_cursor {
            let range = app.notes_selection.primary();
            let start_line = app.notes_rope.char_to_line(range.from());
            let end_line = app.notes_rope.char_to_line(range.to());
            let start_line_char = app.notes_rope.line_to_char(start_line);
            let end_line_char = app.notes_rope.line_to_char(end_line);
            (
                Some((range.from() - start_line_char, start_line)),
                Some((range.to() - end_line_char, end_line))
            )
        } else {
            (None, None)
        };

        renderer.render_with_cursor_and_selection(
            content_x, content_y + 1, display_width, display_height - 1,
            viewport_relative_cursor,
            sel_start,
            sel_end
        )?;
    }

    Ok(())
}

/// Render the text editor pane with zoom controls
fn render_text_pane(app: &mut App, x: u16, y: u16, width: u16, height: u16) -> Result<()> {
    // Draw container border - highlight if active
    let border_color = if app.active_pane == ActivePane::Right {
        "\x1b[38;2;100;150;255m"  // Bright blue when active
    } else {
        "\x1b[38;2;60;60;60m"     // Dim gray when inactive
    };
    print!("{}╭{}╮\x1b[0m",
        format!("\x1b[{};{}H{}", y + 1, x + 1, border_color),
        "─".repeat((width - 2) as usize));

    // Draw extraction method label (always show extraction method since this is always the extraction pane)
    let method_label = match app.extraction_method {
        ExtractionMethod::Segments => "PDFium",
        ExtractionMethod::PdfAlto => "PDFAlto",
        ExtractionMethod::LeptessOCR => "Leptess OCR",
    };
    let label_text = format!(" Extraction: {} ", method_label);
    print!("\x1b[{};{}H\x1b[38;2;150;150;150m{}\x1b[0m", y + 1, x + 2, label_text);

    // Draw side borders
    for row in 1..height - 1 {
        print!("\x1b[{};{}H{}│\x1b[0m", y + row + 1, x + 1, border_color);
        print!("\x1b[{};{}H{}│\x1b[0m", y + row + 1, x + width, border_color);
    }

    // Draw bottom border
    print!("\x1b[{};{}H{}╰{}╯\x1b[0m",
        y + height, x + 1, border_color, "─".repeat((width - 2) as usize));

    // Render extraction text content inside the container (always shows extraction)
    if let Some(renderer) = &mut app.edit_display {
        renderer.update_from_rope(&app.extraction_rope);

        // Only show cursor if this pane is active
        let show_cursor = app.active_pane == ActivePane::Right;

        let cursor_pos = app.extraction_selection.primary().head;
        let cursor_line = app.extraction_rope.char_to_line(cursor_pos);
        let line_start = app.extraction_rope.line_to_char(cursor_line);
        let cursor_col = cursor_pos - line_start;

        renderer.follow_cursor(cursor_col, cursor_line, 3);

        let viewport_relative_cursor = (cursor_col, cursor_line);

        let content_x = x + 2;
        let content_y = y + 1;
        // Use full available space now that scrollbars are removed
        let display_width = width.saturating_sub(2);  // Just borders
        let display_height = height.saturating_sub(2); // Just borders

        // Always use normal rendering - text zoom doesn't work well in terminals
        // The zoom controls remain but just show the status without changing rendering
        if app.block_selection.is_some() {
            renderer.render_with_block_selection(
                content_x, content_y + 1, display_width, display_height - 1,
                viewport_relative_cursor,
                app.block_selection.as_ref()
            )?;
        } else {
            let (sel_start, sel_end) = if app.extraction_selection.primary().len() > 0 && show_cursor {
                let range = app.extraction_selection.primary();
                let start_line = app.extraction_rope.char_to_line(range.from());
                let end_line = app.extraction_rope.char_to_line(range.to());
                let start_line_char = app.extraction_rope.line_to_char(start_line);
                let end_line_char = app.extraction_rope.line_to_char(end_line);
                (
                    Some((range.from() - start_line_char, start_line)),
                    Some((range.to() - end_line_char, end_line))
                )
            } else {
                (None, None)
            };

            renderer.render_with_cursor_and_selection(
                content_x, content_y + 1, display_width, display_height - 1,
                viewport_relative_cursor,
                sel_start,
                sel_end
            )?;
        }

        // Scrollbars removed for cleaner interface
    }

    Ok(())
}


// Status bar function removed - disabled in main rendering loop