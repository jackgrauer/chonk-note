// Minimal debug that doesn't change app behavior AT ALL
use std::fs::OpenOptions;
use std::io::Write;

/// Just writes coordinate info to a file after EVERY click
/// No UI changes, no behavior changes
pub fn log_click(x: u16, y: u16, cursor_row: usize, cursor_col: usize) {
    if let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open("/tmp/clicks.log")
    {
        writeln!(file, "Click({},{}) → Cursor({},{})", x, y, cursor_row, cursor_col).ok();

        // If they don't match, also write to stderr so we see it immediately
        if x as usize != cursor_col || y as usize != cursor_row {
            eprintln!("MISMATCH: Click({},{}) → Cursor({},{})", x, y, cursor_row, cursor_col);
        }
    }
}

/// Run this AFTER normal build to see mismatches
pub fn analyze_clicks() {
    println!("=== Click Analysis ===");

    if let Ok(contents) = std::fs::read_to_string("/tmp/clicks.log") {
        let mut mismatches = 0;
        for line in contents.lines() {
            if line.contains("→") {
                // Parse Click(x,y) → Cursor(r,c)
                let parts: Vec<&str> = line.split(" → ").collect();
                if parts.len() == 2 {
                    let click = parts[0].replace("Click(", "").replace(")", "");
                    let cursor = parts[1].replace("Cursor(", "").replace(")", "");

                    let click_coords: Vec<&str> = click.split(",").collect();
                    let cursor_coords: Vec<&str> = cursor.split(",").collect();

                    if click_coords.len() == 2 && cursor_coords.len() == 2 {
                        let click_x: usize = click_coords[0].parse().unwrap_or(0);
                        let click_y: usize = click_coords[1].parse().unwrap_or(0);
                        let cursor_col: usize = cursor_coords[1].parse().unwrap_or(0);
                        let cursor_row: usize = cursor_coords[0].parse().unwrap_or(0);

                        // Check if they match (accounting for pane offsets)
                        if (click_x != cursor_col) || (click_y != cursor_row) {
                            println!("❌ Mismatch: clicked ({},{}) but cursor at ({},{})",
                                click_x, click_y, cursor_row, cursor_col);
                            mismatches += 1;
                        }
                    }
                }
            }
        }

        if mismatches == 0 {
            println!("✅ All clicks matched cursor position");
        } else {
            println!("\n❌ Found {} mismatches", mismatches);
            println!("This means coordinate calculation is broken");
        }
    } else {
        println!("No clicks logged yet. Run the app and click around first.");
    }
}