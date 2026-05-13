// Aurora palette — ANSI 256 constants matching lab's aurora palette exactly.
// Source: lab/crates/lab/src/output/theme.rs
pub const SERVICE_NAME: u8 = 211; // pink        (255,175,215)
pub const ACCENT_PRIMARY: u8 = 39; // bright blue (41,182,246)
pub const TEXT_MUTED: u8 = 250; // light grey  (167,188,201)
pub const SUCCESS: u8 = 115; // teal        (125,211,199)
pub const WARN: u8 = 180; // amber       (198,163,107)
pub const ERROR: u8 = 174; // muted red   (199,132,144)

/// Wrap `text` in ANSI 256-colour foreground escape if `colorize` is true.
#[must_use]
pub fn paint(ansi256: u8, text: &str, colorize: bool) -> String {
    if colorize {
        format!("\x1b[38;5;{ansi256}m{text}\x1b[0m")
    } else {
        text.to_string()
    }
}

/// Wrap `text` in bold escape if `colorize` is true.
#[must_use]
pub fn bold(text: &str, colorize: bool) -> String {
    if colorize {
        format!("\x1b[1m{text}\x1b[0m")
    } else {
        text.to_string()
    }
}

/// Dim/faint escape for TRACE/DEBUG levels.
#[must_use]
pub fn dim(text: &str, colorize: bool) -> String {
    if colorize {
        format!("\x1b[2m{text}\x1b[0m")
    } else {
        text.to_string()
    }
}
