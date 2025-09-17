// KITTY-NATIVE TERMINAL CONTROL
// Direct escape sequences, no crossterm abstraction
use std::io::{self, Write, Read};

// Kitty-native key definitions
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
        print!("\x1b[?1000h");  // Enable mouse tracking
        io::stdout().flush()?;
        Ok(())
    }

    pub fn exit_fullscreen() -> Result<(), io::Error> {
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

    // Raw keyboard input parsing
    pub fn read_key() -> Result<Option<KeyEvent>, io::Error> {
        let mut buffer = [0u8; 16];
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

        // Parse escape sequences
        Self::parse_input(&buffer[..bytes_read])
    }

    fn parse_input(bytes: &[u8]) -> Result<Option<KeyEvent>, io::Error> {
        if bytes.is_empty() {
            return Ok(None);
        }

        let mut modifiers = KeyModifiers {
            ctrl: false,
            alt: false,
            shift: false,
            cmd: false,
        };

        match bytes {
            // Special keys FIRST (before control character parsing)
            [13] => Ok(Some(KeyEvent { code: KeyCode::Enter, modifiers })),
            [127] => Ok(Some(KeyEvent { code: KeyCode::Backspace, modifiers })),
            [9] => Ok(Some(KeyEvent { code: KeyCode::Tab, modifiers })),
            [27] if bytes.len() == 1 => Ok(Some(KeyEvent { code: KeyCode::Esc, modifiers })),

            // Simple characters
            [b] if *b >= 32 && *b <= 126 => {
                Ok(Some(KeyEvent {
                    code: KeyCode::Char(*b as char),
                    modifiers,
                }))
            }

            // Control characters (excluding Enter=13, Tab=9 which are handled above)
            [b] if *b >= 1 && *b <= 26 && *b != 13 && *b != 9 => {
                modifiers.ctrl = true;
                let ch = (*b - 1 + b'a') as char;
                Ok(Some(KeyEvent {
                    code: KeyCode::Char(ch),
                    modifiers,
                }))
            }

            // Command key on macOS (Cmd+char)
            [226, 140, 152, b] => {
                modifiers.cmd = true;
                Ok(Some(KeyEvent {
                    code: KeyCode::Char(*b as char),
                    modifiers,
                }))
            }

            // Arrow keys
            [27, 91, 65] => Ok(Some(KeyEvent { code: KeyCode::Up, modifiers })),
            [27, 91, 66] => Ok(Some(KeyEvent { code: KeyCode::Down, modifiers })),
            [27, 91, 68] => Ok(Some(KeyEvent { code: KeyCode::Left, modifiers })),
            [27, 91, 67] => Ok(Some(KeyEvent { code: KeyCode::Right, modifiers })),

            // Shift+Arrow keys (for selection)
            [27, 91, 49, 59, 50, 65] => {
                modifiers.shift = true;
                Ok(Some(KeyEvent { code: KeyCode::Up, modifiers }))
            }
            [27, 91, 49, 59, 50, 66] => {
                modifiers.shift = true;
                Ok(Some(KeyEvent { code: KeyCode::Down, modifiers }))
            }
            [27, 91, 49, 59, 50, 68] => {
                modifiers.shift = true;
                Ok(Some(KeyEvent { code: KeyCode::Left, modifiers }))
            }
            [27, 91, 49, 59, 50, 67] => {
                modifiers.shift = true;
                Ok(Some(KeyEvent { code: KeyCode::Right, modifiers }))
            }

            // Home/End
            [27, 91, 72] => Ok(Some(KeyEvent { code: KeyCode::Home, modifiers })),
            [27, 91, 70] => Ok(Some(KeyEvent { code: KeyCode::End, modifiers })),

            // Page Up/Down
            [27, 91, 53, 126] => Ok(Some(KeyEvent { code: KeyCode::PageUp, modifiers })),
            [27, 91, 54, 126] => Ok(Some(KeyEvent { code: KeyCode::PageDown, modifiers })),

            _ => Ok(None), // Unknown sequence
        }
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