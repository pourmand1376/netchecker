# netchecker

![App-image](app-image.webp)

A fast, cross-platform internet reachability checker. It probes your local
network infrastructure, DNS, and a set of domestic / global / filtered websites
**in parallel**, reporting each as `PASS` / `FAIL` / `INFO` with timings — in
about 1–2 seconds.

Built for one annoying question: "my internet is _technically_ up, but which
parts actually work — the ISP, DNS, the proxy, the filtered sites?" Especially
useful behind a VPN/proxy, where the answer differs per route.

```
===============================================
   netchecker — Network Standpoint Test
   Running Mode:  DIRECT
   Local IP:      192.168.1.6
   DNS resolvers: 192.168.1.1
===============================================

[+] Infrastructure Standpoints:
  [PASS] Local Network Interface        - Active on en0 (192.168.1.6) — Wi-Fi SSID: home
  [PASS] Ping Default Gateway (192.168.1.1) - reachable — 2 ms
  [PASS] Ping Raw Global IP (1.1.1.1)   - Global IP routing up — 4 ms

[+] DNS Resolution:
  [PASS] www.google.com                 - Resolved to 142.251.153.119 — 21 ms

[*] Domestic Websites (DIRECT Route):
  [PASS] digikala.com                   - HTTP 301 — 119 ms
```

## Install

The recommended way is [`cargo binstall`](https://github.com/cargo-bins/cargo-binstall),
which downloads a prebuilt binary from the GitHub Releases — no compiling:

```sh
cargo binstall netchecker
```

Don't have it yet? `cargo install cargo-binstall` (or grab it from its releases page).

Other options:

```sh
# Homebrew (macOS / Linux)
brew install pourmand1376/tap/netchecker

# Cargo (compile from source)
cargo install netchecker

# Nix
nix run github:pourmand1376/netchecker

# Debian / Ubuntu — grab the .deb from the Releases page
sudo apt install ./netchecker_*.deb
```

Or build from source: `git clone`, then `cargo build --release`.

## Usage

```sh
netchecker        # auto: uses the system proxy if one is set, otherwise direct
netchecker -d     # force direct — bypass proxies, test the raw ISP path
netchecker -p     # force proxy — route global/filtered sites via the system proxy
```

With no arguments it picks the mode automatically; there is no prompt, so it's
safe in scripts and cron. Domestic sites always use the direct route.

## How it works

- **DNS that can't wedge.** A bad or just-changed macOS resolver makes
  `getaddrinfo()` (via mDNSResponder) serialise lookups and stall every "parallel"
  probe. netchecker resolves names itself with `hickory-resolver`, straight to the
  configured nameservers over UDP/53 — the OS resolver is never in the path, so a
  bad lookup fails fast instead of hanging everything.
- **IP-pinned probes.** On the direct route each HTTP probe is handed its
  pre-resolved IP, so it never calls the OS resolver either. Redirects aren't
  followed on that route (a cross-host redirect would re-trigger a lookup); a
  `3xx` still counts as reachable.
- **Reachability without root.** No ICMP. TCP connects race a few common ports; a
  `ConnectionRefused` proves the host answered, so it counts as reachable. Only a
  host that fails on *every* port is reported down.
- **True egress detection.** The active interface is whichever local address the
  kernel would actually use to reach the internet (found via a UDP `connect` that
  sends nothing), not a heuristic — correct even on split-tunnel VPNs.

## Platform support

The core — DNS bypass, HTTP probes, interface/gateway detection, TCP
reachability — is native Rust and identical on all three platforms:

| Feature                       | macOS | Linux | Windows |
| ----------------------------- | :---: | :---: | :-----: |
| Infrastructure / DNS / probes |  ✅   |  ✅   |   ✅    |
| Egress interface + gateway    |  ✅   |  ✅   |   ✅    |
| System proxy detection        |  ✅¹  |  env² |  env²   |
| Wi-Fi SSID                    |  ⚠️³  |  ⚠️⁴ |   ⚠️⁵   |

1. `scutil --proxy` (catches GUI-configured proxies) plus env vars.
2. `HTTPS_PROXY` / `ALL_PROXY` / `HTTP_PROXY` env vars.
3. Best-effort via `ipconfig` / `networksetup`; on macOS 14+ the SSID needs
   Location Services permission and may come back empty.
4. Best-effort via `nmcli` / `iwgetid`.
5. Best-effort via `netsh wlan show interfaces`.

SSID is the one genuinely fragile piece — there's no dependable cross-platform
API. When it can't be read, netchecker shows "SSID unavailable or wired" rather
than failing.

## Customising the site list

The domestic / global / filtered lists live in
[`src/sites.rs`](src/sites.rs) — edit and rebuild.

## License

MIT — see [LICENSE](LICENSE).
