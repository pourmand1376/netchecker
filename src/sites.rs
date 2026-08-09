//! Site probe definitions and the domestic / global / filtered lists.

use std::net::Ipv4Addr;

#[derive(Clone)]
pub struct Probe {
    pub label: String,
    pub url: String,
    /// Whether this probe should go through the system proxy (when in proxy
    /// mode). Domestic sites are always tested on the direct route.
    pub use_proxy: bool,
    /// Pre-resolved IP for the direct route, filled in before probing so curl's
    /// equivalent (reqwest) never has to call the OS resolver.
    pub ip: Option<Ipv4Addr>,
}

impl Probe {
    fn new(label: &str, url: &str, use_proxy: bool) -> Self {
        Probe {
            label: label.to_string(),
            url: url.to_string(),
            use_proxy,
            ip: None,
        }
    }

    /// The hostname portion of the URL (everything between `://` and the next `/`).
    pub fn host(&self) -> &str {
        let after_scheme = self.url.split("://").nth(1).unwrap_or(&self.url);
        after_scheme.split('/').next().unwrap_or(after_scheme)
    }
}

/// Hosts shown explicitly in the DNS Resolution section.
pub const DNS_HOSTS: &[&str] = &["digikala.com", "motamem.org", "www.google.com", "soft98.ir"];

pub struct Probes {
    pub domestic: Vec<Probe>,
    pub global: Vec<Probe>,
    pub filtered: Vec<Probe>,
}

impl Probes {
    pub fn all(&self) -> impl Iterator<Item = &Probe> {
        self.domestic
            .iter()
            .chain(self.global.iter())
            .chain(self.filtered.iter())
    }
}

/// Build the probe lists for the selected mode. In proxy mode, global/filtered
/// sites route through the proxy; domestic sites always go direct.
pub fn build(proxy_mode: bool) -> Probes {
    let p = proxy_mode;
    Probes {
        domestic: vec![
            Probe::new("digikala.com", "https://digikala.com", false),
            Probe::new("iranketab.ir", "https://www.iranketab.ir", false),
            Probe::new("soft98.ir", "https://soft98.ir", false),
            Probe::new("varzesh3.com", "https://www.varzesh3.com", false),
        ],
        global: vec![
            Probe::new("Wikipedia", "https://www.wikipedia.org", p),
            Probe::new("Substack", "https://substack.com", p),
            Probe::new("Apple", "https://www.apple.com", p),
            Probe::new("Google", "https://www.google.com", p),
            Probe::new("Motamem (motamem.org)", "https://motamem.org", p),
        ],
        filtered: vec![
            Probe::new("YouTube", "https://www.youtube.com", p),
            Probe::new("Telegram", "https://t.me", p),
            Probe::new("Instagram", "https://www.instagram.com", p),
            Probe::new("Twitter / X", "https://x.com", p),
            Probe::new("Facebook", "https://www.facebook.com", p),
        ],
    }
}
