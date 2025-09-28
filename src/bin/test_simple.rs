// Simple test for the Helix-native editor with block selection
use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use std::io::{self, Write};

fn main() -> Result<()> {
    println!("Simple Helix-Native Editor Test");
    println!("================================");
    println!();
    println!("This is our new editor with:");
    println!("  ✅ Helix text manipulation (Rope, Selection, Transaction)");
    println!("  ✅ Block selection support");
    println!("  ✅ NO cursor acceleration!");
    println!("  ✅ Clean command pattern");
    println!();
    println!("Commands:");
    println!("  Arrow keys     - Move cursor (single step)");
    println!("  h/j/k/l        - Vim-style movement");
    println!("  Ctrl-V         - Block selection mode");
    println!("  i              - Insert mode");
    println!("  Escape         - Exit mode/block selection");
    println!("  Ctrl-Q         - Quit");
    println!();
    println!("Press Enter to start...");

    let mut _input = String::new();
    io::stdin().read_line(&mut _input)?;

    // Run the editor
    run_editor()
}

fn run_editor() -> Result<()> {
    // We'll include the simple_helix_editor inline for this test
    use chonker7::simple_helix_editor::{SimpleHelixEditor, Command, SelectionMode, create_simple_keymap};

    // Initialize terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    stdout.execute(EnterAlternateScreen)?;

    // Create editor with test text
    let test_text = "Line 1: Welcome to Helix-native Chonker7!\n\
                     Line 2: This uses Helix's Rope data structure\n\
                     Line 3: With proper Transaction-based editing\n\
                     Line 4: And our custom block selection!\n\
                     Line 5: No more cursor acceleration\n\
                     Line 6: Clean command pattern\n\
                     Line 7: Ready for notes and PDF modes";

    let mut editor = SimpleHelixEditor::from_text(test_text);
    let keymap = create_simple_keymap();

    loop {
        // Clear and render
        print!("\x1b[2J\x1b[H");
        render(&editor)?;

        // Handle input
        if let Event::Key(key) = event::read()? {
            // Quit on Ctrl-Q
            if key.code == KeyCode::Char('q') && key.modifiers.contains(KeyModifiers::CONTROL) {
                break;
            }

            // Convert key and find command
            let helix_key = convert_key(key);
            let helix_mods = convert_modifiers(key.modifiers);

            // Special handling for insert mode
            if editor.mode == SelectionMode::Insert {
                if key.code == KeyCode::Esc {
                    editor.execute_command(Command::NormalMode)?;
                } else if let KeyCode::Char(c) = key.code {
                    editor.execute_command(Command::InsertChar(c))?;
                } else if key.code == KeyCode::Enter {
                    editor.execute_command(Command::InsertChar('\n'))?;
                } else if key.code == KeyCode::Backspace {
                    editor.execute_command(Command::Backspace)?;
                }
            } else if editor.mode == SelectionMode::BlockInsert {
                if key.code == KeyCode::Esc {
                    editor.execute_command(Command::ExitBlockMode)?;
                } else if let KeyCode::Char(c) = key.code {
                    editor.execute_command(Command::InsertChar(c))?;
                }
            } else {
                // Normal/Block mode - use keymap
                if let Some(command) = keymap.get(&(helix_key, helix_mods)) {
                    editor.execute_command(command.clone())?;
                }
            }
        }
    }

    // Cleanup
    disable_raw_mode()?;
    stdout.execute(LeaveAlternateScreen)?;

    println!("Editor closed successfully!");
    Ok(())
}

fn render(editor: &chonker7::simple_helix_editor::SimpleHelixEditor) -> Result<()> {
    use chonker7::simple_helix_editor::SelectionMode;

    println!("╔═══════════════════════════════════════════════════════════════╗");
    println!("║  HELIX-NATIVE EDITOR - Mode: {:30?} ║", editor.mode);
    println!("╚═══════════════════════════════════════════════════════════════╝");
    println!();

    let text = &editor.document.rope;
    let selection = &editor.document.selection;

    match editor.mode {
        SelectionMode::Block | SelectionMode::BlockInsert => {
            // Block selection rendering
            if let Some(ref block) = editor.block_selection {
                let lines: Vec<String> = text.to_string().lines().map(|s| s.to_string()).collect();

                for (line_num, line) in lines.iter().enumerate() {
                    print!("{:3} │ ", line_num + 1);

                    // Check if this line is in the block
                    let (start_pos, end_pos) = block.normalized();

                    if line_num >= start_pos.0 && line_num <= end_pos.0 {
                        let start_col = start_pos.1;
                        let end_col = end_pos.1;

                        // Before block
                        if start_col <= line.len() {
                            print!("{}", &line[..start_col.min(line.len())]);

                            // Block selection (inverse)
                            print!("\x1b[7m");
                            if start_col < line.len() {
                                print!("{}", &line[start_col..end_col.min(line.len())]);
                            }
                            // Virtual space in block
                            for _ in line.len()..end_col {
                                print!(" ");
                            }
                            print!("\x1b[0m");

                            // After block
                            if end_col < line.len() {
                                print!("{}", &line[end_col..]);
                            }
                        } else {
                            print!("{}", line);
                            // Block in virtual space
                            for _ in line.len()..start_col {
                                print!(" ");
                            }
                            print!("\x1b[7m");
                            for _ in start_col..end_col {
                                print!(" ");
                            }
                            print!("\x1b[0m");
                        }
                    } else {
                        print!("{}", line);
                    }
                    println!();
                }

                println!();
                let (start, end) = block.normalized();
                println!("Block: {}×{} chars",
                    end.1 - start.1,
                    end.0 - start.0 + 1
                );
            }
        }
        _ => {
            // Normal rendering with cursor
            let cursor_pos = selection.primary().cursor(text.slice(..));

            for (idx, ch) in text.chars().enumerate() {
                if idx == cursor_pos {
                    // Show cursor
                    if ch == '\n' {
                        print!("\x1b[7m \x1b[0m\n");
                    } else {
                        print!("\x1b[7m{}\x1b[0m", ch);
                    }
                } else {
                    print!("{}", ch);
                }
            }
            println!();
        }
    }

    println!();
    println!("{}", "─".repeat(65));
    println!("Arrows/hjkl=move | Ctrl-V=block | i=insert | Esc=normal | Ctrl-Q=quit");

    if let Some(col) = editor.virtual_cursor_col {
        print!(" | Virtual col: {}", col);
    }
    println!();

    io::stdout().flush()?;
    Ok(())
}

fn convert_key(key: event::KeyEvent) -> helix_view::input::KeyCode {
    use helix_view::input::KeyCode as HKeyCode;
    use event::KeyCode;

    match key.code {
        KeyCode::Char(c) => HKeyCode::Char(c),
        KeyCode::Enter => HKeyCode::Enter,
        KeyCode::Esc => HKeyCode::Esc,
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
    }
}

fn convert_modifiers(mods: KeyModifiers) -> helix_view::input::KeyModifiers {
    use helix_view::input::KeyModifiers as HMods;

    let mut result = HMods::empty();

    if mods.contains(KeyModifiers::SHIFT) {
        result |= HMods::SHIFT;
    }
    if mods.contains(KeyModifiers::CONTROL) {
        result |= HMods::CONTROL;
    }
    if mods.contains(KeyModifiers::ALT) {
        result |= HMods::ALT;
    }

    result
}