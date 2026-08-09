//! Reachability checks via TCP connect instead of ICMP.
//!
//! Raw ICMP (ping) needs root on Linux/macOS and admin on Windows, which is a
//! terrible UX for a casual diagnostic. TCP connect needs no privileges and is
//! arguably a better reachability signal.
//!
//! The key insight: a `ConnectionRefused` (RST) proves the host is alive and
//! routable — it answered. Only a *timeout* or "no route" means the host is
//! actually unreachable. So we treat refused as reachable and only real
//! timeouts/errors as down.

use std::io::ErrorKind;
use std::net::{IpAddr, SocketAddr};
use std::time::{Duration, Instant};

use tokio::net::TcpStream;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(2);

pub struct ReachResult {
    pub ok: bool,
    pub detail: String,
    pub ms: u128,
}

/// Is `ip` reachable? Attempts a TCP connect to `port`; a refusal still counts
/// as reachable (the host answered). Only a timeout counts as unreachable.
pub async fn reachable(ip: IpAddr, port: u16) -> ReachResult {
    let addr = SocketAddr::new(ip, port);
    let start = Instant::now();
    let outcome = tokio::time::timeout(CONNECT_TIMEOUT, TcpStream::connect(addr)).await;
    let ms = start.elapsed().as_millis();

    match outcome {
        Ok(Ok(_)) => ReachResult {
            ok: true,
            detail: "reachable".into(),
            ms,
        },
        // Host answered with a reset — it's up, the port is just closed.
        Ok(Err(e)) if e.kind() == ErrorKind::ConnectionRefused => ReachResult {
            ok: true,
            detail: "reachable (port closed, host answered)".into(),
            ms,
        },
        Ok(Err(e)) => ReachResult {
            ok: false,
            detail: format!("unreachable ({})", e.kind()),
            ms,
        },
        Err(_) => ReachResult {
            ok: false,
            detail: "unreachable or timed out".into(),
            ms,
        },
    }
}
