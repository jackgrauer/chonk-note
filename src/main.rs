// MINIMAL CHONKER - Just PDF text extraction to editable grid
use anyhow::Result;
// CROSSTERM ELIMINATED! Pure Kitty-native PDF viewer
use kitty_native::KittyTerminal;
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
// mod pdf_renderer; // Removed - using pdfalto instead
mod kitty_file_picker;
mod viuer_display;
mod keyboard;
// mod dual_pane_keyboard; // Removed - consolidated into keyboard.rs
mod kitty_native;
mod mouse;
mod block_selection;
mod notes_database;
mod notes_mode;
mod debug;
mod virtual_grid;
mod grid_cursor;
mod coordinate_system;
mod text_filter;

use edit_renderer::EditPanelRenderer;
use mouse::MouseState;
use block_selection::BlockSelection;
// Theme eliminated - using direct ANSI


#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ExtractionMethod {
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

    // Text editing with Helix-core
    pub extraction_rope: Rope,         // Extracted text from PDF (right pane)
    pub notes_rope: Rope,              // Notes text (left pane when in notes mode)
    pub extraction_selection: Selection,  // Right pane selection state (for Helix operations)
    pub notes_selection: Selection,    // Left pane (notes) selection state (for Helix operations)
    pub extraction_history: History,   // Undo/redo for extraction pane
    pub notes_history: History,        // Undo/redo for notes pane
    pub extraction_block_selection: Option<BlockSelection>,  // Block selection for extraction pane
    pub notes_block_selection: Option<BlockSelection>,  // Block selection for notes pane
    pub active_pane: ActivePane,       // Which pane has focus

    // Grid-based cursors for true grid movement
    pub extraction_grid: virtual_grid::VirtualGrid,    // Grid for extraction pane
    pub notes_grid: virtual_grid::VirtualGrid,         // Grid for notes pane
    pub extraction_cursor: grid_cursor::GridCursor,    // Grid cursor for extraction
    pub notes_cursor: grid_cursor::GridCursor,         // Grid cursor for notes

    // Notes list for sidebar
    pub notes_list: Vec<notes_database::Note>,
    pub selected_note_index: usize,    // Currently selected note in the list
    pub notes_list_scroll: usize,      // Scroll offset for notes list
    pub unsaved_changes: bool,          // Track if current note has unsaved changes
    pub sidebar_expanded: bool,        // Whether sidebar is showing full titles
    pub editing_title: bool,           // Whether user is editing the title
    pub title_buffer: String,          // Buffer for editing title

    // Rendering
    pub edit_display: Option<EditPanelRenderer>,
    pub notes_display: Option<EditPanelRenderer>,  // Separate renderer for notes pane

    // App state (keep unchanged)
    pub status_message: String,
    pub dark_mode: bool,
    pub exit_requested: bool,
    pub needs_redraw: bool,
    pub open_file_picker: bool,
    pub mode_just_changed: bool,  // Track when we switch between PDF and Notes mode


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

    // Store block cut data for paste
    pub block_clipboard: Option<Vec<String>>,  // Stores cut block data

    // Text wrapping toggle
    pub wrap_text: bool,  // Whether to wrap text to viewport width
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
            extraction_method: ExtractionMethod::PdfAlto,
            dual_pane_mode: true,

            // Text editing
            extraction_rope: Rope::from(""),         // Extraction text (right pane)
            notes_rope: Rope::from(""),              // Notes text (left pane in notes mode)
            extraction_selection: Selection::point(0),
            notes_selection: Selection::point(0),
            extraction_history: History::default(),  // Empty history for extraction
            notes_history: History::default(),       // Empty history for notes
            extraction_block_selection: None,        // No block selection initially
            notes_block_selection: None,             // No block selection initially
            active_pane: ActivePane::Right,          // Start with extraction pane active

            // Grid-based cursors
            extraction_grid: virtual_grid::VirtualGrid::new(Rope::from("")),
            notes_grid: virtual_grid::VirtualGrid::new(Rope::from("")),
            extraction_cursor: grid_cursor::GridCursor::new(),
            notes_cursor: grid_cursor::GridCursor::new(),

            // Rendering
            edit_display: None,
            notes_display: None,

            // App state
            status_message: "Ctrl+J/K: Pages | Ctrl+T: Extraction | Ctrl+E: Notes | Ctrl+Q: Quit".to_string(),
            dark_mode: true,
            exit_requested: false,
            needs_redraw: true,
            open_file_picker: false,
            mode_just_changed: false,


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

            // Notes list
            notes_list: Vec::new(),
            selected_note_index: 0,
            notes_list_scroll: 0,
            unsaved_changes: false,
            sidebar_expanded: false,
            editing_title: false,
            title_buffer: String::new(),

            // Block clipboard
            block_clipboard: None,

            // Text wrapping
            wrap_text: false,
        })
    }

    pub fn new_notes_mode() -> Result<Self> {
        let mut notes_mode = notes_mode::NotesMode::new()?;
        let mut notes_list = Vec::new();

        // Load existing notes
        if let Ok(notes) = notes_mode.db.list_notes(100) {
            notes_list = notes;
        }

        // Start with an empty note
        let mut notes_rope = Rope::from("");
        let mut notes_selection = Selection::point(0);
        notes_mode.handle_command(&mut notes_rope, &mut notes_selection, "new")?;

        Ok(Self {
            // Mode
            app_mode: AppMode::NotesEditor,
            notes_mode: Some(notes_mode),

            // PDF-related fields (empty until PDF is loaded)
            pdf_path: PathBuf::new(),  // Empty path, will be set when opening PDF
            current_page: 0,
            total_pages: 0,
            current_page_image: None,
            extraction_method: ExtractionMethod::PdfAlto,
            dual_pane_mode: false,

            // Text editing
            extraction_rope: Rope::from(""),
            notes_rope: notes_rope.clone(),
            extraction_selection: Selection::point(0),
            notes_selection,
            extraction_history: History::default(),
            notes_history: History::default(),
            extraction_block_selection: None,
            notes_block_selection: None,
            active_pane: ActivePane::Left,  // Notes mode starts with left pane active

            // Grid-based cursors
            extraction_grid: virtual_grid::VirtualGrid::new(Rope::from("")),
            notes_grid: virtual_grid::VirtualGrid::new(notes_rope),
            extraction_cursor: grid_cursor::GridCursor::new(),
            notes_cursor: grid_cursor::GridCursor::new(),

            // Rendering
            edit_display: None,
            notes_display: None,

            // App state
            status_message: "Notes Mode (auto-save) - Ctrl+Up/Down: Nav | Ctrl+O: Open | Ctrl+N: New".to_string(),
            dark_mode: true,
            exit_requested: false,
            needs_redraw: true,
            open_file_picker: false,
            mode_just_changed: false,

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

            // Notes list
            notes_list,
            selected_note_index: 0,
            notes_list_scroll: 0,
            unsaved_changes: false,
            sidebar_expanded: false,
            editing_title: false,
            title_buffer: String::new(),

            // Block clipboard
            block_clipboard: None,

            // Text wrapping
            wrap_text: false,
        })
    }

    pub async fn load_pdf_page(&mut self) -> Result<()> {
        use std::process::Command;
        use tempfile::NamedTempFile;

        // Apply zoom to the base size when rendering (reduced by 25% for better text alignment)
        let base_width = 900;  // Was 1200, reduced by 25%
        let base_height = 1200; // Was 1600, reduced by 25%
        let zoomed_width = (base_width as f32 * self.pdf_zoom) as u32;
        let zoomed_height = (base_height as f32 * self.pdf_zoom) as u32;

        // Use pdftoppm to render PDF page to PNG
        let temp_file = NamedTempFile::with_suffix(".png")?;
        let image_path = temp_file.path();

        let page_arg = format!("{}", self.current_page + 1); // pdftoppm uses 1-indexed pages
        let scale = (zoomed_width as f32 / 612.0).max(zoomed_height as f32 / 792.0); // PDF points to pixels
        let scale_arg = format!("{}", scale);

        let output = Command::new("pdftoppm")
            .arg("-png")
            .arg("-f").arg(&page_arg)
            .arg("-l").arg(&page_arg)
            .arg("-scale-to-x").arg(format!("{}", zoomed_width))
            .arg("-scale-to-y").arg(format!("{}", zoomed_height))
            .arg("-singlefile")
            .arg(&self.pdf_path)
            .arg(image_path.with_extension(""))
            .output()?;

        let image = if output.status.success() {
            // Load the rendered image
            image::open(image_path).unwrap_or_else(|_| image::DynamicImage::new_rgb8(zoomed_width, zoomed_height))
        } else {
            // Fallback to placeholder if pdftoppm fails
            image::DynamicImage::new_rgb8(zoomed_width, zoomed_height)
        };

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

        // Filter out any ANSI codes from extracted text
        let cleaned_text = text_filter::clean_text_for_insertion(&text);
        self.extraction_rope = Rope::from_str(&cleaned_text);
        self.extraction_selection = Selection::point(0);  // Reset cursor to top-left

        // Update the extraction grid with the new rope
        self.extraction_grid = virtual_grid::VirtualGrid::new(self.extraction_rope.clone());
        self.extraction_cursor = grid_cursor::GridCursor::new();  // Reset cursor to 0,0

        // Update renderer from rope and reset viewport to top-left
        if let Some(renderer) = &mut self.edit_display {
            renderer.update_from_rope_with_wrap(&self.extraction_rope, self.wrap_text);
            // Reset viewport to show top-left of extracted text
            renderer.scroll_x = 0;
            renderer.scroll_y = 0;
            renderer.viewport_x = 0;
            renderer.viewport_y = 0;
        } else {
            let mut renderer = EditPanelRenderer::new(text_width, text_height);
            renderer.update_from_rope_with_wrap(&self.extraction_rope, self.wrap_text);
            self.edit_display = Some(renderer);
        }

        let method_name = match self.extraction_method {
            ExtractionMethod::PdfAlto => "PDFAlto",
            ExtractionMethod::LeptessOCR => "OCR",
        };
        self.status_message = format!("Extracted with {} method", method_name);
        Ok(())
    }

    pub fn next_page(&mut self) {
        if self.total_pages > 0 && self.current_page < self.total_pages - 1 {
            let _ = viuer_display::clear_graphics();
            self.current_page += 1;
            // Clear extraction rope and reset
            self.extraction_rope = Rope::from("");
            self.extraction_selection = Selection::point(0);
            self.edit_display = None;
            self.current_page_image = None;
            self.needs_redraw = true;
        }
    }

    pub fn prev_page(&mut self) {
        if self.current_page > 0 {
            let _ = viuer_display::clear_graphics();
            self.current_page -= 1;
            // Clear extraction rope and reset
            self.extraction_rope = Rope::from("");
            self.extraction_selection = Selection::point(0);
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
    // Switch active pane
    pub fn switch_active_pane(&mut self, pane: ActivePane) {
        self.active_pane = pane;
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

                // Force a screen clear on next redraw since we're changing layouts
                self.mode_just_changed = true;
                self.needs_redraw = true;

                // Load notes list for sidebar
                if let Some(ref mut notes_mode) = self.notes_mode {
                    // Get all notes from database
                    if let Ok(notes) = notes_mode.db.list_notes(100) {
                        self.notes_list = notes;
                    }

                    // If no notes exist, create two sample notes
                    if self.notes_list.is_empty() {
                        // Create first sample note
                        let note1 = notes_mode.db.create_note(
                            "Meeting Notes".to_string(),
                            "# Meeting Notes\n\nToday's action items:\n- Review the PDF extraction code\n- Implement auto-save feature\n- Test the notes system\n\nTags: work, todo".to_string(),
                            vec!["work".to_string(), "todo".to_string()]
                        )?;

                        // Create second sample note
                        let note2 = notes_mode.db.create_note(
                            "Project Ideas".to_string(),
                            "# Project Ideas\n\nPotential improvements:\n1. Add markdown support\n2. Implement search functionality\n3. Add export to PDF feature\n4. Create themes system\n\nTags: ideas, development".to_string(),
                            vec!["ideas".to_string(), "development".to_string()]
                        )?;

                        // Add to notes list
                        self.notes_list.push(note1);
                        self.notes_list.push(note2);

                        // Load the first note
                        if !self.notes_list.is_empty() {
                            let first_note = &self.notes_list[0];
                            self.notes_rope = Rope::from(first_note.content.as_str());
                            self.notes_selection = Selection::point(0);
                            notes_mode.current_note = Some(first_note.clone());
                            self.selected_note_index = 0;
                        }
                    } else if !self.notes_list.is_empty() {
                        // Load the first existing note
                        let first_note = &self.notes_list[0];
                        self.notes_rope = Rope::from_str(&first_note.content);
                        self.notes_selection = Selection::point(0);
                        notes_mode.current_note = Some(first_note.clone());
                        self.selected_note_index = 0;
                    }
                }

                // Notes are already in notes_rope, no need to copy
                self.status_message = "Notes Mode (auto-save) - Ctrl+Up/Down: Nav | Ctrl+O: Open | Ctrl+N: New | Ctrl+E: Back".to_string();

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

                // Notes are already saved in notes_rope

                // Switch back to PDF mode
                self.app_mode = AppMode::PdfViewer;
                self.dual_pane_mode = true;
                self.status_message = "PDF Mode - Ctrl+J/K: Pages | Ctrl+T: Extraction | Ctrl+E: Notes".to_string();

                // Extraction text is already in extraction_rope
                self.mode_just_changed = true;
                self.needs_redraw = true;
            }
        }
        Ok(())
    }

    // Toggle between extraction methods
    pub async fn toggle_extraction_method(&mut self) -> Result<()> {
        self.extraction_method = match self.extraction_method {
            ExtractionMethod::PdfAlto => ExtractionMethod::LeptessOCR,
            ExtractionMethod::LeptessOCR => ExtractionMethod::PdfAlto,
        };

        // Show immediate feedback before processing
        let method_name = match self.extraction_method {
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
                // Filter out any ANSI codes from extracted text
        let cleaned_text = text_filter::clean_text_for_insertion(&text);
        self.extraction_rope = Rope::from_str(&cleaned_text);
                self.extraction_selection = Selection::point(0);  // Reset cursor to top-left
                // Update renderer from rope and reset viewport
                if let Some(renderer) = &mut self.edit_display {
                    renderer.update_from_rope_with_wrap(&self.extraction_rope, self.wrap_text);
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

            // BEGIN SYNCHRONIZED UPDATE - prevents flicker by batching all drawing
            print!("\x1b[?2026h");

            // Clear screen ONLY when switching between modes (PDF ↔ Notes)
            // This prevents artifacts from the old layout remaining visible
            if app.mode_just_changed {
                print!("\x1b[2J");
                app.mode_just_changed = false;
            }

            // Save cursor position
            print!("\x1b[s");

            // Render based on mode with notes list sidebar in Notes mode
            if app.app_mode == AppMode::NotesEditor {
                // In notes mode, show two panes: notes list | notes editor (no extraction pane)
                // Sidebar width depends on whether it's expanded
                let notes_list_width = if app.sidebar_expanded { 30 } else { 4 };
                let remaining_width = term_width.saturating_sub(notes_list_width);

                // Render notes list sidebar on far left
                render_notes_list(&app, 0, 0, notes_list_width, term_height)?;

                // Render notes editor - use all remaining width
                let notes_start_x = notes_list_width;
                render_notes_pane(&mut *app, notes_start_x, 0, remaining_width, term_height)?;
            } else {
                // In PDF mode, render PDF on left
                render_pdf_pane(app, 0, 0, split_x, term_height)?;

                // Render visible divider column
                render_divider(split_x, term_height)?;

                // Extraction text starts after divider
                let extraction_start = split_x + 1;
                if extraction_start < term_width {
                    render_text_pane(&mut *app, extraction_start, 0, term_width - extraction_start, term_height)?;
                }
            }

            // Restore cursor position
            print!("\x1b[u");

            // END SYNCHRONIZED UPDATE - now display everything at once
            print!("\x1b[?2026l");
            stdout.flush()?;


            // Status bar disabled to prevent debug flood
            // render_status_bar(&mut stdout, app, term_width, term_height)?;

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

                        // HELIX-CORE: Track selection changes
                        let old_selection = app.extraction_selection.clone();

                        if !keyboard::handle_input(app, key).await? {
                            break;
                        }
                        if app.exit_requested {
                            break;
                        }

                        // HELIX-CORE: Check for changes
                        let selection_changed = app.extraction_selection != old_selection;

                        if selection_changed {
                            // Any selection change triggers redraw
                            app.needs_redraw = true;
                        }
                    }
                    kitty_native::InputEvent::Mouse(mouse_event) => {
                        // Handle mouse events
                        mouse::handle_mouse(app, mouse_event, &mut mouse_state).await?;
                    }
                }
            }
        }
    }
    
    Ok(())
}

/// Render the PDF pane (no borders)
fn render_pdf_pane(app: &mut App, x: u16, y: u16, width: u16, height: u16) -> Result<()> {
    // Draw zoom controls in top right corner of PDF pane (no border)
    let zoom_text = format!(" {:.0}% ", app.pdf_zoom * 100.0);
    let button_x = x + width - 12;
    print!("\x1b[{};{}H\x1b[38;2;100;100;100m[\x1b[38;2;200;200;200m-\x1b[38;2;100;100;100m]{} [\x1b[38;2;200;200;200m+\x1b[38;2;100;100;100m]\x1b[0m",
        y, button_x, zoom_text);

    // Render PDF content - use full space now (no borders)
    if let Some(image) = &app.current_page_image {
        let content_x = x;
        let content_y = y;
        let content_width = width;
        let content_height = height;

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

/// Render the notes pane (no borders)
fn render_notes_pane(app: &mut App, x: u16, y: u16, width: u16, height: u16) -> Result<()> {
    // Render title bar at the top
    let title_height = 1;
    let title = if app.editing_title {
        // Show the buffer being edited with a cursor
        format!("{}|", app.title_buffer)
    } else if !app.notes_list.is_empty() && app.selected_note_index < app.notes_list.len() {
        app.notes_list[app.selected_note_index].title.clone()
    } else {
        "Untitled".to_string()
    };

    // Draw title bar with bold text and amber background
    print!("\x1b[{};{}H\x1b[48;2;255;193;7m\x1b[38;2;0;0;0m\x1b[1m{}{}\x1b[0m",
        y + 1, x + 1,
        title,
        " ".repeat((width as usize).saturating_sub(title.len())));

    // Adjust content area to account for title bar
    let content_y = y + title_height;
    let content_height = height.saturating_sub(title_height);

    // Create notes renderer if needed
    if app.notes_display.is_none() {
        let mut renderer = EditPanelRenderer::new(width, content_height);
        renderer.update_from_rope_with_wrap(&app.notes_rope, app.wrap_text);
        app.notes_display = Some(renderer);
    }

    if let Some(renderer) = &mut app.notes_display {
        // Always resize to current width/height in case it changed
        renderer.resize(width, content_height);
        renderer.update_from_rope_with_wrap(&app.notes_rope, app.wrap_text);

        // Only show cursor if this pane is active
        let show_cursor = app.active_pane == ActivePane::Left;

        // Use grid cursor position (can be in virtual space!)
        let cursor_line = app.notes_cursor.row;
        let cursor_col = app.notes_cursor.col;

        renderer.follow_cursor(cursor_col, cursor_line, 3);

        // The cursor position for rendering should be absolute (in buffer coordinates)
        // Not relative to viewport
        let absolute_cursor = (cursor_col, cursor_line);

        // Use full space - no padding
        let content_x = x;
        let display_width = width;
        let display_height = content_height;

        // Render notes with block selection support
        if app.notes_block_selection.is_some() {
            renderer.render_with_block_selection(
                content_x, content_y, display_width, display_height,
                absolute_cursor,
                app.notes_block_selection.as_ref(),
                show_cursor
            )?;
        } else {
            let (sel_start, sel_end) = if show_cursor {
                let range = app.notes_selection.primary();
                // Only show selection highlighting if it's not collapsed to a point
                if range.from() != range.to() {
                    let start_line = app.notes_rope.char_to_line(range.from());
                    let end_line = app.notes_rope.char_to_line(range.to().saturating_sub(1).max(0));
                    let start_line_char = app.notes_rope.line_to_char(start_line);
                    let end_line_char = app.notes_rope.line_to_char(end_line);

                    // Safety: ensure we don't underflow
                    let start_col = range.from().saturating_sub(start_line_char);
                    let end_col = range.to().saturating_sub(end_line_char);

                    (
                        Some((start_col, start_line)),
                        Some((end_col, end_line))
                    )
                } else {
                    (None, None)
                }
            } else {
                (None, None)
            };

            renderer.render_with_cursor_and_selection(
                content_x, content_y, display_width, display_height,
                absolute_cursor,
                sel_start,
                sel_end,
                show_cursor
            )?;
        }
    }

    Ok(())
}

/// Render the text editor pane (no borders)
fn render_text_pane(app: &mut App, x: u16, y: u16, width: u16, height: u16) -> Result<()> {
    // No borders - use full space
    // Render extraction text content (always shows extraction)
    if let Some(renderer) = &mut app.edit_display {
        renderer.update_from_rope_with_wrap(&app.extraction_rope, app.wrap_text);

        // Only show cursor if this pane is active
        let show_cursor = app.active_pane == ActivePane::Right;

        // Use grid cursor position (can be in virtual space!)
        let cursor_line = app.extraction_cursor.row;
        let cursor_col = app.extraction_cursor.col;

        renderer.follow_cursor(cursor_col, cursor_line, 3);

        // The cursor position for rendering should be absolute (in buffer coordinates)
        // Not relative to viewport
        let absolute_cursor = (cursor_col, cursor_line);

        // Use full space - no padding
        let content_x = x;
        let content_y = y;
        let display_width = width;
        let display_height = height;

        // Always use normal rendering - text zoom doesn't work well in terminals
        // The zoom controls remain but just show the status without changing rendering
        if app.extraction_block_selection.is_some() {
            renderer.render_with_block_selection(
                content_x, content_y, display_width, display_height,
                absolute_cursor,
                app.extraction_block_selection.as_ref(),
                show_cursor
            )?;
        } else {
            let (sel_start, sel_end) = if show_cursor {
                let range = app.extraction_selection.primary();
                // Only show selection highlighting if it's not collapsed to a point
                if range.from() != range.to() {
                    let start_line = app.extraction_rope.char_to_line(range.from());
                    let end_line = app.extraction_rope.char_to_line(range.to().saturating_sub(1).max(0));
                    let start_line_char = app.extraction_rope.line_to_char(start_line);
                    let end_line_char = app.extraction_rope.line_to_char(end_line);

                    // Safety: ensure we don't underflow
                    let start_col = range.from().saturating_sub(start_line_char);
                    let end_col = range.to().saturating_sub(end_line_char);

                    (
                        Some((start_col, start_line)),
                        Some((end_col, end_line))
                    )
                } else {
                    (None, None)
                }
            } else {
                (None, None)
            };

            renderer.render_with_cursor_and_selection(
                content_x, content_y, display_width, display_height,
                absolute_cursor,
                sel_start,
                sel_end,
                show_cursor
            )?;
        }

        // Scrollbars removed for cleaner interface
    }

    Ok(())
}

/// Render the minimal notes list sidebar (just numbers)
fn render_notes_list(app: &App, x: u16, y: u16, width: u16, height: u16) -> Result<()> {
    // No borders for minimal design - just a subtle divider line is drawn separately

    // Clear all lines in the notes list area first with bright blue background
    for row in 0..height {
        print!("\x1b[{};{}H\x1b[48;2;30;60;100m{}\x1b[0m", y + row + 1, x + 1, " ".repeat(width as usize));
    }

    // Show notes as simple numbers
    if app.notes_list.is_empty() {
        // Show + for new note
        print!("\x1b[{};{}H\x1b[48;2;30;60;100m\x1b[38;2;200;200;200m +\x1b[0m", y + 2, x + 1);
    } else {
        // Display notes as numbers with scrolling support
        let visible_count = (height - 2) as usize;

        // Use the scroll offset from app
        let start_index = app.notes_list_scroll;
        let end_index = (start_index + visible_count).min(app.notes_list.len());

        for (display_pos, note_idx) in (start_index..end_index).enumerate() {
            let is_selected = note_idx == app.selected_note_index;
            let note = &app.notes_list[note_idx];

            // Highlight selected note with Material Design colors
            let (bg_color, text_color) = if is_selected {
                ("\x1b[48;2;255;193;7m", "\x1b[38;2;0;0;0m")  // Material amber with black text
            } else {
                ("\x1b[48;2;30;60;100m", "\x1b[38;2;220;220;220m")  // Bright blue background with light grey text
            };

            if app.sidebar_expanded {
                // Show full title (truncated to fit width), or buffer if editing this note
                let title = if app.editing_title && is_selected {
                    format!("{}|", app.title_buffer)
                } else if note.title.is_empty() {
                    "Untitled".to_string()
                } else {
                    note.title.clone()
                };
                let max_title_len = (width as usize).saturating_sub(4);
                let display_title: String = if title.len() > max_title_len {
                    format!("{}…", &title[..max_title_len.saturating_sub(1)])
                } else {
                    title
                };

                // Make title bold for emphasis
                print!("\x1b[{};{}H{}\x1b[1m{} {}\x1b[0m",
                    y + display_pos as u16 + 1, x + 1,
                    bg_color, text_color, display_title);
            } else {
                // Show note number (1-indexed for user friendliness)
                let note_num = note_idx + 1;

                // Add indicator: > for selected
                let indicator = if is_selected { "> " } else { "  " };

                // Draw the indicator and note number
                print!("\x1b[{};{}H{}{}{}{}\x1b[0m",
                    y + display_pos as u16 + 1, x,
                    bg_color, text_color, indicator, note_num);
            }
        }

        // Show scroll indicators if needed
        if start_index > 0 {
            // Show up arrow at top (Material green)
            print!("\x1b[{};{}H\x1b[48;2;30;60;100m\x1b[38;2;76;175;80m↑\x1b[0m", y, x + 2);
        }
        if end_index < app.notes_list.len() {
            // Show down arrow at bottom (Material green)
            print!("\x1b[{};{}H\x1b[48;2;30;60;100m\x1b[38;2;76;175;80m↓\x1b[0m", y + height - 1, x + 2);
        }
    }

    Ok(())
}

/// Render a visible divider column between panes
fn render_divider(x: u16, height: u16) -> Result<()> {
    // Draw a gray column with a resize handle in the middle
    for row in 0..height {
        print!("\x1b[{};{}H\x1b[K", row + 1, x + 1); // +1 because terminal rows are 1-based, clear line

        if row == height / 2 {
            // Resize handle in the middle - brighter
            print!("\x1b[48;2;80;80;80m \x1b[0m");
        } else if row >= height / 2 - 1 && row <= height / 2 + 1 {
            // Area around handle - medium gray
            print!("\x1b[48;2;60;60;60m \x1b[0m");
        } else {
            // Rest of divider - darker gray
            print!("\x1b[48;2;40;40;40m \x1b[0m");
        }
    }
    Ok(())
}

// Status bar function removed - disabled in main rendering loop