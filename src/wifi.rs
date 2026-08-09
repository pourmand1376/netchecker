//! Wi-Fi SSID — best-effort, per-OS.
//!
//! There is no dependable cross-platform way to read the SSID, so this is the
//! one genuinely fragile part of the tool. Each OS needs its own source, and on
//! macOS 14+ the SSID is gated behind Location Services permission (you may get
//! an empty result even on Wi-Fi). Callers must treat `None` as "unknown / wired
//! / not permitted", never as an error.

/// Return the current Wi-Fi SSID if we can determine it, else `None`.
/// `iface` is the active interface name, used where the OS query needs it.
pub fn ssid(iface: Option<&str>) -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        return macos_ssid(iface);
    }
    #[cfg(target_os = "linux")]
    {
        let _ = iface;
        return linux_ssid();
    }
    #[cfg(target_os = "windows")]
    {
        let _ = iface;
        return windows_ssid();
    }
    #[allow(unreachable_code)]
    {
        let _ = iface;
        None
    }
}

/// Accept a candidate SSID only if it's non-empty and not one of the tools'
/// error/"not associated"/"not a Wi-Fi interface" messages. Those tools print
/// their errors to stdout, so a naive parse would happily treat "Error
/// obtaining wireless information." as an SSID.
fn nonempty(s: String) -> Option<String> {
    let s = s.trim().to_string();
    if s.is_empty() {
        return None;
    }
    let low = s.to_ascii_lowercase();
    const REJECT: &[&str] = &[
        "error",
        "not a wi-fi",
        "not associated",
        "you are not",
        "off",
        "disabled",
    ];
    if REJECT.iter().any(|bad| low.contains(bad)) {
        return None;
    }
    Some(s)
}

#[cfg(target_os = "macos")]
fn macos_ssid(iface: Option<&str>) -> Option<String> {
    use std::process::Command;

    // `ipconfig getsummary <iface>` still reports SSID on modern macOS.
    if let Some(dev) = iface {
        if let Ok(out) = Command::new("ipconfig").args(["getsummary", dev]).output() {
            let text = String::from_utf8_lossy(&out.stdout);
            for line in text.lines() {
                let line = line.trim();
                if let Some(v) = line
                    .strip_prefix("SSID :")
                    .and_then(|rest| nonempty(rest.to_string()))
                {
                    return Some(v);
                }
            }
        }
        // Fallback: networksetup prints "Current Wi-Fi Network: <name>".
        if let Ok(out) = Command::new("networksetup")
            .args(["-getairportnetwork", dev])
            .output()
        {
            let text = String::from_utf8_lossy(&out.stdout);
            if let Some(rest) = text.split(": ").nth(1) {
                return nonempty(rest.to_string());
            }
        }
    }
    None
}

#[cfg(target_os = "linux")]
fn linux_ssid() -> Option<String> {
    use std::process::Command;

    // NetworkManager: the active Wi-Fi row is "yes:<ssid>".
    if let Ok(out) = Command::new("nmcli")
        .args(["-t", "-f", "active,ssid", "dev", "wifi"])
        .output()
    {
        let text = String::from_utf8_lossy(&out.stdout);
        for line in text.lines() {
            if let Some(v) = line
                .strip_prefix("yes:")
                .and_then(|rest| nonempty(rest.to_string()))
            {
                return Some(v);
            }
        }
    }
    // Fallback: iwgetid prints just the SSID.
    if let Ok(out) = Command::new("iwgetid").arg("-r").output() {
        return nonempty(String::from_utf8_lossy(&out.stdout).to_string());
    }
    None
}

#[cfg(target_os = "windows")]
fn windows_ssid() -> Option<String> {
    use std::process::Command;

    let out = Command::new("netsh")
        .args(["wlan", "show", "interfaces"])
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    for line in text.lines() {
        let line = line.trim();
        // Match "SSID" but not "BSSID".
        if !line.starts_with("SSID") || line.starts_with("BSSID") {
            continue;
        }
        if let Some(v) = line
            .split_once(':')
            .and_then(|(_, v)| nonempty(v.to_string()))
        {
            return Some(v);
        }
    }
    None
}
