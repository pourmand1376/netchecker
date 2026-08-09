//! Terminal colors and status formatting.
//!
//! ANSI escapes work on macOS/Linux terminals and on Windows 10+ terminals
//! (Windows Terminal, modern conhost with VT processing). We keep this
//! dependency-free rather than pulling a coloring crate.

pub const RED: &str = "\x1b[0;31m";
pub const GREEN: &str = "\x1b[0;32m";
pub const YELLOW: &str = "\x1b[1;33m";
pub const CYAN: &str = "\x1b[0;36m";
pub const RESET: &str = "\x1b[0m";

/// A single check result: name, pass/fail, and a human detail string.
pub struct Status {
    pub name: String,
    pub ok: bool,
    pub detail: String,
}

impl Status {
    pub fn new(name: impl Into<String>, ok: bool, detail: impl Into<String>) -> Self {
        Status {
            name: name.into(),
            ok,
            detail: detail.into(),
        }
    }

    pub fn print(&self) {
        let tag = if self.ok {
            format!("{GREEN}[PASS]{RESET}")
        } else {
            format!("{RED}[FAIL]{RESET}")
        };
        println!("  {tag} {:<35} - {}", self.name, self.detail);
    }
}
