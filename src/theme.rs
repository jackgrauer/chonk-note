use crossterm::style::Color;

// Ghostty-inspired color theme for pure crossterm
pub struct ChonkerTheme;

impl ChonkerTheme {
    // Dark mode backgrounds - pure black
    pub fn bg_secondary_dark() -> Color { Color::Rgb { r: 0, g: 0, b: 0 } }
    pub fn bg_status_dark() -> Color { Color::Rgb { r: 0, g: 0, b: 0 } }
    
    // Light mode backgrounds
    pub fn bg_secondary_light() -> Color { Color::Rgb { r: 250, g: 250, b: 250 } }
    
    // Accent colors (same for both modes)
    pub fn accent_pdf() -> Color { Color::Rgb { r: 129, g: 162, b: 190 } }   // #81A2BE - Soft blue
    pub fn accent_text() -> Color { Color::Rgb { r: 143, g: 161, b: 179 } }  // #8FA1B3 - Muted cyan
    pub fn accent_options() -> Color { Color::Rgb { r: 100, g: 149, b: 237 } } // #6495ED - Cornflower blue
    pub fn accent_load_file() -> Color { Color::Rgb { r: 169, g: 133, b: 202 } } // #A985CA - Soft purple
    
    // Text colors for dark mode
    pub fn text_primary_dark() -> Color { Color::Rgb { r: 197, g: 200, b: 198 } }
    pub fn text_secondary_dark() -> Color { Color::Rgb { r: 150, g: 152, b: 150 } }
    pub fn text_dim_dark() -> Color { Color::Rgb { r: 96, g: 99, b: 102 } }
    pub fn text_header_dark() -> Color { Color::Rgb { r: 255, g: 255, b: 255 } }
    pub fn text_status_dark() -> Color { Color::Rgb { r: 197, g: 200, b: 198 } }
    
    // Functional colors (same for both modes)
    pub fn success() -> Color { Color::Rgb { r: 181, g: 189, b: 104 } }      // #B5BD68 - Green
    
    // For backwards compatibility - default to dark mode
    pub fn bg_secondary() -> Color { Self::bg_secondary_dark() }
    pub fn bg_status() -> Color { Self::bg_status_dark() }
    pub fn text_primary() -> Color { Self::text_primary_dark() }
    pub fn text_secondary() -> Color { Self::text_secondary_dark() }
    pub fn text_dim() -> Color { Self::text_dim_dark() }
    pub fn text_header() -> Color { Self::text_header_dark() }
}