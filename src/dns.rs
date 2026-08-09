//! DNS resolution that bypasses the operating system's resolver.
//!
//! Why this matters (and why the original tool shelled out to `dig`): on macOS,
//! `getaddrinfo()` goes through mDNSResponder. When the local resolver is bad or
//! was just changed, that daemon wedges and serialises every lookup, collapsing
//! "parallel" probes into a long sequential stall. `hickory-resolver` is a pure
//! Rust resolver that talks to the configured nameservers directly over UDP/53,
//! so it never touches getaddrinfo/mDNSResponder and a wedged system resolver
//! can't stall us — a bad lookup just fails fast under our timeout.

use std::net::{IpAddr, Ipv4Addr};
use std::time::{Duration, Instant};

use hickory_resolver::config::{ResolverConfig, ResolverOpts};
use hickory_resolver::{TokioAsyncResolver, system_conf};

const LOOKUP_TIMEOUT: Duration = Duration::from_secs(2);

/// Build a resolver from the system's configured nameservers, falling back to
/// Cloudflare if the system config can't be read. We resolve against the local
/// resolvers on purpose: for the domestic/filtered sites, resolving through the
/// ISP's DNS is exactly the behaviour we want to observe.
pub fn build() -> TokioAsyncResolver {
    let (config, mut opts) = system_conf::read_system_conf()
        .unwrap_or_else(|_| (ResolverConfig::cloudflare(), ResolverOpts::default()));
    opts.timeout = LOOKUP_TIMEOUT;
    opts.attempts = 1; // fail fast; no retries stacking up under the wedge
    opts.cache_size = 0; // a diagnostic wants fresh answers, not cached ones
    TokioAsyncResolver::tokio(config, opts)
}

/// The nameservers the system is configured to use, de-duplicated, for display.
pub fn configured_servers() -> Vec<IpAddr> {
    let Ok((config, _)) = system_conf::read_system_conf() else {
        return Vec::new();
    };
    let mut seen = Vec::new();
    for ns in config.name_servers() {
        let ip = ns.socket_addr.ip();
        if !seen.contains(&ip) {
            seen.push(ip);
        }
    }
    seen
}

/// Resolve one host to an IPv4 address, timing the lookup.
///
/// Returns `(host, Some(ip)|None, elapsed_ms)`. `None` means the lookup failed
/// or timed out — that host's probe is then reported as unresolved rather than
/// being allowed to stall everything else.
pub async fn resolve_timed(
    resolver: &TokioAsyncResolver,
    host: String,
) -> (String, Option<Ipv4Addr>, u128) {
    let start = Instant::now();
    let ip = match resolver.ipv4_lookup(format!("{host}.")).await {
        Ok(lookup) => lookup.iter().next().map(|a| a.0),
        Err(_) => None,
    };
    (host, ip, start.elapsed().as_millis())
}
