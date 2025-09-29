// Filter out ANSI escape sequences and control characters from text input

/// Check if a character should be allowed in the text editor
pub fn is_allowed_char(c: char) -> bool {
    match c {
        // Allow normal printable characters
        '\x20'..='\x7E' => true,  // ASCII printable range

        // Allow specific control characters
        '\n' | '\r' | '\t' => true,  // Newline, carriage return, tab

        // Allow Unicode printable characters (non-control)
        ch if ch.is_alphanumeric() || ch.is_whitespace() => true,
        ch if !ch.is_control() => true,

        // Block everything else (including ANSI escape sequences)
        _ => false,
    }
}

/// Strip ANSI escape sequences from a string
pub fn strip_ansi_codes(text: &str) -> String {
    let mut result = String::new();
    let mut in_escape = false;
    let mut escape_type = '\0';

    for c in text.chars() {
        if in_escape {
            // We're in an escape sequence
            match escape_type {
                '[' => {
                    // CSI sequence - ends with a letter
                    if c.is_ascii_alphabetic() {
                        in_escape = false;
                        escape_type = '\0';
                    }
                }
                ']' => {
                    // OSC sequence - ends with ST (ESC \) or BEL
                    if c == '\x07' {  // BEL
                        in_escape = false;
                        escape_type = '\0';
                    }
                }
                _ => {
                    // Other escape sequences - usually single character
                    in_escape = false;
                    escape_type = '\0';
                }
            }
        } else if c == '\x1B' {  // ESC character
            in_escape = true;
            // Look ahead for sequence type (this is simplified)
        } else if c == '[' && in_escape {
            escape_type = '[';
        } else if c == ']' && in_escape {
            escape_type = ']';
        } else if is_allowed_char(c) {
            result.push(c);
        }
    }

    result
}

/// Clean text for insertion into the editor
pub fn clean_text_for_insertion(text: &str) -> String {
    strip_ansi_codes(text)
        .chars()
        .filter(|c| is_allowed_char(*c))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_ansi_codes() {
        assert_eq!(strip_ansi_codes("Hello\x1B[31mRed\x1B[0mWorld"), "HelloRedWorld");
        assert_eq!(strip_ansi_codes("\x1B[2J\x1B[H"), "");
        assert_eq!(strip_ansi_codes("Normal text"), "Normal text");
    }

    #[test]
    fn test_allowed_chars() {
        assert!(is_allowed_char('a'));
        assert!(is_allowed_char(' '));
        assert!(is_allowed_char('\n'));
        assert!(is_allowed_char('\t'));
        assert!(!is_allowed_char('\x1B'));  // ESC
        assert!(!is_allowed_char('\x00'));  // NULL
        assert!(!is_allowed_char('\x07'));  // BEL
    }
}