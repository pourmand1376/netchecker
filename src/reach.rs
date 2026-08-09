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
//!
//! We race several ports because a host that's up may silently drop packets on
//! one port (no RST) while answering on another. A host is reachable if ANY
//! port connects or refuses; it's only unreachable if every port fails.

use std::io::ErrorKind;
use std::net::{IpAddr, SocketAddr};
use std::time::{Duration, Instant};

use futures::future::join_all;
use tokio::net::TcpStream;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(2);

pub struct ReachResult {
    pub ok: bool,
    pub detail: String,
    pub ms: u128,
}

/// Per-port connect outcome, reduced across the raced ports.
enum PortOutcome {
    Up,       // connected or refused — the host answered
    TimedOut, // no answer within the window
    Error,    // other error (e.g. no route)
}

async fn probe_port(addr: SocketAddr) -> PortOutcome {
    match tokio::time::timeout(CONNECT_TIMEOUT, TcpStream::connect(addr)).await {
        Ok(Ok(_)) => PortOutcome::Up,
        Ok(Err(e)) if e.kind() == ErrorKind::ConnectionRefused => PortOutcome::Up,
        Ok(Err(_)) => PortOutcome::Error,
        Err(_) => PortOutcome::TimedOut,
    }
}

/// Is `ip` reachable? Races TCP connects across `ports`. Reachable if any port
/// connects or refuses (the host answered); unreachable only if all fail.
pub async fn reachable(ip: IpAddr, ports: &[u16]) -> ReachResult {
    let start = Instant::now();
    let outcomes = join_all(ports.iter().map(|&p| probe_port(SocketAddr::new(ip, p)))).await;
    let ms = start.elapsed().as_millis();

    if outcomes.iter().any(|o| matches!(o, PortOutcome::Up)) {
        ReachResult {
            ok: true,
            detail: "reachable".into(),
            ms,
        }
    } else if outcomes.iter().all(|o| matches!(o, PortOutcome::TimedOut)) {
        ReachResult {
            ok: false,
            detail: "unreachable or timed out".into(),
            ms,
        }
    } else {
        ReachResult {
            ok: false,
            detail: "unreachable (no route)".into(),
            ms,
        }
    }
}
