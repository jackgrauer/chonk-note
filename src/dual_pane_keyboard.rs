// Stub module for notes-only mode (dual pane keyboard removed)
use crate::App;
use crate::kitty_native::KeyEvent;
use anyhow::Result;

pub fn handle_dual_pane_input(_app: &mut App, _key: &KeyEvent) -> Result<bool> {
    // No special handling in notes-only mode
    Ok(false)
}
