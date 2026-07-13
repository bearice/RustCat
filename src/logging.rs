//! Minimal logging gate.
//!
//! RustCat is a long-lived tray app. When launched from a desktop entry its
//! stdout/stderr are wired to the systemd journal, so routine informational
//! prints (e.g. the per-second "CPU Usage: …" line) would spam the journal
//! forever. Only emit informational logs when the user is actually watching —
//! i.e. when stdout is a TTY — or when they explicitly opt in via the
//! `RUSTCAT_DEBUG` / `RUST_LOG` environment variable.
//!
//! Genuine errors stay on stderr unconditionally: they are rare and are
//! exactly what you want in the journal when something goes wrong.

use std::io::IsTerminal;
use std::sync::OnceLock;

static ENABLED: OnceLock<bool> = OnceLock::new();

/// `true` when informational logging should be emitted.
///
/// Resolved once and cached: the TTY/env state does not change for the
/// lifetime of the process.
pub fn enabled() -> bool {
    *ENABLED.get_or_init(|| {
        if std::io::stdout().is_terminal() {
            return true;
        }
        // Treat any non-empty value of RUSTCAT_DEBUG, or a RUST_LOG that
        // isn't "off"/empty, as an explicit opt-in.
        if let Ok(v) = std::env::var("RUSTCAT_DEBUG") {
            if !v.is_empty() && v != "0" && !v.eq_ignore_ascii_case("false") {
                return true;
            }
        }
        if let Ok(v) = std::env::var("RUST_LOG") {
            if !v.is_empty() && !v.eq_ignore_ascii_case("off") {
                return true;
            }
        }
        false
    })
}

/// Emit an informational line to stdout, but only when debug logging is
/// enabled (TTY or opt-in env var). Uses `print!` so callers control the
/// trailing newline.
#[macro_export]
macro_rules! debug {
    ($($arg:tt)*) => {
        if $crate::logging::enabled() {
            println!($($arg)*);
        }
    };
}