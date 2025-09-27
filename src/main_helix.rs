// Main entry point for Helix-native Chonker7
use anyhow::Result;
use clap::{Parser, Subcommand};
use crossterm::{
    event::{self, Event, KeyEvent},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use std::io::{self, Write};

mod block_selection;
mod helix_native_editor;
mod helix_keymap;

use helix_native_editor::{EditorCommand, HelixNativeEditor, SelectionMode};
use helix_keymap::{handle_key_input, create_default_keymap};

#[derive(Parser)]
#[command(author, version, about)]
struct Args {
    /// File to open
    file: Option<String>,

    #[command(subcommand)]
    mode: Option<Mode>,
}

#[derive(Subcommand)]
enum Mode {
    /// Note-taking mode
    Notes,
    /// PDF viewing mode
    Pdf {
        /// PDF file to open
        file: String,
    },
    /// Pure text editing mode (default)
    Editor,
}

fn main() -> Result<()> {
    let args = Args::parse();

    // Initialize terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    stdout.execute(EnterAlternateScreen)?;

    // Create Helix-native editor
    let mut editor = HelixNativeEditor::new()?;

    // Set mode based on arguments
    match args.mode {
        Some(Mode::Notes) => {
            println!("Notes mode activated");
            // TODO: Initialize notes database
        }
        Some(Mode::Pdf { file }) => {
            println!("PDF mode: {}", file);
            // TODO: Load PDF document
        }
        _ => {
            // Default editor mode
            if let Some(file) = args.file {
                // TODO: Open file
                println!("Opening: {}", file);
            }
        }
    }

    // Main event loop
    let result = run_editor(&mut editor);

    // Cleanup terminal
    disable_raw_mode()?;
    stdout.execute(LeaveAlternateScreen)?;

    result
}

fn run_editor(editor: &mut HelixNativeEditor) -> Result<()> {
    loop {
        // Render the editor
        render(editor)?;

        // Handle input
        if let Event::Key(key) = event::read()? {
            // Check for quit command
            if key.code == event::KeyCode::Char('q')
                && key.modifiers.contains(event::KeyModifiers::CONTROL)
            {
                break;
            }

            // Convert crossterm key to helix key
            let helix_key = convert_key(key);

            // Handle the key
            handle_key_input(editor, helix_key)?;
        }
    }

    Ok(())
}

fn render(editor: &mut HelixNativeEditor) -> Result<()> {
    // Clear screen
    print!("\x1b[2J\x1b[H");

    // Get document content
    let (_view, doc) = editor.editor.current_ref();
    let text = doc.text();

    // Render based on mode
    match editor.selection_mode {
        SelectionMode::Block | SelectionMode::BlockInsert => {
            println!("=== BLOCK SELECTION MODE ===\n");

            if let Some(ref block) = editor.block_selection {
                // Render text with block selection highlighted
                for (line_num, line) in text.to_string().lines().enumerate() {
                    if line_num >= block.anchor.line && line_num <= block.cursor.line {
                        // Highlight the selected portion
                        let (start_pos, end_pos) = block.normalized();
                        let start_col = start_pos.column;
                        let end_col = end_pos.column;

                        if start_col < line.len() {
                            print!("{}", &line[..start_col.min(line.len())]);
                            print!("\x1b[7m"); // Inverse video for selection
                            print!(
                                "{}",
                                &line[start_col.min(line.len())..end_col.min(line.len())]
                            );
                            print!("\x1b[0m"); // Reset
                            println!("{}", &line[end_col.min(line.len())..]);
                        } else {
                            println!("{}", line);
                        }
                    } else {
                        println!("{}", line);
                    }
                }

                println!("\nBlock: {}x{} lines",
                    block.cursor_visual_col - block.anchor_visual_col,
                    block.cursor.line - block.anchor.line + 1
                );
            }
        }
        SelectionMode::Normal => {
            println!("=== NORMAL MODE ===\n");

            // Render text with regular selection
            let selection = doc.selection(editor.editor.tree.focus);
            let primary = selection.primary();

            for (idx, ch) in text.chars().enumerate() {
                if idx == primary.cursor(text.slice(..)) {
                    print!("\x1b[7m{}\x1b[0m", ch); // Highlight cursor
                } else if idx >= primary.from() && idx < primary.to() && primary.from() != primary.to() {
                    print!("\x1b[4m{}\x1b[0m", ch); // Underline selection
                } else {
                    print!("{}", ch);
                }
            }
        }
    }

    // Show status line
    println!("\n{}", "─".repeat(80));
    println!(
        "Mode: {:?} | Ctrl-Q to quit | Ctrl-V for block selection",
        editor.selection_mode
    );

    // Flush output
    io::stdout().flush()?;

    Ok(())
}

fn convert_key(key: KeyEvent) -> helix_view::input::KeyEvent {
    use helix_view::input::{KeyCode as HKeyCode, KeyModifiers as HKeyModifiers};

    let code = match key.code {
        event::KeyCode::Char(c) => HKeyCode::Char(c),
        event::KeyCode::Enter => HKeyCode::Enter,
        event::KeyCode::Escape | event::KeyCode::Esc => HKeyCode::Esc,
        event::KeyCode::Backspace => HKeyCode::Backspace,
        event::KeyCode::Left => HKeyCode::Left,
        event::KeyCode::Right => HKeyCode::Right,
        event::KeyCode::Up => HKeyCode::Up,
        event::KeyCode::Down => HKeyCode::Down,
        event::KeyCode::Home => HKeyCode::Home,
        event::KeyCode::End => HKeyCode::End,
        event::KeyCode::PageUp => HKeyCode::PageUp,
        event::KeyCode::PageDown => HKeyCode::PageDown,
        event::KeyCode::Tab => HKeyCode::Tab,
        event::KeyCode::Delete => HKeyCode::Delete,
        event::KeyCode::Insert => HKeyCode::Insert,
        event::KeyCode::F(n) => HKeyCode::F(n),
        _ => HKeyCode::Null,
    };

    let modifiers = HKeyModifiers::from_bits_truncate(
        if key.modifiers.contains(event::KeyModifiers::SHIFT) {
            HKeyModifiers::SHIFT.bits()
        } else {
            0
        } | if key.modifiers.contains(event::KeyModifiers::CONTROL) {
            HKeyModifiers::CONTROL.bits()
        } else {
            0
        } | if key.modifiers.contains(event::KeyModifiers::ALT) {
            HKeyModifiers::ALT.bits()
        } else {
            0
        } | if key.modifiers.contains(event::KeyModifiers::SUPER) {
            HKeyModifiers::NONE.bits() // Map SUPER to something if needed
        } else {
            0
        },
    );

    helix_view::input::KeyEvent { code, modifiers }
}