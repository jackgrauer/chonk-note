// Debug capture module for Chonker 7.59
use std::sync::{Arc, Mutex};
use lazy_static::lazy_static;

lazy_static! {
    pub static ref DEBUG_BUFFER: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
}

/// Add a debug message to both stderr and the debug buffer
pub fn debug_print(_msg: String) {
    // Debug output completely disabled
}

/// Get a copy of all debug messages
pub fn get_debug_messages() -> Vec<String> {
    if let Ok(buffer) = DEBUG_BUFFER.lock() {
        buffer.clone()
    } else {
        Vec::new()
    }
}

/// Clear the debug buffer
pub fn clear_debug_buffer() {
    if let Ok(mut buffer) = DEBUG_BUFFER.lock() {
        buffer.clear();
    }
}

// Convenience macro for debug printing
#[macro_export]
macro_rules! debug_log {
    ($($arg:tt)*) => {
        $crate::debug_capture::debug_print(format!($($arg)*))
    };
}