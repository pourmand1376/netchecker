//! System proxy detection.
//!
//! Fully cross-platform detection means reading a different source per OS:
//! SystemConfiguration on macOS, the registry/WinHTTP on Windows, env vars on
//! Linux. We cover the two that matter in practice:
//!
//!   * env vars (`HTTPS_PROXY`, `ALL_PROXY`, `HTTP_PROXY`) — works everywhere and
//!     is how most CLI setups declare a proxy;
//!   * `scutil --proxy` on macOS — matches the original tool's behaviour and
//!     catches GUI-configured system proxies that aren't in the environment.
//!
//! Returns a URL string that `reqwest::Proxy::all` accepts, or `None`.

/// Detect a usable proxy URL for the active system, or `None`.
pub fn detect() -> Option<String> {
    #[cfg(target_os = "macos")]
    if let Some(p) = macos_scutil_proxy() {
        return Some(p);
    }
    env_proxy()
}

/// SOCKS takes priority (socks5h keeps DNS on the proxy side), then HTTPS, HTTP.
fn env_proxy() -> Option<String> {
    for key in ["ALL_PROXY", "all_proxy"] {
        if let Ok(v) = std::env::var(key) {
            if !v.is_empty() {
                return Some(v);
            }
        }
    }
    for key in ["HTTPS_PROXY", "https_proxy", "HTTP_PROXY", "http_proxy"] {
        if let Ok(v) = std::env::var(key) {
            if !v.is_empty() {
                return Some(v);
            }
        }
    }
    None
}

#[cfg(target_os = "macos")]
fn macos_scutil_proxy() -> Option<String> {
    use std::process::Command;

    let out = Command::new("scutil").arg("--proxy").output().ok()?;
    let text = String::from_utf8_lossy(&out.stdout);

    let val = |key: &str| -> Option<String> {
        for line in text.lines() {
            let line = line.trim();
            if let Some(rest) = line.strip_prefix(key) {
                let rest = rest.trim_start();
                if let Some(rest) = rest.strip_prefix(':') {
                    return Some(rest.trim().to_string());
                }
            }
        }
        None
    };
    let enabled = |key: &str| val(key).as_deref() == Some("1");

    if enabled("SOCKSEnable") {
        if let (Some(h), Some(p)) = (val("SOCKSProxy"), val("SOCKSPort")) {
            return Some(format!("socks5h://{h}:{p}"));
        }
    }
    // macOS "HTTPS"/"HTTP" proxy entries describe an HTTP CONNECT proxy.
    for prefix in ["HTTPS", "HTTP"] {
        if enabled(&format!("{prefix}Enable")) {
            if let (Some(h), Some(p)) = (val(&format!("{prefix}Proxy")), val(&format!("{prefix}Port")))
            {
                return Some(format!("http://{h}:{p}"));
            }
        }
    }
    None
}
