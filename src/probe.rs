//! HTTP site probes — the reqwest equivalent of the original `curl --resolve`.
//!
//! Two routes:
//!   * Direct: pin the pre-resolved IP so reqwest never calls the OS resolver
//!     (the anti-wedge guarantee), and do NOT follow redirects — a cross-host
//!     redirect would trigger a fresh OS lookup and reopen the wedge. A 3xx
//!     still counts as reachable.
//!   * Proxy: hand the request to the proxy, which resolves the name itself, so
//!     following redirects is safe.

use std::net::SocketAddr;
use std::time::{Duration, Instant};

use reqwest::redirect::Policy;

use crate::sites::Probe;
use crate::ui::Status;

const TIMEOUT: Duration = Duration::from_secs(2);
const HARDCAP: Duration = Duration::from_secs(3);

pub async fn check_site(probe: &Probe, internet_ok: bool, proxy: Option<&str>) -> Status {
    if !internet_ok {
        return Status::new(probe.label.clone(), false, "Skipped — no internet");
    }

    let host = probe.host();
    let via_proxy = probe.use_proxy && proxy.is_some();

    let mut builder = reqwest::Client::builder()
        .connect_timeout(TIMEOUT)
        .timeout(TIMEOUT)
        .user_agent("netchecker/0.1");

    if via_proxy {
        let proxy = proxy.unwrap();
        match reqwest::Proxy::all(proxy) {
            Ok(p) => builder = builder.proxy(p).redirect(Policy::limited(5)),
            Err(_) => {
                return Status::new(
                    probe.label.clone(),
                    false,
                    format!("Invalid proxy URL: {proxy}"),
                );
            }
        }
    } else {
        // Direct route: pin the IP, disable proxies, don't follow redirects.
        let Some(ip) = probe.ip else {
            return Status::new(
                probe.label.clone(),
                false,
                "Local DNS could not resolve host — skipped",
            );
        };
        builder = builder
            .no_proxy()
            .redirect(Policy::none())
            .resolve(host, SocketAddr::new(ip.into(), 443));
    }

    let client = match builder.build() {
        Ok(c) => c,
        Err(e) => return Status::new(probe.label.clone(), false, format!("client error: {e}")),
    };

    let start = Instant::now();
    let fut = client.head(&probe.url).send();
    let outcome = tokio::time::timeout(HARDCAP, fut).await;
    let ms = start.elapsed().as_millis();

    match outcome {
        Err(_) => Status::new(
            probe.label.clone(),
            false,
            format!("Timed out after {ms} ms"),
        ),
        Ok(Err(e)) => {
            let reason = if e.is_connect() {
                "Connection failed"
            } else if e.is_timeout() {
                "Timed out"
            } else {
                "Request failed"
            };
            Status::new(probe.label.clone(), false, format!("{reason} — {ms} ms"))
        }
        Ok(Ok(resp)) => {
            let code = resp.status().as_u16();
            if (200..400).contains(&code) {
                Status::new(probe.label.clone(), true, format!("HTTP {code} — {ms} ms"))
            } else {
                Status::new(
                    probe.label.clone(),
                    false,
                    format!("HTTP {code} (blocked/error) — {ms} ms"),
                )
            }
        }
    }
}
