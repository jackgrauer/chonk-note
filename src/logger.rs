// File-based logging that won't interfere with terminal UI
use std::fs::OpenOptions;
use std::io::Write;
use std::sync::Mutex;
use once_cell::sync::Lazy;

static LOG_FILE: Lazy<Mutex<Option<std::fs::File>>> = Lazy::new(|| {
    if std::env::var("CHONKER_LOG").is_ok() {
        let path = std::env::var("CHONKER_LOG_FILE")
            .unwrap_or_else(|_| "/tmp/chonker7.log".to_string());

        match OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
        {
            Ok(file) => Mutex::new(Some(file)),
            Err(_) => Mutex::new(None),
        }
    } else {
        Mutex::new(None)
    }
});

#[macro_export]
macro_rules! log_debug {
    ($($arg:tt)*) => {
        $crate::logger::write_log(&format!($($arg)*))
    };
}

pub fn write_log(msg: &str) {
    if let Ok(mut guard) = LOG_FILE.lock() {
        if let Some(ref mut file) = *guard {
            let timestamp = chrono::Local::now().format("%H:%M:%S%.3f");
            writeln!(file, "[{}] {}", timestamp, msg).ok();
            file.flush().ok();
        }
    }
}

pub fn log_coordinate_event(
    event: &str,
    screen: (u16, u16),
    pane: (usize, usize),
    document: (usize, usize),
    viewport: (usize, usize),
) {
    write_log(&format!(
        "{}: Screen({},{}) → Pane({},{}) + Viewport({},{}) = Doc({},{})",
        event, screen.0, screen.1, pane.0, pane.1,
        viewport.0, viewport.1, document.0, document.1
    ));
}