# netchecker

A fast, cross-platform internet reachability checker. It probes your local
network infrastructure, DNS resolution, and a set of domestic / global /
filtered websites **in parallel**, and reports each as `PASS` / `FAIL` / `INFO`
with timings — in about 1–2 seconds total.

It was built for diagnosing a specific, annoying situation: "my internet is
_technically_ up, but which parts actually work — the ISP, DNS, the proxy, the
filtered sites?" It's especially useful behind a VPN/proxy where the answer
differs per route.

```
===============================================
   netchecker — Network Standpoint Test
   Running Mode:  DIRECT
   Local IP:      192.168.1.6
   DNS resolvers: 192.168.1.1
   Per-probe timeout: 2 seconds
===============================================
[*] Dispatching parallel probes...

[+] Infrastructure Standpoints:
  [PASS] Local Network Interface        - Active on en0 (192.168.1.6) — Wi-Fi SSID: home
  [PASS] Ping Default Gateway (192.168.1.1) - reachable — 2 ms
  [PASS] Ping Raw Global IP (1.1.1.1)   - Global IP routing up — 4 ms

[+] DNS Resolution:
  [PASS] www.google.com                 - Resolved to 142.251.153.119 — 21 ms
  ...

[*] Domestic Websites (DIRECT Route):
  [PASS] digikala.com                   - HTTP 301 — 119 ms
  ...
```

## Install

```sh
cargo install netchecker
```

Or build from source:

```sh
git clone https://github.com/pourmand1376/netchecker
cd netchecker
cargo build --release
./target/release/netchecker
```

## Usage

```sh
netchecker            # prompts for mode
netchecker direct     # test the raw ISP connection, bypassing proxies
netchecker proxy      # route global/filtered sites through the system proxy
netchecker -d         # short flags also work (-d / -p)
```

- **Direct mode** pins every request to a pre-resolved IP and bypasses all
  proxies — it tests your raw ISP path.
- **Proxy mode** sends the global/filtered sites through your system proxy
  (auto-detected). Domestic sites always go direct.

## How it works (and why)

- **DNS that can't wedge.** On macOS, `getaddrinfo()` goes through
  mDNSResponder; when the local resolver is bad or was just changed, that daemon
  serialises lookups and collapses "parallel" probes into a long stall.
  netchecker resolves names itself with [`hickory-resolver`], which talks to the
  configured nameservers directly over UDP/53 and never touches the OS resolver.
  A bad lookup fails fast instead of hanging everything.

- **IP-pinned probes.** On the direct route, each HTTP probe is handed the
  pre-resolved IP so it never calls the OS resolver — the same anti-wedge
  guarantee, end to end. Redirects aren't followed on the direct route (a
  cross-host redirect would trigger a fresh OS lookup); a `3xx` still counts as
  reachable.

- **Reachability without root.** No ICMP (which needs elevated privileges).
  netchecker uses TCP connects and races a few common ports — a
  `ConnectionRefused` still proves the host answered, so it counts as reachable.
  Only a host that fails on *every* port is reported down.

- **True egress detection.** The active interface is found by asking the kernel
  which local address it would use to reach the internet (a UDP `connect` that
  sends nothing), not by a heuristic. This gets the right answer on split-tunnel
  VPNs, where the "default route" and the real internet path differ.

## Platform support

The core — DNS bypass, HTTP probes, interface/gateway detection, TCP
reachability — is native Rust and works the same on all three platforms:

| Feature                        | macOS | Linux | Windows |
| ------------------------------ | :---: | :---: | :-----: |
| Infrastructure / DNS / probes  |  ✅   |  ✅   |   ✅    |
| Egress interface + gateway     |  ✅   |  ✅   |   ✅    |
| System proxy detection         |  ✅¹  |  env² |  env²   |
| Wi-Fi SSID                     |  ⚠️³  |  ⚠️⁴ |   ⚠️⁵   |

1. Reads `scutil --proxy` (catches GUI-configured proxies) plus env vars.
2. Reads `HTTPS_PROXY` / `ALL_PROXY` / `HTTP_PROXY` env vars.
3. Best-effort via `ipconfig` / `networksetup`; on macOS 14+ the SSID is gated
   behind Location Services permission and may come back empty.
4. Best-effort via `nmcli` / `iwgetid` (NetworkManager).
5. Best-effort via `netsh wlan show interfaces`.

SSID is the one genuinely fragile piece — there is no dependable cross-platform
API for it. When it can't be read, netchecker shows "SSID unavailable or wired"
rather than failing.

## Customising the site list

The domestic / global / filtered lists live in [`src/sites.rs`](src/sites.rs).
Edit them to match the sites you care about and rebuild.

## License

MIT — see [LICENSE](LICENSE).
