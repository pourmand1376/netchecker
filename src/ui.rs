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

/// Outcome of a check. `Info` is for states that are neither pass nor fail —
/// e.g. no LAN gateway to ping because the egress is a point-to-point tunnel.
/// Reporting those as red failures would imply breakage where there is none.
#[derive(Clone, Copy, PartialEq)]
pub enum Outcome {
    Pass,
    Fail,
    Info,
}

/// A single check result: name, outcome, and a human detail string.
pub struct Status {
    pub name: String,
    pub outcome: Outcome,
    pub detail: String,
}

impl Status {
    /// Pass/fail result from a boolean.
    pub fn new(name: impl Into<String>, ok: bool, detail: impl Into<String>) -> Self {
        Status {
            name: name.into(),
            outcome: if ok { Outcome::Pass } else { Outcome::Fail },
            detail: detail.into(),
        }
    }

    /// A neutral, informational result (neither pass nor fail).
    pub fn info(name: impl Into<String>, detail: impl Into<String>) -> Self {
        Status {
            name: name.into(),
            outcome: Outcome::Info,
            detail: detail.into(),
        }
    }

    pub fn print(&self) {
        let tag = match self.outcome {
            Outcome::Pass => format!("{GREEN}[PASS]{RESET}"),
            Outcome::Fail => format!("{RED}[FAIL]{RESET}"),
            Outcome::Info => format!("{CYAN}[INFO]{RESET}"),
        };
        println!("  {tag} {:<35} - {}", self.name, self.detail);
    }
}
