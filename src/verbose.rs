use std::sync::atomic::{AtomicBool, Ordering};

static VERBOSE: AtomicBool = AtomicBool::new(false);

/// Enable verbose output. Call once at startup.
pub fn enable() {
    VERBOSE.store(true, Ordering::Relaxed);
}

/// Check whether verbose output is enabled.
pub fn is_enabled() -> bool {
    VERBOSE.load(Ordering::Relaxed)
}

/// Print a status message to stderr, but only when `--verbose` is active.
///
/// Usage is identical to `eprintln!`:
/// ```ignore
/// verbose!("crgx: downloading from {}...", url);
/// ```
macro_rules! verbose {
    ($($arg:tt)*) => {
        if $crate::verbose::is_enabled() {
            eprintln!($($arg)*);
        }
    };
}

pub(crate) use verbose;
