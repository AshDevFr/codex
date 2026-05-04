//! Per-host backoff for release-source polls.
//!
//! Plugins that share a domain (rare today, but possible in the future when,
//! e.g., a Nyaa scraper and a Nyaa RSS plugin both hit `nyaa.si`) need to
//! cooperate on rate-limit signals. The signal is observed by the polling
//! task (via `ReleasePollResponse.upstream_status` or an RPC error) and
//! converted into a per-domain backoff multiplier that the scheduler
//! consults when picking the next poll time.
//!
//! Implementation details:
//!
//! - Backoff state is in-memory only. A scheduler restart clears it; the
//!   next poll will hit the upstream cleanly. We could persist it, but
//!   429/503 signals are typically minutes-fresh, not hours-fresh.
//! - State is keyed by `host` (lowercased), which we extract via a small
//!   parser to avoid pulling in the `url` crate. Inputs we expect (`https://nyaa.si/...`,
//!   `nyaa.si`, etc.) all extract identically.
//! - Multiplier doubles per consecutive failure (1.0 → 2.0 → 4.0 → … → cap).
//! - On success the multiplier resets to 1.0 immediately.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::RwLock;

/// Maximum backoff multiplier. Caps the doubling growth so a long stretch
/// of 503s doesn't push the next poll to "next month."
pub const MAX_BACKOFF_MULTIPLIER: f64 = 16.0;

/// Maximum age we trust a backoff signal for. After this elapses, the
/// stored multiplier is treated as 1.0 (a multiplier "expires").
///
/// Set to 24h: longer than any reasonable upstream rate-limit window, but
/// short enough that a stale signal can't permanently throttle a source.
pub const BACKOFF_MAX_AGE: Duration = Duration::from_secs(24 * 3_600);

/// The two status codes we treat as backoff signals.
pub const HTTP_TOO_MANY_REQUESTS: u16 = 429;
pub const HTTP_SERVICE_UNAVAILABLE: u16 = 503;

/// Returns true if a status code should trigger backoff growth.
pub fn is_backoff_status(status: u16) -> bool {
    matches!(status, HTTP_TOO_MANY_REQUESTS | HTTP_SERVICE_UNAVAILABLE)
}

/// Tracker for per-host backoff multipliers. Cheap to clone — wraps an
/// `Arc<RwLock<...>>` internally so the scheduler and polling tasks see a
/// shared view.
#[derive(Debug, Clone, Default)]
pub struct HostBackoff {
    inner: Arc<RwLock<HashMap<String, BackoffEntry>>>,
}

#[derive(Debug, Clone, Copy)]
struct BackoffEntry {
    multiplier: f64,
    updated_at: Instant,
}

impl HostBackoff {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a successful poll for `url`. Resets the host's multiplier to 1.0.
    /// Tolerates `url` being a host or a full URL.
    pub async fn record_success(&self, url_or_host: &str) {
        let host = match host_of(url_or_host) {
            Some(h) => h,
            None => return,
        };
        self.inner.write().await.remove(&host);
    }

    /// Record an HTTP error for `url`. If `status` is a recognized backoff
    /// signal, doubles the host's multiplier (capped). Other statuses are
    /// no-ops at this layer.
    pub async fn record_http_error(&self, url_or_host: &str, status: u16) {
        if !is_backoff_status(status) {
            return;
        }
        let host = match host_of(url_or_host) {
            Some(h) => h,
            None => return,
        };
        let mut guard = self.inner.write().await;
        let entry = guard.entry(host).or_insert(BackoffEntry {
            multiplier: 1.0,
            updated_at: Instant::now(),
        });
        entry.multiplier = (entry.multiplier * 2.0).min(MAX_BACKOFF_MULTIPLIER);
        if entry.multiplier < 2.0 {
            // First failure starts at 2.0, not 1.0 — we want immediate
            // visible delay on the first 429/503.
            entry.multiplier = 2.0;
        }
        entry.updated_at = Instant::now();
    }

    /// Return the current multiplier for `url`. `1.0` when there's no
    /// backoff active or the entry has expired (`> BACKOFF_MAX_AGE`).
    pub async fn multiplier(&self, url_or_host: &str) -> f64 {
        let host = match host_of(url_or_host) {
            Some(h) => h,
            None => return 1.0,
        };
        let guard = self.inner.read().await;
        match guard.get(&host) {
            Some(entry) if entry.updated_at.elapsed() <= BACKOFF_MAX_AGE => entry.multiplier,
            _ => 1.0,
        }
    }
}

/// Extract a normalized host from `url_or_host`. Returns `None` for empty
/// inputs or strings that don't look hostlike.
///
/// This is intentionally tiny: we don't need full RFC 3986 parsing —
/// callers feed us either a bare host or `scheme://host[:port]/path`.
pub fn host_of(url_or_host: &str) -> Option<String> {
    let s = url_or_host.trim();
    if s.is_empty() {
        return None;
    }
    // Strip scheme.
    let after_scheme = match s.split_once("://") {
        Some((_, rest)) => rest,
        None => s,
    };
    // Strip path/query/fragment — first '/', '?', '#' wins.
    let host_with_port = after_scheme.split(['/', '?', '#']).next().unwrap_or("");
    if host_with_port.is_empty() {
        return None;
    }
    // Strip port (rightmost ':' that isn't inside brackets — IPv6 caveat
    // is fine to ignore for our use case).
    let host = host_with_port
        .rsplit_once(':')
        .map(|(h, _)| h)
        .unwrap_or(host_with_port);
    if host.is_empty() {
        return None;
    }
    Some(host.to_ascii_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_of_extracts_from_url() {
        assert_eq!(
            host_of("https://nyaa.si/?q=foo").as_deref(),
            Some("nyaa.si")
        );
        assert_eq!(
            host_of("HTTP://Example.com/path").as_deref(),
            Some("example.com")
        );
        assert_eq!(
            host_of("http://localhost:8080/x").as_deref(),
            Some("localhost")
        );
    }

    #[test]
    fn host_of_accepts_bare_host() {
        assert_eq!(host_of("nyaa.si").as_deref(), Some("nyaa.si"));
        assert_eq!(host_of("  Foo.Bar  ").as_deref(), Some("foo.bar"));
    }

    #[test]
    fn host_of_handles_empty_and_garbage() {
        assert!(host_of("").is_none());
        assert!(host_of("   ").is_none());
        assert!(host_of("://").is_none());
    }

    #[tokio::test]
    async fn no_backoff_by_default() {
        let b = HostBackoff::new();
        assert_eq!(b.multiplier("https://nyaa.si/").await, 1.0);
    }

    #[tokio::test]
    async fn first_429_jumps_to_two() {
        let b = HostBackoff::new();
        b.record_http_error("https://nyaa.si/", 429).await;
        assert_eq!(b.multiplier("https://nyaa.si/").await, 2.0);
    }

    #[tokio::test]
    async fn consecutive_failures_double_until_cap() {
        let b = HostBackoff::new();
        for _ in 0..10 {
            b.record_http_error("https://nyaa.si/", 503).await;
        }
        assert_eq!(b.multiplier("nyaa.si").await, MAX_BACKOFF_MULTIPLIER);
    }

    #[tokio::test]
    async fn success_resets_backoff() {
        let b = HostBackoff::new();
        b.record_http_error("https://nyaa.si/", 429).await;
        b.record_http_error("https://nyaa.si/", 429).await;
        assert!(b.multiplier("nyaa.si").await > 1.0);

        b.record_success("https://nyaa.si/").await;
        assert_eq!(b.multiplier("nyaa.si").await, 1.0);
    }

    #[tokio::test]
    async fn unrelated_host_is_unaffected() {
        let b = HostBackoff::new();
        b.record_http_error("https://nyaa.si/", 429).await;
        assert_eq!(b.multiplier("mangaupdates.com").await, 1.0);
    }

    #[tokio::test]
    async fn non_backoff_status_is_ignored() {
        let b = HostBackoff::new();
        b.record_http_error("https://nyaa.si/", 500).await;
        b.record_http_error("https://nyaa.si/", 404).await;
        assert_eq!(b.multiplier("nyaa.si").await, 1.0);
    }

    #[test]
    fn is_backoff_status_recognizes_429_and_503() {
        assert!(is_backoff_status(429));
        assert!(is_backoff_status(503));
        assert!(!is_backoff_status(200));
        assert!(!is_backoff_status(500));
    }
}
