//! netchecker — cross-platform internet reachability checker.
//!
//! Probes local infrastructure (interface, gateway, raw IP routing), DNS
//! resolution, and a set of domestic / global / filtered websites in parallel,
//! and reports PASS/FAIL. Direct mode tests the raw ISP connection; proxy mode
//! routes global/filtered sites through the system proxy.
//!
//! This crate ships as a binary (`netchecker`); the modules are exposed as a
//! library too so the pieces (DNS bypass, TCP reachability, site probes) can be
//! reused and documented. The CLI entry point is [`run`].

pub mod dns;
pub mod probe;
pub mod proxy;
pub mod reach;
pub mod sites;
pub mod sysinfo;
pub mod ui;
pub mod wifi;

use std::collections::{HashMap, HashSet};
use std::net::{IpAddr, Ipv4Addr};
use std::time::Instant;

use clap::Parser;
use futures::future::join_all;

use ui::{CYAN, RED, RESET, Status, YELLOW};

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Direct,
    Proxy,
}

impl Mode {
    fn label(self) -> &'static str {
        match self {
            Mode::Direct => "DIRECT",
            Mode::Proxy => "PROXY",
        }
    }
}

#[derive(Parser)]
#[command(
    name = "netchecker",
    about = "Cross-platform internet reachability checker",
    version
)]
struct Cli {
    /// Mode: "direct" (raw ISP) or "proxy" (via system proxy).
    /// Omit to auto-select: proxy if one is configured, otherwise direct.
    mode: Option<String>,
    /// Force direct mode: bypass proxies and test the raw ISP connection.
    #[arg(short, long, conflicts_with = "proxy")]
    direct: bool,
    /// Force proxy mode: route global/filtered sites through the system proxy.
    #[arg(short, long)]
    proxy: bool,
}

/// Decide the mode. Explicit flags/arg win; otherwise auto-select based on
/// whether a system proxy is configured — no interactive prompt.
fn choose_mode(cli: &Cli, proxy_detected: bool) -> Mode {
    if cli.direct || matches!(cli.mode.as_deref(), Some("direct") | Some("d")) {
        return Mode::Direct;
    }
    if cli.proxy || matches!(cli.mode.as_deref(), Some("proxy") | Some("p")) {
        return Mode::Proxy;
    }
    if proxy_detected {
        Mode::Proxy
    } else {
        Mode::Direct
    }
}

/// Run the full check: parse args, probe everything in parallel, print results.
pub async fn run() {
    let cli = Cli::parse();
    // Detect the system proxy up front so the mode can auto-select from it.
    let detected = proxy::detect();
    let mode = choose_mode(&cli, detected.is_some());
    let proxy: Option<String> = if mode == Mode::Proxy { detected } else { None };

    let info = sysinfo::gather();
    let resolver = dns::build();

    print_header(mode, &info, proxy.as_deref());

    let start = Instant::now();

    // --- Infrastructure -----------------------------------------------------
    println!("{YELLOW}[+] Infrastructure Standpoints:{RESET}");
    interface_status(&info).print();

    let gateway_fut = async {
        match info.gateway {
            Some(gw) => {
                let r = reach::reachable(IpAddr::V4(gw), &[80, 443, 53]).await;
                Status::new(
                    format!("Ping Default Gateway ({gw})"),
                    r.ok,
                    format!("{} — {} ms", r.detail, r.ms),
                )
            }
            // No gateway is normal for a point-to-point tunnel (VPN) egress:
            // traffic leaves via the tunnel directly, there's no LAN first hop
            // to ping. Not a failure — internet reachability is checked below.
            None => Status::info(
                "Default Gateway",
                "No LAN gateway (point-to-point/tunnel egress) — see raw IP check below",
            ),
        }
    };
    let raw_fut = async {
        let r = reach::reachable(IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1)), &[443]).await;
        let msg = if r.ok {
            "Global IP routing up"
        } else {
            "Global IP unreachable or timed out"
        };
        (
            Status::new(
                "Ping Raw Global IP (1.1.1.1)",
                r.ok,
                format!("{msg} — {} ms", r.ms),
            ),
            r.ok,
        )
    };

    let (gw_status, (raw_status, internet_ok)) = futures::join!(gateway_fut, raw_fut);
    gw_status.print();
    raw_status.print();

    // --- Build probes, then resolve every host we'll probe directly ---------
    let mut probes = sites::build(mode == Mode::Proxy);
    let proxy_active = proxy.is_some();

    // Direct-route hosts (proxy resolves its own), plus the reported DNS hosts.
    let mut hosts: HashSet<String> = HashSet::new();
    for p in probes.all() {
        if !(p.use_proxy && proxy_active) {
            hosts.insert(p.host().to_string());
        }
    }
    for h in sites::DNS_HOSTS {
        hosts.insert((*h).to_string());
    }

    let host_list: Vec<String> = hosts.into_iter().collect();
    let resolved: HashMap<String, (Option<Ipv4Addr>, u128)> = join_all(
        host_list
            .iter()
            .map(|h| dns::resolve_timed(&resolver, h.clone())),
    )
    .await
    .into_iter()
    .map(|(host, ip, ms)| (host, (ip, ms)))
    .collect();

    // --- DNS Resolution section --------------------------------------------
    println!("\n{YELLOW}[+] DNS Resolution:{RESET}");
    for host in sites::DNS_HOSTS {
        match resolved.get(*host) {
            Some((Some(ip), ms)) => {
                Status::new(*host, true, format!("Resolved to {ip} — {ms} ms")).print()
            }
            Some((None, ms)) => Status::new(
                *host,
                false,
                format!("Resolution failed or timed out — {ms} ms"),
            )
            .print(),
            None => Status::new(*host, false, "Not resolved").print(),
        }
    }

    // Pin resolved IPs onto the direct probes.
    for p in probes
        .domestic
        .iter_mut()
        .chain(probes.global.iter_mut())
        .chain(probes.filtered.iter_mut())
    {
        if !(p.use_proxy && proxy_active) {
            p.ip = resolved.get(p.host()).and_then(|(ip, _)| *ip);
        }
    }

    // --- Fire all site probes at once --------------------------------------
    let ordered: Vec<&sites::Probe> = probes.all().collect();
    let results: Vec<Status> = join_all(
        ordered
            .iter()
            .map(|p| probe::check_site(p, internet_ok, proxy.as_deref())),
    )
    .await;

    let d = probes.domestic.len();
    let g = probes.global.len();
    let ml = mode.label();

    println!("\n{CYAN}[*] Domestic Websites (DIRECT Route):{RESET}");
    for s in &results[..d] {
        s.print();
    }
    println!("\n{CYAN}[*] Global Web Standpoints ({ml} Route):{RESET}");
    for s in &results[d..d + g] {
        s.print();
    }
    println!("\n{CYAN}[*] Filtered Websites ({ml} Route):{RESET}");
    for s in &results[d + g..] {
        s.print();
    }

    let total = start.elapsed();
    println!("\n{YELLOW}==============================================={RESET}");
    println!(
        "   Total probe time: {CYAN}{} ms ({:.2} s){RESET}",
        total.as_millis(),
        total.as_secs_f64()
    );
    println!("{YELLOW}==============================================={RESET}\n");
}

fn interface_status(info: &sysinfo::SysInfo) -> Status {
    let Some(iface) = info.interface.as_deref() else {
        return Status::new(
            "Local Network Interface",
            false,
            "No active network interface",
        );
    };
    let mut detail = format!("Active on {iface}");
    if let Some(ip) = info.local_ip {
        detail.push_str(&format!(" ({ip})"));
    }
    match wifi::ssid(Some(iface)) {
        Some(ssid) => detail.push_str(&format!(" — Wi-Fi SSID: {ssid}")),
        None => detail.push_str(" — SSID unavailable or wired"),
    }
    Status::new("Local Network Interface", true, detail)
}

fn print_header(mode: Mode, info: &sysinfo::SysInfo, proxy: Option<&str>) {
    let ip = info
        .local_ip
        .map(|i| i.to_string())
        .unwrap_or_else(|| "unknown".into());
    let servers = if info.dns_servers.is_empty() {
        "none".to_string()
    } else {
        info.dns_servers
            .iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>()
            .join(", ")
    };

    println!("\n{YELLOW}==============================================={RESET}");
    println!("{YELLOW}   netchecker — Network Standpoint Test        {RESET}");
    println!("   Running Mode:  {CYAN}{}{RESET}", mode.label());
    println!("   Local IP:      {CYAN}{ip}{RESET}");
    println!("   DNS resolvers: {CYAN}{servers}{RESET}");
    if mode == Mode::Proxy {
        match proxy {
            Some(p) => println!("   System Proxy:  {CYAN}{p}{RESET}"),
            None => println!("   System Proxy:  {RED}Not detected; using direct route{RESET}"),
        }
    }
    println!("   Per-probe timeout: {CYAN}2 seconds{RESET}");
    println!("{YELLOW}==============================================={RESET}");
    println!("{CYAN}[*] Dispatching parallel probes...{RESET}\n");
}
