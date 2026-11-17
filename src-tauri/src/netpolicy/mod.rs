//! Outbound network policy for CKL builds.
//!
//! Termimus only talks to the SSH hosts the user configures, the self-hosted
//! sync relay, and (optionally) an AI endpoint. This module enforces the two
//! rules that keep company data from leaving the network:
//!
//! 1. **AI endpoints must be self-hosted** — loopback or private-network
//!    addresses only (`ensure_private_endpoint`, `PrivateOnlyResolver`). Public
//!    cloud AI providers are rejected, because AI requests carry terminal
//!    output, host labels and usernames.
//! 2. **Sync must use TLS** — `https://` only, with plain `http://` allowed for
//!    loopback testing (`require_secure_sync_url`). The bundle itself is
//!    end-to-end encrypted, but the bearer token and device metadata are not.
//!
//! Hostnames are checked by resolving them and requiring *every* resolved
//! address to be private. `PrivateOnlyResolver` repeats that check at connect
//! time so a DNS answer that changes between check and connect (DNS rebinding)
//! cannot redirect a request to a public address.

use reqwest::dns::{Addrs, Name, Resolve, Resolving};
use reqwest::Url;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

const POLICY_PREFIX: &str = "Blocked by CKL data policy";

/// True for addresses that cannot route to the public internet: loopback,
/// RFC 1918 private IPv4, and IPv6 unique-local (`fc00::/7`). Link-local,
/// CGNAT (`100.64.0.0/10`), unspecified and every public range are rejected.
pub fn is_private_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => is_private_v4(v4),
        IpAddr::V6(v6) => {
            if let Some(v4) = v6.to_ipv4_mapped() {
                return is_private_v4(v4);
            }
            v6.is_loopback() || is_unique_local_v6(v6)
        }
    }
}

fn is_private_v4(ip: Ipv4Addr) -> bool {
    ip.is_loopback() || ip.is_private()
}

fn is_unique_local_v6(ip: Ipv6Addr) -> bool {
    (ip.segments()[0] & 0xfe00) == 0xfc00
}

fn is_loopback_host(host: &str) -> bool {
    if host.eq_ignore_ascii_case("localhost") {
        return true;
    }
    parse_ip_host(host).is_some_and(|ip| match ip {
        IpAddr::V4(v4) => v4.is_loopback(),
        IpAddr::V6(v6) => v6.is_loopback() || v6.to_ipv4_mapped().is_some_and(|m| m.is_loopback()),
    })
}

/// `Url::host_str` wraps IPv6 literals in brackets.
fn parse_ip_host(host: &str) -> Option<IpAddr> {
    host.trim_start_matches('[').trim_end_matches(']').parse().ok()
}

fn parse_http_url(raw: &str, what: &str) -> Result<Url, String> {
    let url = Url::parse(raw.trim()).map_err(|e| format!("Invalid {what} URL: {e}"))?;
    match url.scheme() {
        "http" | "https" => {}
        other => return Err(format!("Invalid {what} URL: unsupported scheme '{other}'")),
    }
    if url.host_str().is_none_or(str::is_empty) {
        return Err(format!("Invalid {what} URL: missing host"));
    }
    Ok(url)
}

fn public_address_error(host: &str, ip: IpAddr) -> String {
    format!(
        "{POLICY_PREFIX}: AI endpoint '{host}' resolves to public address {ip}. \
         Only self-hosted AI endpoints on localhost or a private network \
         (10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16, fc00::/7) are allowed."
    )
}

/// Synchronous check usable when saving a provider: valid http(s) URL, and
/// an IP-literal host must be private. Hostnames are resolved later by
/// [`ensure_private_endpoint`] / [`PrivateOnlyResolver`].
pub fn check_private_endpoint_syntax(raw: &str) -> Result<(), String> {
    let url = parse_http_url(raw, "AI endpoint")?;
    let host = url.host_str().unwrap_or_default();
    if let Some(ip) = parse_ip_host(host) {
        if !is_private_ip(ip) {
            return Err(public_address_error(host, ip));
        }
    }
    Ok(())
}

/// Resolves `host` and returns its addresses only if every one is private.
async fn resolve_private(host: &str) -> Result<Vec<SocketAddr>, String> {
    if let Some(ip) = parse_ip_host(host) {
        return if is_private_ip(ip) {
            Ok(vec![SocketAddr::new(ip, 0)])
        } else {
            Err(public_address_error(host, ip))
        };
    }
    let addrs: Vec<SocketAddr> = tokio::net::lookup_host((host, 0))
        .await
        .map_err(|e| format!("Could not resolve AI endpoint '{host}': {e}"))?
        .collect();
    if addrs.is_empty() {
        return Err(format!("Could not resolve AI endpoint '{host}': no addresses"));
    }
    if let Some(public) = addrs.iter().find(|a| !is_private_ip(a.ip())) {
        return Err(public_address_error(host, public.ip()));
    }
    Ok(addrs)
}

/// Full pre-flight check for an AI request: URL syntax plus DNS resolution.
/// Gives the user a clear error before any connection is attempted.
pub async fn ensure_private_endpoint(raw: &str) -> Result<(), String> {
    let url = parse_http_url(raw, "AI endpoint")?;
    resolve_private(url.host_str().unwrap_or_default()).await.map(|_| ())
}

/// `reqwest` DNS resolver that only ever hands out private addresses. Install
/// it on any client that must not reach the public internet. IP-literal URLs
/// bypass DNS, so callers must still run [`ensure_private_endpoint`] first.
pub struct PrivateOnlyResolver;

impl Resolve for PrivateOnlyResolver {
    fn resolve(&self, name: Name) -> Resolving {
        let host = name.as_str().to_string();
        Box::pin(async move {
            let addrs = resolve_private(&host).await?;
            Ok::<Addrs, Box<dyn std::error::Error + Send + Sync>>(Box::new(addrs.into_iter()))
        })
    }
}

/// Sync relay URLs must be `https://`; plain `http://` is accepted only for
/// loopback (local testing against `docker compose up`).
pub fn require_secure_sync_url(raw: &str) -> Result<(), String> {
    let url = parse_http_url(raw, "sync server")?;
    if url.scheme() == "https" {
        return Ok(());
    }
    let host = url.host_str().unwrap_or_default();
    if is_loopback_host(host) {
        return Ok(());
    }
    Err(format!(
        "{POLICY_PREFIX}: the sync server must use https:// (got http://{host}). \
         Put the relay behind a TLS reverse proxy; plain http:// is only allowed for localhost."
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ip(s: &str) -> IpAddr {
        s.parse().unwrap()
    }

    #[test]
    fn classifies_private_and_public_addresses() {
        for private in [
            "127.0.0.1", "127.9.9.9", "10.1.2.3", "172.16.0.1", "172.31.255.255",
            "192.168.1.10", "::1", "fd12:3456::1", "fc00::1", "::ffff:10.0.0.1",
        ] {
            assert!(is_private_ip(ip(private)), "{private} should be private");
        }
        for public in [
            "8.8.8.8", "1.1.1.1", "172.32.0.1", "172.15.255.255", "100.64.0.1",
            "169.254.169.254", "0.0.0.0", "2606:4700::1111", "fe80::1", "::ffff:8.8.8.8", "::",
        ] {
            assert!(!is_private_ip(ip(public)), "{public} should be rejected");
        }
    }

    #[test]
    fn syntax_check_rejects_public_literals_and_bad_schemes() {
        assert!(check_private_endpoint_syntax("http://localhost:11434/v1").is_ok());
        assert!(check_private_endpoint_syntax("http://192.168.1.5:8000").is_ok());
        assert!(check_private_endpoint_syntax("https://[fd00::5]/v1").is_ok());
        // Hostnames pass syntax and are enforced at resolution time.
        assert!(check_private_endpoint_syntax("https://llm.internal/v1").is_ok());

        assert!(check_private_endpoint_syntax("http://8.8.8.8/v1").is_err());
        assert!(check_private_endpoint_syntax("http://[2606:4700::1111]/v1").is_err());
        // Non-dotted IPv4 forms are normalised by the URL parser, not bypassed.
        assert!(check_private_endpoint_syntax("http://134744072/v1").is_err());
        assert!(check_private_endpoint_syntax("ftp://localhost").is_err());
        assert!(check_private_endpoint_syntax("not a url").is_err());
    }

    #[tokio::test]
    async fn ensure_private_endpoint_checks_literals_without_dns() {
        assert!(ensure_private_endpoint("http://127.0.0.1:11434").await.is_ok());
        let err = ensure_private_endpoint("https://1.1.1.1/v1").await.unwrap_err();
        assert!(err.starts_with(POLICY_PREFIX), "{err}");
    }

    #[tokio::test]
    async fn localhost_resolves_as_private() {
        assert!(ensure_private_endpoint("http://localhost:11434/v1").await.is_ok());
    }

    #[test]
    fn sync_requires_tls_except_loopback() {
        assert!(require_secure_sync_url("https://sync.example.com").is_ok());
        assert!(require_secure_sync_url("https://10.0.0.5:8443").is_ok());
        assert!(require_secure_sync_url("http://localhost:8080").is_ok());
        assert!(require_secure_sync_url("http://127.0.0.1:8080/").is_ok());
        assert!(require_secure_sync_url("http://[::1]:8080").is_ok());

        assert!(require_secure_sync_url("http://sync.example.com").is_err());
        assert!(require_secure_sync_url("http://192.168.1.20:8080").is_err());
        assert!(require_secure_sync_url("http://localhost.evil.com").is_err());
        assert!(require_secure_sync_url("ws://localhost:8080").is_err());
        assert!(require_secure_sync_url("").is_err());
    }
}
