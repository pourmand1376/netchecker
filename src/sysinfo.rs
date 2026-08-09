//! System network facts: active interface, gateway, local IP, DNS resolvers.
//!
//! DNS resolvers come from `hickory-resolver`'s system-config reader; interface
//! and gateway from the `netdev` crate — no shelling out to `route`, `ipconfig`,
//! or `scutil`.
//!
//! The active interface is NOT taken from `netdev::get_default_interface()`.
//! That heuristic mis-picks on split-tunnel VPNs (it returned a `utun` tunnel
//! over the real `en0` on a test machine). Instead we ask the kernel directly:
//! open a UDP socket and `connect()` it toward a public IP — no packets are
//! sent — then read back the local address the routing table chose. That is the
//! true egress IP for internet traffic, full-tunnel or split-tunnel, on every
//! OS. We then match that IP to a `netdev` interface to recover its name and
//! gateway.

use std::net::{IpAddr, Ipv4Addr, UdpSocket};

/// Everything we can learn about the local network standpoint.
pub struct SysInfo {
    pub interface: Option<String>,
    pub gateway: Option<Ipv4Addr>,
    pub local_ip: Option<Ipv4Addr>,
    pub dns_servers: Vec<IpAddr>,
}

/// Ask the kernel which local IPv4 it would use to reach the internet.
/// `connect` on a UDP socket sends nothing; it just resolves the route.
fn egress_ipv4() -> Option<Ipv4Addr> {
    let sock = UdpSocket::bind("0.0.0.0:0").ok()?;
    sock.connect("1.1.1.1:80").ok()?;
    match sock.local_addr().ok()?.ip() {
        IpAddr::V4(v4) if !v4.is_unspecified() => Some(v4),
        _ => None,
    }
}

pub fn gather() -> SysInfo {
    let egress = egress_ipv4();
    let interfaces = netdev::get_interfaces();

    // The interface that actually owns the egress IP is the active one.
    let active = egress.and_then(|ip| {
        interfaces
            .iter()
            .find(|i| i.ipv4.iter().any(|net| net.addr() == ip))
    });

    // Fall back to netdev's default guess only if the egress match failed
    // (e.g. offline, or the egress IP isn't on an enumerated interface).
    let fallback;
    let iface = match active {
        Some(i) => Some(i),
        None => {
            fallback = netdev::get_default_interface().ok();
            fallback.as_ref()
        }
    };

    let interface = iface.map(|i| {
        // Windows exposes a human-friendly name; prefer it when present.
        match i.friendly_name.as_deref() {
            Some(fname) if !fname.is_empty() => fname.to_string(),
            _ => i.name.clone(),
        }
    });

    let gateway = iface
        .and_then(|i| i.gateway.as_ref())
        .and_then(|g| g.ipv4.first().copied());

    let local_ip = egress.or_else(|| iface.and_then(|i| i.ipv4.first().map(|net| net.addr())));

    SysInfo {
        interface,
        gateway,
        local_ip,
        dns_servers: crate::dns::configured_servers(),
    }
}
