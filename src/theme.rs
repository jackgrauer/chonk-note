// CROSSTERM ELIMINATED! Pure Kitty RGB theming
use crate::kitty_native::KittyTerminal;

pub struct KittyTheme;

impl KittyTheme {
    // RGB color values (no crossterm Color enum)

    // Text colors
    pub const ACCENT_TEXT: (u8, u8, u8) = (143, 161, 179);      // #8FA1B3 - Muted cyan
    pub const ACCENT_LOAD_FILE: (u8, u8, u8) = (169, 133, 202); // #A985CA - Soft purple
    pub const TEXT_PRIMARY: (u8, u8, u8) = (197, 200, 198);     // Light gray
    pub const TEXT_SECONDARY: (u8, u8, u8) = (150, 152, 150);   // Medium gray
    pub const TEXT_DIM: (u8, u8, u8) = (96, 99, 102);           // Dark gray
    pub const TEXT_HEADER: (u8, u8, u8) = (255, 255, 255);      // White
    pub const SUCCESS: (u8, u8, u8) = (181, 189, 104);          // #B5BD68 - Green

    // Background colors
    pub const BG_STATUS: (u8, u8, u8) = (0, 0, 0);              // Black
    pub const BG_CURSOR: (u8, u8, u8) = (143, 161, 179);        // Muted cyan
    pub const BG_SELECTION: (u8, u8, u8) = (0, 0, 139);         // Dark blue

    // Helper functions for direct Kitty color setting
    pub fn set_accent_text_fg() -> std::io::Result<()> {
        KittyTerminal::set_fg_rgb(Self::ACCENT_TEXT.0, Self::ACCENT_TEXT.1, Self::ACCENT_TEXT.2)
    }

    pub fn set_accent_load_file_bg() -> std::io::Result<()> {
        KittyTerminal::set_bg_rgb(Self::ACCENT_LOAD_FILE.0, Self::ACCENT_LOAD_FILE.1, Self::ACCENT_LOAD_FILE.2)
    }

    pub fn set_text_primary_fg() -> std::io::Result<()> {
        KittyTerminal::set_fg_rgb(Self::TEXT_PRIMARY.0, Self::TEXT_PRIMARY.1, Self::TEXT_PRIMARY.2)
    }

    pub fn set_text_header_fg() -> std::io::Result<()> {
        KittyTerminal::set_fg_rgb(Self::TEXT_HEADER.0, Self::TEXT_HEADER.1, Self::TEXT_HEADER.2)
    }

    pub fn set_success_fg() -> std::io::Result<()> {
        KittyTerminal::set_fg_rgb(Self::SUCCESS.0, Self::SUCCESS.1, Self::SUCCESS.2)
    }

    pub fn set_text_dim_fg() -> std::io::Result<()> {
        KittyTerminal::set_fg_rgb(Self::TEXT_DIM.0, Self::TEXT_DIM.1, Self::TEXT_DIM.2)
    }

    pub fn reset_colors() -> std::io::Result<()> {
        KittyTerminal::reset_colors()
    }
}