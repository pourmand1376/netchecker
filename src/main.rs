//! netchecker — cross-platform internet reachability checker.
//!
//! Probes local infrastructure (interface, gateway, raw IP routing), DNS
//! resolution, and a set of domestic / global / filtered websites in parallel,
//! and reports PASS/FAIL. Direct mode tests the raw ISP connection; proxy mode
//! routes global/filtered sites through the system proxy.

mod dns;
mod probe;
mod proxy;
mod reach;
mod sites;
mod sysinfo;
mod ui;
mod wifi;

use std::collections::{HashMap, HashSet};
use std::io::Write;
use std::net::{IpAddr, Ipv4Addr};
use std::time::Instant;

use clap::Parser;
use futures::future::join_all;

use ui::{Status, CYAN, GREEN, RED, RESET, YELLOW};

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
    /// Mode: "direct" (raw ISP) or "proxy" (via system proxy). Prompts if omitted.
    mode: Option<String>,
    /// Direct mode: bypass proxies and test the raw ISP connection.
    #[arg(short, long, conflicts_with = "proxy")]
    direct: bool,
    /// Proxy mode: route global/filtered sites through the system proxy.
    #[arg(short, long)]
    proxy: bool,
}

fn choose_mode(cli: &Cli) -> Mode {
    if cli.direct || matches!(cli.mode.as_deref(), Some("direct") | Some("d")) {
        return Mode::Direct;
    }
    if cli.proxy || matches!(cli.mode.as_deref(), Some("proxy") | Some("p")) {
        return Mode::Proxy;
    }
    // No explicit choice: prompt (default to proxy on EOF / non-interactive).
    println!("{CYAN}Select Network Check Mode:{RESET}");
    println!("  {GREEN}[1]{RESET} Proxy Mode (use system proxy for global/filtered sites)");
    println!("  {YELLOW}[2]{RESET} Direct Mode (bypass proxies and test the raw ISP connection)");
    print!("Enter choice (1 or 2): ");
    let _ = std::io::stdout().flush();
    let mut line = String::new();
    match std::io::stdin().read_line(&mut line) {
        Ok(_) if line.trim() == "2" => Mode::Direct,
        _ => Mode::Proxy,
    }
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let mode = choose_mode(&cli);
    let proxy: Option<String> = if mode == Mode::Proxy {
        proxy::detect()
    } else {
        None
    };

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
                let r = reach::reachable(IpAddr::V4(gw), 80).await;
                Status::new(
                    format!("Ping Default Gateway ({gw})"),
                    r.ok,
                    format!("{} — {} ms", r.detail, r.ms),
                )
            }
            None => Status::new("Ping Default Gateway", false, "No gateway IP found"),
        }
    };
    let raw_fut = async {
        let r = reach::reachable(IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1)), 443).await;
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
    let resolved: HashMap<String, (Option<Ipv4Addr>, u128)> =
        join_all(host_list.iter().map(|h| dns::resolve_timed(&resolver, h.clone())))
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
