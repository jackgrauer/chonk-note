
#[macro_export]
macro_rules! debug_log {
    ($($arg:tt)*) => {
        #[cfg(debug_assertions)]
        {
            if $crate::debug::is_debug_enabled() {
                if let Ok(mut file) = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open("/Users/jack/chonker7_debug.log")
                {
                    writeln!(file, $($arg)*).ok();
                }
            }
        }
    };
}