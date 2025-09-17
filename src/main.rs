// MINIMAL CHONKER - Just PDF text extraction to editable grid
use anyhow::Result;
// CROSSTERM ELIMINATED! Pure Kitty-native PDF viewer
use kitty_native::KittyTerminal;
// All crossterm eliminated - pure Kitty ANSI
use std::{io::{self, Write}, path::PathBuf, time::{Duration, Instant}};
use image::DynamicImage;
use clap::Parser;

// HELIX-CORE INTEGRATION! Professional text editing
use helix_core::{
    Rope, Selection, Transaction,
    history::{History, State},  // Need State for history
};

mod content_extractor;
mod edit_renderer;
mod pdf_renderer;
// mod file_picker;          // ELIMINATED - using kitty_file_picker
mod kitty_file_picker;    // Kitty-native replacement
mod theme;
mod viuer_display;
mod keyboard;
mod kitty_native;

use edit_renderer::EditPanelRenderer;
// Theme eliminated - using direct ANSI

#[cfg(target_os = "macos")]
const MOD_KEY: kitty_native::KeyModifiers = kitty_native::KeyModifiers::SUPER;
#[cfg(not(target_os = "macos"))]
const MOD_KEY: kitty_native::KeyModifiers = kitty_native::KeyModifiers::CONTROL;

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
    // PDF-related fields (keep unchanged)
    pub pdf_path: PathBuf,
    pub current_page: usize,
    pub total_pages: usize,
    pub current_page_image: Option<DynamicImage>,
    pub extraction_method: ExtractionMethod,
    pub dual_pane_mode: bool,

    // HELIX-CORE INTEGRATION! Professional text editing
    pub rope: Rope,                    // Text buffer (replaces edit_data)
    pub selection: Selection,          // Cursor + selections (replaces all cursor fields)
    pub history: History,              // Undo/redo for free!

    // Rendering
    pub edit_display: Option<EditPanelRenderer>,

    // App state (keep unchanged)
    pub status_message: String,
    pub dark_mode: bool,
    pub exit_requested: bool,
    pub needs_redraw: bool,
    pub open_file_picker: bool,

    // Cursor acceleration eliminated - helix-core handles movement better

    // Viewport tracking for flicker prevention
    pub last_viewport_scroll: (u16, u16),
}

impl App {
    pub fn new(pdf_path: PathBuf, start_page: usize) -> Result<Self> {
        let total_pages = content_extractor::get_page_count(&pdf_path)?;
        Ok(Self {
            // PDF-related fields
            pdf_path,
            current_page: start_page.saturating_sub(1),
            total_pages,
            current_page_image: None,
            extraction_method: ExtractionMethod::Segments,
            dual_pane_mode: true,

            // HELIX-CORE INTEGRATION!
            rope: Rope::from(""),                    // Empty rope initially
            selection: Selection::point(0),          // Cursor at position 0
            history: History::default(),             // Empty history

            // Rendering
            edit_display: None,

            // App state
            status_message: String::new(),
            dark_mode: true,
            exit_requested: false,
            needs_redraw: true,
            open_file_picker: false,

            // Cursor acceleration eliminated

            // Viewport tracking
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

        self.rope = Rope::from_str(&text);
        self.selection = Selection::point(0);  // Reset cursor

        // Update renderer from rope
        if let Some(renderer) = &mut self.edit_display {
            renderer.update_from_rope(&self.rope);
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
            // HELIX-CORE: Clear rope and reset
            self.rope = Rope::from("");
            self.selection = Selection::point(0);
            self.edit_display = None;
            self.current_page_image = None;
            self.needs_redraw = true;
        }
    }

    pub fn prev_page(&mut self) {
        if self.current_page > 0 {
            let _ = viuer_display::clear_graphics();
            self.current_page -= 1;
            // HELIX-CORE: Clear rope and reset
            self.rope = Rope::from("");
            self.selection = Selection::point(0);
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
                self.rope = Rope::from_str(&text);
                // HELIX-CORE: Update renderer from rope
                if let Some(renderer) = &mut self.edit_display {
                    renderer.update_from_rope(&self.rope);
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
        // Try to find a default PDF in Documents
        if let Some(path) = find_default_pdf() {
            path
        } else {
            // Fallback to file picker if no default found
            if let Some(path) = kitty_file_picker::pick_pdf_file()? {
                path
            } else {
                eprintln!("No PDF file specified and none found");
                std::process::exit(1);
            }
        }
    };

    let mut app = App::new(pdf_path, args.page)?;
    app.load_pdf_page().await?;
    
    setup_terminal()?;
    let result = run_app(&mut app).await;
    restore_terminal()?;
    
    result
}

fn find_default_pdf() -> Option<PathBuf> {
    let docs_path = PathBuf::from("/Users/jack/Documents");
    if let Ok(entries) = std::fs::read_dir(&docs_path) {
        for entry in entries.flatten() {
            if let Some(ext) = entry.path().extension() {
                if ext.to_string_lossy().to_lowercase() == "pdf" {
                    return Some(entry.path());
                }
            }
        }
    }
    None
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

    // Initial render
    app.needs_redraw = true;
    
    loop {
        let (term_width, term_height) = KittyTerminal::size()?;
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
            
            if let Some(new_path) = kitty_file_picker::pick_pdf_file()? {
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
            // CROSSTERM ELIMINATED! Direct Kitty positioning
            KittyTerminal::move_to(0, 0)?;
            last_render_time = now;

            // Always dual pane mode: PDF on left, text editor on right
            if let Some(image) = &app.current_page_image {
                let _ = viuer_display::display_pdf_image(
                    image, 0, 0, split_x - 1, term_height - 2, app.dark_mode
                );
            }

            // Render text editor on right
            if let Some(renderer) = &app.edit_display {
                // HELIX-CORE: Convert selection to old format for renderer
                let cursor_pos = app.selection.primary().head;
                let cursor_line = app.rope.byte_to_line(cursor_pos);
                let line_start = app.rope.line_to_byte(cursor_line);
                let cursor = (cursor_pos - line_start, cursor_line);

                // Handle selection
                let (sel_start, sel_end) = if app.selection.len() > 1 {
                    let range = app.selection.primary();
                    let start_line = app.rope.byte_to_line(range.from());
                    let end_line = app.rope.byte_to_line(range.to());
                    let start_line_byte = app.rope.line_to_byte(start_line);
                    let end_line_byte = app.rope.line_to_byte(end_line);
                    (
                        Some((range.from() - start_line_byte, start_line)),
                        Some((range.to() - end_line_byte, end_line))
                    )
                } else {
                    (None, None)
                };

                renderer.render_with_cursor_and_selection(
                    split_x, 0, term_width - split_x, term_height - 2,
                    cursor,
                    sel_start,
                    sel_end
                )?;
            }

            // Status bar disabled to prevent debug flood
            // render_status_bar(&mut stdout, app, term_width, term_height)?;

            stdout.flush()?;
            app.needs_redraw = false;
        }
        
        // CROSSTERM ELIMINATED! Direct Kitty input
        if KittyTerminal::poll_input()? {
            if let Some(key) = KittyTerminal::read_key()? {
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
        }
    }
    
    Ok(())
}

// Status bar function removed - disabled in main rendering loop