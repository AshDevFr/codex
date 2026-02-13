//! Rate limiting service using token bucket algorithm
//!
//! Provides per-client rate limiting for API endpoints with support for:
//! - Anonymous users (by IP address)
//! - Authenticated users (by user ID)
//! - Configurable burst limits and refill rates
//! - Background cleanup of stale buckets

use dashmap::DashMap;
use std::hash::Hash;
use std::net::IpAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time::interval;
use tokio_util::sync::CancellationToken;
use tracing::{debug, trace};
use uuid::Uuid;

use crate::config::RateLimitConfig;

/// Client identifier for rate limiting
#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub enum ClientId {
    /// Anonymous request identified by IP address
    Ip(IpAddr),
    /// Authenticated request identified by user ID
    User(Uuid),
}

impl std::fmt::Display for ClientId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ClientId::Ip(ip) => write!(f, "ip:{}", ip),
            ClientId::User(id) => write!(f, "user:{}", id),
        }
    }
}

/// Token bucket state for a client
#[derive(Clone, Debug)]
pub struct TokenBucket {
    /// Current number of available tokens
    tokens: f64,
    /// Last time tokens were refilled
    last_refill: Instant,
    /// Maximum tokens (bucket capacity / burst size)
    max_tokens: u32,
    /// Tokens added per second (refill rate)
    refill_rate: f64,
}

impl TokenBucket {
    /// Create a new bucket with given capacity and refill rate
    ///
    /// # Arguments
    /// * `max_tokens` - Maximum tokens (burst size)
    /// * `refill_rate` - Tokens added per second (requests per second)
    pub fn new(max_tokens: u32, refill_rate: f64) -> Self {
        Self {
            tokens: max_tokens as f64,
            last_refill: Instant::now(),
            max_tokens,
            refill_rate,
        }
    }

    /// Refill tokens based on elapsed time since last refill
    fn refill(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill).as_secs_f64();
        let new_tokens = elapsed * self.refill_rate;

        self.tokens = (self.tokens + new_tokens).min(self.max_tokens as f64);
        self.last_refill = now;
    }

    /// Try to consume a token, refilling based on elapsed time
    ///
    /// Returns true if a token was consumed, false if bucket is empty
    pub fn try_consume(&mut self) -> bool {
        self.refill();

        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }

    /// Get current token count after refill
    pub fn available_tokens(&mut self) -> u32 {
        self.refill();
        self.tokens as u32
    }

    /// Get seconds until a token will be available
    pub fn seconds_until_token(&self) -> f64 {
        if self.tokens >= 1.0 {
            return 0.0;
        }

        let tokens_needed = 1.0 - self.tokens;
        tokens_needed / self.refill_rate
    }

    /// Check if bucket is stale (no activity for TTL)
    pub fn is_stale(&self, ttl: Duration) -> bool {
        self.last_refill.elapsed() > ttl
    }
}

/// Result of a rate limit check
#[derive(Clone, Debug)]
pub struct RateLimitResult {
    /// Whether the request is allowed
    pub allowed: bool,
    /// Remaining requests available
    pub remaining: u32,
    /// Seconds until a token will be available (0 if allowed)
    pub retry_after_secs: u64,
    /// Total limit (burst size) for this client type
    pub limit: u32,
}

/// Rate limiter service using token bucket algorithm
///
/// Thread-safe service that tracks request rates per client using DashMap.
/// Supports different limits for anonymous and authenticated users.
#[derive(Clone)]
pub struct RateLimiterService {
    /// Token buckets indexed by client identifier
    buckets: Arc<DashMap<ClientId, TokenBucket>>,
    /// Configuration
    config: Arc<RateLimitConfig>,
}

impl RateLimiterService {
    /// Create a new rate limiter service
    pub fn new(config: Arc<RateLimitConfig>) -> Self {
        Self {
            buckets: Arc::new(DashMap::new()),
            config,
        }
    }

    /// Get the appropriate limits for a client type
    fn get_limits(&self, client_id: &ClientId) -> (u32, f64) {
        match client_id {
            ClientId::Ip(_) => (
                self.config.anonymous_burst,
                self.config.anonymous_rps as f64,
            ),
            ClientId::User(_) => (
                self.config.authenticated_burst,
                self.config.authenticated_rps as f64,
            ),
        }
    }

    /// Check if request is allowed, consuming a token if so
    ///
    /// Returns a `RateLimitResult` with the decision and rate limit headers info.
    pub fn check_rate_limit(&self, client_id: ClientId) -> RateLimitResult {
        let (max_tokens, refill_rate) = self.get_limits(&client_id);

        let mut entry = self
            .buckets
            .entry(client_id.clone())
            .or_insert_with(|| TokenBucket::new(max_tokens, refill_rate));

        let bucket = entry.value_mut();
        let allowed = bucket.try_consume();
        let remaining = bucket.available_tokens();
        let retry_after_secs = if allowed {
            0
        } else {
            bucket.seconds_until_token().ceil() as u64
        };

        trace!(
            client = %client_id,
            allowed,
            remaining,
            retry_after_secs,
            "Rate limit check"
        );

        RateLimitResult {
            allowed,
            remaining,
            retry_after_secs,
            limit: max_tokens,
        }
    }

    /// Get current state for a client without consuming a token
    ///
    /// Returns `None` if the client has no bucket (never made a request).
    #[allow(dead_code)] // Useful for monitoring/debugging
    pub fn get_state(&self, client_id: &ClientId) -> Option<RateLimitResult> {
        let (max_tokens, _refill_rate) = self.get_limits(client_id);

        self.buckets.get_mut(client_id).map(|mut entry| {
            let bucket = entry.value_mut();
            bucket.refill();
            let remaining = bucket.available_tokens();

            RateLimitResult {
                allowed: remaining > 0,
                remaining,
                retry_after_secs: if remaining > 0 {
                    0
                } else {
                    bucket.seconds_until_token().ceil() as u64
                },
                limit: max_tokens,
            }
        })
    }

    /// Check if rate limiting is enabled
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Check if a path is exempt from rate limiting
    ///
    /// Note: This is a simplified check using `starts_with`. The actual middleware
    /// uses compiled `GlobSet` patterns for efficient glob matching.
    #[allow(dead_code)] // Useful for monitoring/debugging
    pub fn is_path_exempt(&self, path: &str) -> bool {
        self.config
            .exempt_paths
            .iter()
            .any(|exempt| path.starts_with(exempt))
    }

    /// Get the number of active buckets (for monitoring)
    #[allow(dead_code)] // Useful for monitoring/debugging
    pub fn bucket_count(&self) -> usize {
        self.buckets.len()
    }

    /// Cleanup stale buckets that haven't been accessed within TTL
    ///
    /// Returns the number of buckets removed.
    fn cleanup_stale_buckets(&self) -> usize {
        let ttl = Duration::from_secs(self.config.bucket_ttl_secs);
        let before = self.buckets.len();

        self.buckets
            .retain(|_client_id, bucket| !bucket.is_stale(ttl));

        let removed = before.saturating_sub(self.buckets.len());
        if removed > 0 {
            debug!(
                removed,
                remaining = self.buckets.len(),
                "Cleaned up stale rate limit buckets"
            );
        }
        removed
    }

    /// Start background cleanup task
    ///
    /// Periodically removes stale buckets to prevent unbounded memory growth.
    /// Accepts a `CancellationToken` for graceful shutdown support.
    /// Returns a `JoinHandle` that can be awaited on shutdown.
    pub fn start_background_cleanup(
        self: Arc<Self>,
        cancel_token: CancellationToken,
    ) -> tokio::task::JoinHandle<()> {
        let cleanup_interval = Duration::from_secs(self.config.cleanup_interval_secs);

        tokio::spawn(async move {
            let mut tick = interval(cleanup_interval);

            loop {
                tokio::select! {
                    _ = cancel_token.cancelled() => {
                        debug!("Rate limiter cleanup task shutting down");
                        break;
                    }
                    _ = tick.tick() => {
                        self.cleanup_stale_buckets();
                    }
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr};
    use std::time::Duration;

    fn test_config() -> Arc<RateLimitConfig> {
        Arc::new(RateLimitConfig {
            enabled: true,
            anonymous_rps: 10,
            anonymous_burst: 5, // Small burst for easier testing
            authenticated_rps: 50,
            authenticated_burst: 10, // Small burst for easier testing
            exempt_paths: vec![
                "/health".to_string(),
                "/api/v1/events".to_string(),
                "/api/v1/events/**".to_string(),
            ],
            cleanup_interval_secs: 1,
            bucket_ttl_secs: 2,
        })
    }

    fn test_ip() -> ClientId {
        ClientId::Ip(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)))
    }

    fn test_user() -> ClientId {
        ClientId::User(Uuid::new_v4())
    }

    // ===================
    // TokenBucket tests
    // ===================

    #[test]
    fn test_token_bucket_new() {
        let bucket = TokenBucket::new(10, 5.0);
        assert_eq!(bucket.max_tokens, 10);
        assert_eq!(bucket.refill_rate, 5.0);
        assert_eq!(bucket.tokens, 10.0);
    }

    #[test]
    fn test_token_bucket_allows_burst() {
        let mut bucket = TokenBucket::new(5, 1.0);

        // Should allow exactly burst size requests
        for i in 0..5 {
            assert!(bucket.try_consume(), "Request {} should be allowed", i);
        }
    }

    #[test]
    fn test_token_bucket_blocks_after_burst() {
        let mut bucket = TokenBucket::new(5, 1.0);

        // Consume all tokens
        for _ in 0..5 {
            assert!(bucket.try_consume());
        }

        // Next request should be blocked
        assert!(!bucket.try_consume());
    }

    #[test]
    fn test_token_bucket_refills_over_time() {
        let mut bucket = TokenBucket::new(5, 10.0); // 10 tokens per second

        // Consume all tokens
        for _ in 0..5 {
            assert!(bucket.try_consume());
        }
        assert!(!bucket.try_consume());

        // Simulate time passing by adjusting last_refill
        bucket.last_refill = Instant::now() - Duration::from_millis(200); // 0.2 seconds = 2 tokens

        // Should have refilled some tokens
        let available = bucket.available_tokens();
        assert!(
            available >= 1,
            "Expected at least 1 token, got {}",
            available
        );
    }

    #[test]
    fn test_token_bucket_doesnt_exceed_max() {
        let mut bucket = TokenBucket::new(5, 100.0); // Very fast refill

        // Simulate lots of time passing
        bucket.last_refill = Instant::now() - Duration::from_secs(10);

        // Should cap at max_tokens
        let available = bucket.available_tokens();
        assert_eq!(available, 5);
    }

    #[test]
    fn test_token_bucket_seconds_until_token() {
        let mut bucket = TokenBucket::new(5, 2.0); // 2 tokens per second

        // Fresh bucket has tokens available
        assert_eq!(bucket.seconds_until_token(), 0.0);

        // Consume all tokens
        for _ in 0..5 {
            bucket.try_consume();
        }

        // Should take 0.5 seconds for next token
        let wait = bucket.seconds_until_token();
        assert!(
            wait > 0.0 && wait <= 0.6,
            "Expected ~0.5 seconds, got {}",
            wait
        );
    }

    #[test]
    fn test_bucket_is_stale() {
        let mut bucket = TokenBucket::new(5, 1.0);

        // Fresh bucket is not stale
        assert!(!bucket.is_stale(Duration::from_secs(1)));

        // Simulate time passing
        bucket.last_refill = Instant::now() - Duration::from_secs(10);

        // Now it should be stale
        assert!(bucket.is_stale(Duration::from_secs(5)));
    }

    // ===================
    // ClientId tests
    // ===================

    #[test]
    fn test_client_id_display() {
        let ip = ClientId::Ip(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)));
        assert_eq!(format!("{}", ip), "ip:192.168.1.1");

        let user_id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let user = ClientId::User(user_id);
        assert_eq!(
            format!("{}", user),
            "user:550e8400-e29b-41d4-a716-446655440000"
        );
    }

    #[test]
    fn test_client_id_equality() {
        let ip1 = ClientId::Ip(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)));
        let ip2 = ClientId::Ip(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)));
        let ip3 = ClientId::Ip(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 2)));

        assert_eq!(ip1, ip2);
        assert_ne!(ip1, ip3);

        let user_id = Uuid::new_v4();
        let user1 = ClientId::User(user_id);
        let user2 = ClientId::User(user_id);
        let user3 = ClientId::User(Uuid::new_v4());

        assert_eq!(user1, user2);
        assert_ne!(user1, user3);

        // Different types are not equal
        assert_ne!(ip1, user1);
    }

    // ===================
    // RateLimiterService tests
    // ===================

    #[test]
    fn test_rate_limiter_allows_requests_within_limit() {
        let service = RateLimiterService::new(test_config());
        let client = test_ip();

        // First 5 requests should be allowed (burst size)
        for i in 0..5 {
            let result = service.check_rate_limit(client.clone());
            assert!(result.allowed, "Request {} should be allowed", i);
            assert_eq!(result.limit, 5);
        }
    }

    #[test]
    fn test_rate_limiter_blocks_after_burst() {
        let service = RateLimiterService::new(test_config());
        let client = test_ip();

        // Exhaust burst
        for _ in 0..5 {
            service.check_rate_limit(client.clone());
        }

        // Next request should be blocked
        let result = service.check_rate_limit(client.clone());
        assert!(!result.allowed);
        assert!(result.retry_after_secs > 0);
        assert_eq!(result.remaining, 0);
    }

    #[test]
    fn test_rate_limiter_different_limits_for_anonymous_vs_authenticated() {
        let service = RateLimiterService::new(test_config());

        // Anonymous gets 5 burst
        let anon = test_ip();
        let anon_result = service.check_rate_limit(anon);
        assert_eq!(anon_result.limit, 5);

        // Authenticated gets 10 burst
        let auth = test_user();
        let auth_result = service.check_rate_limit(auth);
        assert_eq!(auth_result.limit, 10);
    }

    #[test]
    fn test_rate_limiter_separate_buckets_per_client() {
        let service = RateLimiterService::new(test_config());

        let client1 = ClientId::Ip(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)));
        let client2 = ClientId::Ip(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 2)));

        // Exhaust client1's bucket
        for _ in 0..5 {
            service.check_rate_limit(client1.clone());
        }

        // client1 is blocked
        let result1 = service.check_rate_limit(client1.clone());
        assert!(!result1.allowed);

        // client2 still has tokens
        let result2 = service.check_rate_limit(client2);
        assert!(result2.allowed);
    }

    #[test]
    fn test_rate_limiter_get_state() {
        let service = RateLimiterService::new(test_config());
        let client = test_ip();

        // No state for new client
        assert!(service.get_state(&client).is_none());

        // After a request, state exists
        service.check_rate_limit(client.clone());
        let state = service.get_state(&client);
        assert!(state.is_some());
        let state = state.unwrap();
        assert!(state.allowed);
        assert!(state.remaining <= 5);
    }

    #[test]
    fn test_rate_limiter_is_path_exempt() {
        let service = RateLimiterService::new(test_config());

        assert!(service.is_path_exempt("/health"));
        assert!(service.is_path_exempt("/health/check"));
        assert!(service.is_path_exempt("/api/v1/events"));
        assert!(service.is_path_exempt("/api/v1/events/stream"));

        assert!(!service.is_path_exempt("/api/v1/books"));
        assert!(!service.is_path_exempt("/api/v1/users"));
        assert!(!service.is_path_exempt("/opds/catalog"));
    }

    #[test]
    fn test_rate_limiter_is_enabled() {
        let config = Arc::new(RateLimitConfig {
            enabled: true,
            ..RateLimitConfig::default()
        });
        let service = RateLimiterService::new(config);
        assert!(service.is_enabled());

        let config = Arc::new(RateLimitConfig {
            enabled: false,
            ..RateLimitConfig::default()
        });
        let service = RateLimiterService::new(config);
        assert!(!service.is_enabled());
    }

    #[test]
    fn test_rate_limiter_bucket_count() {
        let service = RateLimiterService::new(test_config());

        assert_eq!(service.bucket_count(), 0);

        let client1 = ClientId::Ip(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)));
        let client2 = ClientId::Ip(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 2)));

        service.check_rate_limit(client1);
        assert_eq!(service.bucket_count(), 1);

        service.check_rate_limit(client2);
        assert_eq!(service.bucket_count(), 2);
    }

    #[test]
    fn test_rate_limiter_cleanup_stale_buckets() {
        let config = Arc::new(RateLimitConfig {
            enabled: true,
            anonymous_rps: 10,
            anonymous_burst: 5,
            authenticated_rps: 50,
            authenticated_burst: 10,
            exempt_paths: vec![],
            cleanup_interval_secs: 1,
            bucket_ttl_secs: 0, // Immediate staleness for testing
        });
        let service = RateLimiterService::new(config);

        // Create some buckets
        let client1 = ClientId::Ip(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)));
        let client2 = ClientId::Ip(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 2)));

        service.check_rate_limit(client1);
        service.check_rate_limit(client2);
        assert_eq!(service.bucket_count(), 2);

        // With TTL of 0, all buckets should be immediately stale
        let removed = service.cleanup_stale_buckets();
        assert_eq!(removed, 2);
        assert_eq!(service.bucket_count(), 0);
    }

    #[test]
    fn test_rate_limiter_cleanup_preserves_fresh_buckets() {
        let config = Arc::new(RateLimitConfig {
            enabled: true,
            anonymous_rps: 10,
            anonymous_burst: 5,
            authenticated_rps: 50,
            authenticated_burst: 10,
            exempt_paths: vec![],
            cleanup_interval_secs: 1,
            bucket_ttl_secs: 3600, // 1 hour TTL
        });
        let service = RateLimiterService::new(config);

        // Create a bucket
        let client = test_ip();
        service.check_rate_limit(client);
        assert_eq!(service.bucket_count(), 1);

        // Cleanup should preserve fresh buckets
        let removed = service.cleanup_stale_buckets();
        assert_eq!(removed, 0);
        assert_eq!(service.bucket_count(), 1);
    }

    // ===================
    // RateLimitResult tests
    // ===================

    #[test]
    fn test_rate_limit_result_remaining_decreases() {
        let service = RateLimiterService::new(test_config());
        let client = test_ip();

        let result1 = service.check_rate_limit(client.clone());
        let result2 = service.check_rate_limit(client.clone());

        assert!(result2.remaining < result1.remaining);
    }

    // ===================
    // Background cleanup tests (async)
    // ===================

    #[tokio::test]
    async fn test_background_cleanup_graceful_shutdown() {
        let config = Arc::new(RateLimitConfig {
            enabled: true,
            anonymous_rps: 10,
            anonymous_burst: 5,
            authenticated_rps: 50,
            authenticated_burst: 10,
            exempt_paths: vec![],
            cleanup_interval_secs: 60, // Won't trigger during test
            bucket_ttl_secs: 300,
        });
        let service = Arc::new(RateLimiterService::new(config));

        let cancel_token = CancellationToken::new();
        let handle = service
            .clone()
            .start_background_cleanup(cancel_token.clone());

        // Create a bucket
        let client = test_ip();
        service.check_rate_limit(client);
        assert_eq!(service.bucket_count(), 1);

        // Cancel and wait for shutdown
        cancel_token.cancel();
        let result = tokio::time::timeout(Duration::from_secs(5), handle).await;
        assert!(result.is_ok(), "Background task should complete");
    }
}
