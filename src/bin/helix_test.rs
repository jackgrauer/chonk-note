// Test binary for Helix-native editor with block selection
use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use std::io::{self, Write};

// Import from the main crate
use chonker7::{
    block_selection::BlockSelection,
    helix_native_editor::{EditorCommand, HelixNativeEditor, SelectionMode},
    helix_keymap::handle_key_input,
};

fn main() -> Result<()> {
    println!("Helix-Native Editor Test");
    println!("========================");
    println!();
    println!("Commands:");
    println!("  Arrow keys     - Move cursor (no acceleration!)");
    println!("  Ctrl-V         - Block selection mode");
    println!("  Ctrl-Alt-V     - Block insert mode");
    println!("  Escape         - Exit block mode");
    println!("  h/j/k/l        - Vim-style movement");
    println!("  i              - Insert mode");
    println!("  Ctrl-Q         - Quit");
    println!();
    println!("Press any key to start...");

    // Wait for keypress
    let _ = io::stdin().read_line(&mut String::new());

    // Initialize terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    stdout.execute(EnterAlternateScreen)?;

    // Create editor
    let mut editor = HelixNativeEditor::new()?;

    // Add some test text
    {
        let (view, doc) = editor.editor.current_mut();
        let text = "Line 1: This is a test\n\
                   Line 2: Block selection demo\n\
                   Line 3: Move with arrow keys\n\
                   Line 4: No acceleration!\n\
                   Line 5: Press Ctrl-V for block mode\n\
                   Line 6: Works with Helix commands\n\
                   Line 7: Custom + Native = Perfect";

        let transaction = helix_core::Transaction::insert(
            doc.text(),
            doc.selection(view.id),
            text,
        );
        doc.apply(&transaction, view.id);
    }

    // Main loop
    let result = run_test_editor(&mut editor);

    // Cleanup
    disable_raw_mode()?;
    stdout.execute(LeaveAlternateScreen)?;

    if let Err(e) = result {
        eprintln!("Error: {}", e);
    }

    Ok(())
}

fn run_test_editor(editor: &mut HelixNativeEditor) -> Result<()> {
    loop {
        // Clear and render
        print!("\x1b[2J\x1b[H");
        render_editor(editor)?;

        // Handle input
        if let Event::Key(key) = event::read()? {
            // Quit on Ctrl-Q
            if key.code == KeyCode::Char('q') && key.modifiers.contains(KeyModifiers::CONTROL) {
                break;
            }

            // Convert to Helix key event
            let helix_key = convert_crossterm_key(key);

            // Handle through our keymap system
            handle_key_input(editor, helix_key)?;
        }
    }

    Ok(())
}

fn render_editor(editor: &mut HelixNativeEditor) -> Result<()> {
    let (view, doc) = editor.editor.current_ref();
    let text = doc.text();
    let selection = doc.selection(view.id);

    // Header
    println!("╔═══════════════════════════════════════════════════════════════╗");
    println!("║  HELIX-NATIVE EDITOR - Mode: {:?}                              ║", editor.selection_mode);
    println!("╚═══════════════════════════════════════════════════════════════╝");
    println!();

    // Render text with highlights
    match editor.selection_mode {
        SelectionMode::Block | SelectionMode::BlockInsert => {
            // Block selection rendering
            if let Some(ref block) = editor.block_selection {
                let lines: Vec<String> = text.to_string().lines().map(|s| s.to_string()).collect();

                for (line_num, line) in lines.iter().enumerate() {
                    print!("{:3} │ ", line_num + 1);

                    // Check if this line is in the block selection
                    let (start_pos, end_pos) = block.normalized();
                    if line_num >= start_pos.line && line_num <= end_pos.line {
                        // Render with block highlight
                        let start_col = start_pos.column;
                        let end_col = end_pos.column;

                        if start_col <= line.len() {
                            // Before selection
                            print!("{}", &line[..start_col.min(line.len())]);

                            // Selection (inverse video)
                            print!("\x1b[7m");
                            if start_col < line.len() {
                                print!("{}", &line[start_col..end_col.min(line.len())]);
                            }
                            // Add spaces if selection extends beyond line
                            for _ in line.len()..end_col {
                                print!(" ");
                            }
                            print!("\x1b[0m");

                            // After selection
                            if end_col < line.len() {
                                print!("{}", &line[end_col..]);
                            }
                        } else {
                            print!("{}", line);
                            // Selection in virtual space
                            print!("\x1b[7m");
                            for _ in line.len()..end_col {
                                print!(" ");
                            }
                            print!("\x1b[0m");
                        }
                    } else {
                        print!("{}", line);
                    }
                    println!();
                }

                // Show block info
                println!();
                println!("Block Selection: {}×{} ({}:{} to {}:{})",
                    end_pos.column - start_pos.column,
                    end_pos.line - start_pos.line + 1,
                    start_pos.line + 1, start_pos.column,
                    end_pos.line + 1, end_pos.column
                );
            } else {
                // Shouldn't happen, but fallback to normal rendering
                print!("{}", text);
            }
        }
        SelectionMode::Normal => {
            // Normal selection rendering
            let cursor_pos = selection.primary().cursor(text.slice(..));
            let anchor_pos = selection.primary().anchor;

            for (idx, ch) in text.chars().enumerate() {
                if idx == cursor_pos {
                    // Cursor (inverse)
                    print!("\x1b[7m{}\x1b[0m", if ch == '\n' { ' ' } else { ch });
                } else if (idx >= anchor_pos && idx < cursor_pos) || (idx >= cursor_pos && idx < anchor_pos) {
                    // Selection (underline)
                    print!("\x1b[4m{}\x1b[0m", ch);
                } else {
                    print!("{}", ch);
                }
            }
            println!();
        }
    }

    // Status line
    println!();
    println!("{}", "─".repeat(60));
    println!("Commands: Arrows/hjkl=move | Ctrl-V=block | i=insert | Ctrl-Q=quit");
    if let Some(col) = editor.virtual_cursor_col {
        println!("Virtual column: {}", col);
    }

    io::stdout().flush()?;
    Ok(())
}

fn convert_crossterm_key(key: event::KeyEvent) -> helix_view::input::KeyEvent {
    use helix_view::input::{KeyCode as HKeyCode, KeyModifiers as HKeyModifiers};

    let code = match key.code {
        KeyCode::Char(c) => HKeyCode::Char(c),
        KeyCode::Enter => HKeyCode::Enter,
        KeyCode::Escape | KeyCode::Esc => HKeyCode::Esc,
        KeyCode::Backspace => HKeyCode::Backspace,
        KeyCode::Left => HKeyCode::Left,
        KeyCode::Right => HKeyCode::Right,
        KeyCode::Up => HKeyCode::Up,
        KeyCode::Down => HKeyCode::Down,
        KeyCode::Home => HKeyCode::Home,
        KeyCode::End => HKeyCode::End,
        KeyCode::PageUp => HKeyCode::PageUp,
        KeyCode::PageDown => HKeyCode::PageDown,
        KeyCode::Tab => HKeyCode::Tab,
        KeyCode::Delete => HKeyCode::Delete,
        KeyCode::Insert => HKeyCode::Insert,
        KeyCode::F(n) => HKeyCode::F(n),
        _ => HKeyCode::Null,
    };

    let mut modifiers = HKeyModifiers::empty();
    if key.modifiers.contains(KeyModifiers::SHIFT) {
        modifiers |= HKeyModifiers::SHIFT;
    }
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        modifiers |= HKeyModifiers::CONTROL;
    }
    if key.modifiers.contains(KeyModifiers::ALT) {
        modifiers |= HKeyModifiers::ALT;
    }

    helix_view::input::KeyEvent { code, modifiers }
}