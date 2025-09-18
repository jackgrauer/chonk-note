// KITTY-NATIVE TERMINAL CONTROL
// Direct escape sequences, no crossterm abstraction
use std::io::{self, Write, Read};

// Kitty-native input definitions
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyCode {
    Char(char),
    Enter,
    Backspace,
    Tab,
    Esc,
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    PageUp,
    PageDown,
}

// Mouse events support
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MouseButton {
    Left,
    Middle,
    Right,
    ScrollUp,
    ScrollDown,
}

#[derive(Debug, Clone, Copy)]
pub struct MouseEvent {
    pub button: Option<MouseButton>,
    pub x: u16,
    pub y: u16,
    pub modifiers: KeyModifiers,
    pub is_press: bool,  // true = press, false = release
    pub is_drag: bool,
}

// Unified input event
#[derive(Debug)]
pub enum InputEvent {
    Key(KeyEvent),
    Mouse(MouseEvent),
}

#[derive(Debug, Clone, Copy)]
pub struct KeyModifiers {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub cmd: bool,
}

impl KeyModifiers {
    pub const CONTROL: Self = KeyModifiers { ctrl: true, alt: false, shift: false, cmd: false };
    pub const SUPER: Self = KeyModifiers { ctrl: false, alt: false, shift: false, cmd: true };
    pub const SHIFT: Self = KeyModifiers { ctrl: false, alt: false, shift: true, cmd: false };
    pub const ALT: Self = KeyModifiers { ctrl: false, alt: true, shift: false, cmd: false };

    pub fn contains(&self, other: KeyModifiers) -> bool {
        (!other.ctrl || self.ctrl) &&
        (!other.alt || self.alt) &&
        (!other.shift || self.shift) &&
        (!other.cmd || self.cmd)
    }
}

#[derive(Debug)]
pub struct KeyEvent {
    pub code: KeyCode,
    pub modifiers: KeyModifiers,
}

pub struct KittyTerminal;

impl KittyTerminal {
    // Terminal setup
    pub fn enter_fullscreen() -> Result<(), io::Error> {
        print!("\x1b[?1049h");  // Save screen & enter alternate buffer
        print!("\x1b[2J");      // Clear screen
        print!("\x1b[H");       // Move to top-left
        print!("\x1b[?25l");    // Hide cursor

        // Enable mouse tracking
        print!("\x1b[?1000h");  // Enable mouse tracking (this should grab the mouse)
        print!("\x1b[?1002h");  // Enable mouse drag tracking
        print!("\x1b[?1006h");  // Enable SGR mouse mode (extended coordinates)

        io::stdout().flush()?;

        // Debug log that mouse mode was enabled
        if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open("/Users/jack/chonker7_debug.log") {
            use std::io::Write;
            writeln!(file, "[TERMINAL] Mouse tracking enabled with SGR mode").ok();
        }

        Ok(())
    }

    pub fn exit_fullscreen() -> Result<(), io::Error> {
        print!("\x1b[?1006l");  // Disable SGR mouse mode
        print!("\x1b[?1002l");  // Disable mouse drag tracking
        print!("\x1b[?1000l");  // Disable mouse tracking
        print!("\x1b[?25h");    // Show cursor
        print!("\x1b[2J");      // Clear screen
        print!("\x1b[H");       // Move to top-left
        print!("\x1b[?1049l");  // Restore screen & exit alternate buffer
        io::stdout().flush()?;
        Ok(())
    }

    // Cursor control
    pub fn move_to(x: u16, y: u16) -> Result<(), io::Error> {
        print!("\x1b[{};{}H", y + 1, x + 1);  // 1-based coordinates
        io::stdout().flush()?;
        Ok(())
    }

    pub fn hide_cursor() -> Result<(), io::Error> {
        print!("\x1b[?25l");
        io::stdout().flush()?;
        Ok(())
    }

    pub fn show_cursor() -> Result<(), io::Error> {
        print!("\x1b[?25h");
        io::stdout().flush()?;
        Ok(())
    }

    // Colors - direct RGB
    pub fn set_fg_rgb(r: u8, g: u8, b: u8) -> Result<(), io::Error> {
        print!("\x1b[38;2;{};{};{}m", r, g, b);
        io::stdout().flush()?;
        Ok(())
    }

    pub fn set_bg_rgb(r: u8, g: u8, b: u8) -> Result<(), io::Error> {
        print!("\x1b[48;2;{};{};{}m", r, g, b);
        io::stdout().flush()?;
        Ok(())
    }

    pub fn reset_colors() -> Result<(), io::Error> {
        print!("\x1b[m");
        io::stdout().flush()?;
        Ok(())
    }

    // Screen control
    pub fn clear_screen() -> Result<(), io::Error> {
        print!("\x1b[2J");
        io::stdout().flush()?;
        Ok(())
    }

    pub fn clear_line() -> Result<(), io::Error> {
        print!("\x1b[2K");
        io::stdout().flush()?;
        Ok(())
    }

    // Raw terminal mode
    pub fn enable_raw_mode() -> Result<(), io::Error> {
        unsafe {
            let mut termios: libc::termios = std::mem::zeroed();
            if libc::tcgetattr(libc::STDIN_FILENO, &mut termios) != 0 {
                return Err(io::Error::last_os_error());
            }

            // Disable canonical mode, echo, and signals
            termios.c_lflag &= !(libc::ECHO | libc::ICANON | libc::ISIG | libc::IEXTEN);
            termios.c_iflag &= !(libc::IXON | libc::ICRNL | libc::BRKINT | libc::INPCK | libc::ISTRIP);
            termios.c_cflag |= libc::CS8;
            termios.c_oflag &= !libc::OPOST;

            // Set read timeout
            termios.c_cc[libc::VMIN] = 0;
            termios.c_cc[libc::VTIME] = 1;

            if libc::tcsetattr(libc::STDIN_FILENO, libc::TCSAFLUSH, &termios) != 0 {
                return Err(io::Error::last_os_error());
            }
        }
        Ok(())
    }

    pub fn disable_raw_mode() -> Result<(), io::Error> {
        unsafe {
            let mut termios: libc::termios = std::mem::zeroed();
            if libc::tcgetattr(libc::STDIN_FILENO, &mut termios) != 0 {
                return Err(io::Error::last_os_error());
            }

            // Restore canonical mode, echo, and signals
            termios.c_lflag |= libc::ECHO | libc::ICANON | libc::ISIG | libc::IEXTEN;
            termios.c_iflag |= libc::IXON | libc::ICRNL | libc::BRKINT | libc::INPCK | libc::ISTRIP;
            termios.c_oflag |= libc::OPOST;

            if libc::tcsetattr(libc::STDIN_FILENO, libc::TCSAFLUSH, &termios) != 0 {
                return Err(io::Error::last_os_error());
            }
        }
        Ok(())
    }

    // Terminal size detection
    pub fn size() -> Result<(u16, u16), io::Error> {
        unsafe {
            let mut winsize: libc::winsize = std::mem::zeroed();
            if libc::ioctl(libc::STDOUT_FILENO, libc::TIOCGWINSZ, &mut winsize) == 0 {
                Ok((winsize.ws_col, winsize.ws_row))
            } else {
                Ok((80, 24)) // Default fallback
            }
        }
    }

    // Raw input parsing (keyboard and mouse)
    pub fn read_input() -> Result<Option<InputEvent>, io::Error> {
        let mut buffer = [0u8; 64];  // Increased for SGR mouse sequences
        let mut stdin = io::stdin();

        // Check if input is available
        unsafe {
            let mut fds: libc::fd_set = std::mem::zeroed();
            libc::FD_ZERO(&mut fds);
            libc::FD_SET(libc::STDIN_FILENO, &mut fds);

            let mut timeout = libc::timeval {
                tv_sec: 0,
                tv_usec: 16000, // 16ms timeout for 60 FPS
            };

            let result = libc::select(
                libc::STDIN_FILENO + 1,
                &mut fds,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                &mut timeout,
            );

            if result <= 0 {
                return Ok(None); // No input available or timeout
            }
        }

        // Read input
        let bytes_read = stdin.read(&mut buffer)?;
        if bytes_read == 0 {
            return Ok(None);
        }

        // Debug log bytes read
        if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open("/Users/jack/chonker7_debug.log") {
            use std::io::Write;
            writeln!(file, "[READ_INPUT] Read {} bytes: {:?}", bytes_read, &buffer[..bytes_read]).ok();
        }

        // Parse escape sequences
        Self::parse_input(&buffer[..bytes_read])
    }

    // Compatibility wrapper for existing code
    pub fn read_key() -> Result<Option<KeyEvent>, io::Error> {
        match Self::read_input()? {
            Some(InputEvent::Key(key_event)) => Ok(Some(key_event)),
            _ => Ok(None), // Ignore mouse events in legacy API
        }
    }

    fn parse_input(bytes: &[u8]) -> Result<Option<InputEvent>, io::Error> {
        if bytes.is_empty() {
            return Ok(None);
        }

        // Debug log raw input bytes
        if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open("/Users/jack/chonker7_debug.log") {
            use std::io::Write;
            writeln!(file, "[PARSE_INPUT] Raw bytes: {:?}", bytes).ok();
        }

        // Check for SGR mouse sequence first: CSI < button ; x ; y M/m
        if bytes.len() >= 6 && bytes[0] == 27 && bytes[1] == b'[' && bytes[2] == b'<' {
            if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open("/Users/jack/chonker7_debug.log") {
                use std::io::Write;
                writeln!(file, "[PARSE_INPUT] Detected SGR mouse sequence").ok();
            }
            return Self::parse_sgr_mouse(bytes);
        }

        let mut modifiers = KeyModifiers {
            ctrl: false,
            alt: false,
            shift: false,
            cmd: false,
        };

        match bytes {
            // Special keys FIRST (before control character parsing)
            [13] => Ok(Some(InputEvent::Key(KeyEvent { code: KeyCode::Enter, modifiers }))),
            [127] => Ok(Some(InputEvent::Key(KeyEvent { code: KeyCode::Backspace, modifiers }))),
            [9] => Ok(Some(InputEvent::Key(KeyEvent { code: KeyCode::Tab, modifiers }))),
            [27] if bytes.len() == 1 => Ok(Some(InputEvent::Key(KeyEvent { code: KeyCode::Esc, modifiers }))),

            // Simple characters
            [b] if *b >= 32 && *b <= 126 => {
                Ok(Some(InputEvent::Key(KeyEvent {
                    code: KeyCode::Char(*b as char),
                    modifiers,
                })))
            }

            // Control characters (excluding Enter=13, Tab=9 which are handled above)
            [b] if *b >= 1 && *b <= 26 && *b != 13 && *b != 9 => {
                modifiers.ctrl = true;
                let ch = (*b - 1 + b'a') as char;

                // Debug log control character parsing
                if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open("/Users/jack/chonker7_debug.log") {
                    use std::io::Write;
                    writeln!(file, "[PARSE_INPUT] Control char: byte={}, char='{}', ctrl={}",
                        b, ch, modifiers.ctrl).ok();
                }

                Ok(Some(InputEvent::Key(KeyEvent {
                    code: KeyCode::Char(ch),
                    modifiers,
                })))
            }

            // Command key on macOS (Cmd+char)
            [226, 140, 152, b] => {
                modifiers.cmd = true;
                Ok(Some(InputEvent::Key(KeyEvent {
                    code: KeyCode::Char(*b as char),
                    modifiers,
                })))
            }

            // Arrow keys
            [27, 91, 65] => Ok(Some(InputEvent::Key(KeyEvent { code: KeyCode::Up, modifiers }))),
            [27, 91, 66] => Ok(Some(InputEvent::Key(KeyEvent { code: KeyCode::Down, modifiers }))),
            [27, 91, 68] => Ok(Some(InputEvent::Key(KeyEvent { code: KeyCode::Left, modifiers }))),
            [27, 91, 67] => Ok(Some(InputEvent::Key(KeyEvent { code: KeyCode::Right, modifiers }))),

            // Shift+Arrow keys (for selection)
            [27, 91, 49, 59, 50, 65] => {
                modifiers.shift = true;
                Ok(Some(InputEvent::Key(KeyEvent { code: KeyCode::Up, modifiers })))
            }
            [27, 91, 49, 59, 50, 66] => {
                modifiers.shift = true;
                Ok(Some(InputEvent::Key(KeyEvent { code: KeyCode::Down, modifiers })))
            }
            [27, 91, 49, 59, 50, 68] => {
                modifiers.shift = true;
                Ok(Some(InputEvent::Key(KeyEvent { code: KeyCode::Left, modifiers })))
            }
            [27, 91, 49, 59, 50, 67] => {
                modifiers.shift = true;
                Ok(Some(InputEvent::Key(KeyEvent { code: KeyCode::Right, modifiers })))
            }

            // Home/End
            [27, 91, 72] => Ok(Some(InputEvent::Key(KeyEvent { code: KeyCode::Home, modifiers }))),
            [27, 91, 70] => Ok(Some(InputEvent::Key(KeyEvent { code: KeyCode::End, modifiers }))),

            // Page Up/Down
            [27, 91, 53, 126] => Ok(Some(InputEvent::Key(KeyEvent { code: KeyCode::PageUp, modifiers }))),
            [27, 91, 54, 126] => Ok(Some(InputEvent::Key(KeyEvent { code: KeyCode::PageDown, modifiers }))),

            _ => Ok(None), // Unknown sequence
        }
    }

    // Parse SGR mouse events: CSI < button ; x ; y M/m
    fn parse_sgr_mouse(bytes: &[u8]) -> Result<Option<InputEvent>, io::Error> {
        // Debug log SGR parsing
        if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open("/Users/jack/chonker7_debug.log") {
            use std::io::Write;
            writeln!(file, "[SGR_PARSE] Parsing bytes: {:?}", bytes).ok();
        }

        // Skip ESC [ <
        let data = &bytes[3..];

        // Find the M or m at the end
        let end_idx = match data.iter().position(|&b| b == b'M' || b == b'm') {
            Some(idx) => idx,
            None => return Ok(None),
        };
        let is_press = data[end_idx] == b'M';

        // Parse the numbers
        let nums_str = match std::str::from_utf8(&data[..end_idx]) {
            Ok(s) => s,
            Err(_) => return Ok(None),
        };
        let parts: Vec<&str> = nums_str.split(';').collect();

        if parts.len() != 3 {
            return Ok(None);
        }

        let button_code = match parts[0].parse::<u32>() {
            Ok(n) => n,
            Err(_) => return Ok(None),
        };
        let x = match parts[1].parse::<u16>() {
            Ok(n) => n.saturating_sub(1), // Convert to 0-based
            Err(_) => return Ok(None),
        };
        let y = match parts[2].parse::<u16>() {
            Ok(n) => n.saturating_sub(1), // Convert to 0-based
            Err(_) => return Ok(None),
        };

        // Decode button and modifiers from button_code
        let mut modifiers = KeyModifiers {
            ctrl: false,
            alt: false,
            shift: false,
            cmd: false,
        };

        // Extract modifier bits
        if button_code & 4 != 0 { modifiers.shift = true; }
        if button_code & 8 != 0 { modifiers.alt = true; }
        if button_code & 16 != 0 { modifiers.ctrl = true; }

        // Extract button (lower 2 bits for press, bit 5 (value 32) for drag/motion)
        // During drag, the button code is 32 + button number (0,1,2)
        let is_drag = button_code & 32 != 0;
        let button_num = if is_drag {
            button_code & 3  // Extract button from drag code
        } else {
            button_code & 3  // Extract button from normal code
        };

        // Debug log the button code
        if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open("/Users/jack/chonker7_debug.log") {
            use std::io::Write;
            writeln!(file, "[SGR_PARSE] button_code={}, is_drag={}, button_num={}, is_press={}",
                button_code, is_drag, button_num, is_press).ok();
        }

        let button = if is_drag {
            // During drag, button number tells us which button is held
            match button_num {
                0 => Some(MouseButton::Left),
                1 => Some(MouseButton::Middle),
                2 => Some(MouseButton::Right),
                _ => None,
            }
        } else {
            match button_num {
                0 => Some(MouseButton::Left),
                1 => Some(MouseButton::Middle),
                2 => Some(MouseButton::Right),
                3 if !is_press => None, // Release with no button specified
                _ => None,
            }
        };

        // Handle scroll separately (these have different codes)
        let button = if !is_drag && button_code == 64 {
            Some(MouseButton::ScrollUp)
        } else if !is_drag && button_code == 65 {
            Some(MouseButton::ScrollDown)
        } else {
            button
        };

        let event = MouseEvent {
            button,
            x,
            y,
            modifiers,
            is_press,
            is_drag,
        };

        // Debug log parsed mouse event
        if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open("/Users/jack/chonker7_debug.log") {
            use std::io::Write;
            writeln!(file, "[SGR_PARSE] Parsed mouse event: button={:?}, x={}, y={}, press={}, drag={}",
                button, x, y, is_press, is_drag).ok();
        }

        Ok(Some(InputEvent::Mouse(event)))
    }

    // Non-blocking input check
    pub fn poll_input() -> Result<bool, io::Error> {
        unsafe {
            let mut fds: libc::fd_set = std::mem::zeroed();
            libc::FD_ZERO(&mut fds);
            libc::FD_SET(libc::STDIN_FILENO, &mut fds);

            let mut timeout = libc::timeval {
                tv_sec: 0,
                tv_usec: 0, // No timeout - just check
            };

            let result = libc::select(
                libc::STDIN_FILENO + 1,
                &mut fds,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                &mut timeout,
            );

            Ok(result > 0)
        }
    }
}