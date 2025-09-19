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
mod kitty_native;
mod mouse;
mod block_selection;

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
    pub block_selection: Option<BlockSelection>,  // Proper block selection with visual columns
    pub virtual_cursor_col: Option<usize>,  // Virtual cursor column for navigating past line ends

    // Rendering
    pub edit_display: Option<EditPanelRenderer>,

    // App state (keep unchanged)
    pub status_message: String,
    pub dark_mode: bool,
    pub exit_requested: bool,
    pub needs_redraw: bool,
    pub open_file_picker: bool,

    // Cursor acceleration for arrow keys
    pub last_arrow_key: Option<KeyCode>,
    pub arrow_key_count: usize,
    pub last_arrow_time: Option<std::time::Instant>,

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
            rope: Rope::from(""),                    // Empty rope initially
            selection: Selection::point(0),          // Cursor at position 0
            history: History::default(),             // Empty history
            block_selection: None,                   // No block selection initially
            virtual_cursor_col: None,                // No virtual cursor position initially

            // Rendering
            edit_display: None,

            // App state
            status_message: String::new(),
            dark_mode: true,
            exit_requested: false,
            needs_redraw: true,
            open_file_picker: false,

            // Cursor acceleration
            last_arrow_key: None,
            arrow_key_count: 0,
            last_arrow_time: None,

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
        })
    }

    pub async fn load_pdf_page(&mut self) -> Result<()> {
        // Render page at full size (not limited by viewport)
        let image = pdf_renderer::render_pdf_page(&self.pdf_path, self.current_page, 1200, 1600)?;

        // Update PDF dimensions
        self.pdf_full_width = image.width() as u16;
        self.pdf_full_height = image.height() as u16;

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
        // Show file picker when no PDF specified
        if let Some(path) = kitty_file_picker::pick_pdf_file()? {
            path
        } else {
            eprintln!("No PDF file selected");
            std::process::exit(0);  // Exit cleanly if user cancels
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

            // Clear the entire screen to prevent artifacts when resizing
            // More efficient than selective clearing and ensures no remnants
            print!("\x1b[2J");

            // Render PDF with viewport and scrollbars
            if let Some(image) = &app.current_page_image {
                // Calculate viewport dimensions (leave room for scrollbars)
                let pdf_viewport_width = split_x.saturating_sub(3);  // -2 for divider, -1 for scrollbar
                let pdf_viewport_height = term_height.saturating_sub(3); // -2 for borders, -1 for scrollbar

                // Display only the visible portion of the PDF
                let _ = viuer_display::display_pdf_image(
                    image, 0, 0, pdf_viewport_width, pdf_viewport_height, app.dark_mode
                );

                // Draw horizontal scrollbar if needed
                if app.pdf_full_width > pdf_viewport_width {
                    let scrollbar_y = pdf_viewport_height + 1;
                    let thumb_width = ((pdf_viewport_width as f32 / app.pdf_full_width as f32) * pdf_viewport_width as f32) as u16;
                    let thumb_pos = ((app.pdf_scroll_x as f32 / (app.pdf_full_width - pdf_viewport_width) as f32) * (pdf_viewport_width - thumb_width) as f32) as u16;

                    // Draw scrollbar track
                    print!("\x1b[{};1H\x1b[38;2;40;40;40m{}\x1b[0m",
                        scrollbar_y, "─".repeat(pdf_viewport_width as usize));
                    // Draw scrollbar thumb
                    print!("\x1b[{};{}H\x1b[38;2;100;100;100m{}\x1b[0m",
                        scrollbar_y, thumb_pos + 1, "═".repeat(thumb_width.max(2) as usize));
                }

                // Draw vertical scrollbar if needed
                if app.pdf_full_height > pdf_viewport_height {
                    let scrollbar_x = pdf_viewport_width + 1;
                    let thumb_height = ((pdf_viewport_height as f32 / app.pdf_full_height as f32) * pdf_viewport_height as f32) as u16;
                    let thumb_pos = ((app.pdf_scroll_y as f32 / (app.pdf_full_height - pdf_viewport_height) as f32) * (pdf_viewport_height - thumb_height) as f32) as u16;

                    // Draw scrollbar track and thumb
                    for y in 0..pdf_viewport_height {
                        if y >= thumb_pos && y < thumb_pos + thumb_height {
                            print!("\x1b[{};{}H\x1b[38;2;100;100;100m║\x1b[0m", y + 1, scrollbar_x);
                        } else {
                            print!("\x1b[{};{}H\x1b[38;2;40;40;40m│\x1b[0m", y + 1, scrollbar_x);
                        }
                    }
                }
            }

            // Draw divider line between panes
            {
                let divider_color = if app.is_dragging_divider {
                    "\x1b[38;2;100;150;255m"  // Bright blue when dragging
                } else {
                    "\x1b[38;2;60;60;60m"     // Dark gray normally
                };

                for y in 0..term_height {
                    print!("\x1b[{};{}H{}│\x1b[0m",
                        y + 1, split_x, divider_color);
                }
            }

            // Render text editor on right
            if let Some(renderer) = &mut app.edit_display {
                // HELIX-CORE: Convert selection to old format for renderer
                let cursor_pos = app.selection.primary().head;
                let cursor_line = app.rope.char_to_line(cursor_pos);
                let line_start = app.rope.line_to_char(cursor_line);
                let actual_col = cursor_pos - line_start;

                // Use virtual cursor column if set (for positioning past line end)
                let cursor_col = if let Some(vc) = app.virtual_cursor_col {
                    vc
                } else {
                    actual_col
                };

                // AUTO-SCROLL: Make viewport follow cursor with 3-line padding
                renderer.follow_cursor(cursor_col, cursor_line, 3);

                // IMPORTANT: Adjust cursor position for viewport offset
                // The renderer expects viewport-relative coordinates, not absolute document coordinates
                let viewport_relative_cursor = (cursor_col, cursor_line);

                // Get the extraction method name for the label
                let method_label = match app.extraction_method {
                    ExtractionMethod::Segments => "PDFium",
                    ExtractionMethod::PdfAlto => "PDFAlto",
                    ExtractionMethod::LeptessOCR => "Leptess OCR",
                };

                // Calculate text viewport dimensions (leave room for scrollbars)
                let text_viewport_width = term_width.saturating_sub(split_x + 2); // -1 for divider, -1 for scrollbar
                let text_viewport_height = term_height.saturating_sub(4); // -1 for label, -2 for borders, -1 for scrollbar

                // Check if we have block selection mode active
                if app.block_selection.is_some() {
                    // First render the label
                    renderer.render_with_label(
                        split_x + 1, 0, text_viewport_width, 1, Some(method_label)
                    )?;
                    // Then render block selection content
                    renderer.render_with_block_selection(
                        split_x + 1, 1, text_viewport_width, text_viewport_height,
                        viewport_relative_cursor,
                        app.block_selection.as_ref()
                    )?;
                } else {
                    // First render the label
                    renderer.render_with_label(
                        split_x + 1, 0, text_viewport_width, 1, Some(method_label)
                    )?;

                    // Use normal selection renderer
                    let (sel_start, sel_end) = if app.selection.primary().len() > 0 {
                        let range = app.selection.primary();
                        let start_line = app.rope.char_to_line(range.from());
                        let end_line = app.rope.char_to_line(range.to());
                        let start_line_char = app.rope.line_to_char(start_line);
                        let end_line_char = app.rope.line_to_char(end_line);
                        (
                            Some((range.from() - start_line_char, start_line)),
                            Some((range.to() - end_line_char, end_line))
                        )
                    } else {
                        (None, None)
                    };

                    renderer.render_with_cursor_and_selection(
                        split_x + 1, 1, text_viewport_width, text_viewport_height,
                        viewport_relative_cursor,
                        sel_start,
                        sel_end
                    )?;
                }

                // Draw scrollbars for text editor
                renderer.draw_scrollbars(split_x + 1, 1, text_viewport_width, text_viewport_height)?;
            }

            // Status bar disabled to prevent debug flood
            // render_status_bar(&mut stdout, app, term_width, term_height)?;

            stdout.flush()?;
            app.needs_redraw = false;
        }

        // Check for momentum-based scrolling updates
        if mouse_state.scroll_momentum.velocity_y.abs() > 0.1 ||
           mouse_state.scroll_momentum.velocity_x.abs() > 0.1 {
            mouse::apply_smooth_scroll(app, &mut mouse_state);
        }

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

// Status bar function removed - disabled in main rendering loop