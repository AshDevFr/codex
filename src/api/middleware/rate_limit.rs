//! Rate limiting middleware for Axum
//!
//! Implements token bucket rate limiting as a tower Layer/Service.
//! Supports per-IP (anonymous) and per-user (authenticated) rate limiting
//! with configurable burst sizes and refill rates.
//!
//! # Response Headers
//!
//! All responses include rate limit information:
//! - `X-RateLimit-Limit`: Maximum requests allowed (burst size)
//! - `X-RateLimit-Remaining`: Requests remaining in current window
//! - `X-RateLimit-Reset`: Seconds until a token will be available
//!
//! # 429 Too Many Requests
//!
//! When rate limited, returns:
//! ```json
//! {
//!   "error": "rate_limit_exceeded",
//!   "message": "Too many requests. Please retry after N seconds.",
//!   "retry_after": N
//! }
//! ```

use axum::{
    body::Body,
    extract::ConnectInfo,
    http::{header::AUTHORIZATION, HeaderMap, HeaderValue, Request, Response, StatusCode},
    response::IntoResponse,
    Json,
};
use futures::future::BoxFuture;
use serde::Serialize;
use std::{
    collections::HashSet,
    net::{IpAddr, SocketAddr},
    sync::Arc,
    task::{Context, Poll},
};
use tower::{Layer, Service};
use tracing::{debug, trace};
use uuid::Uuid;

use crate::services::rate_limiter::{ClientId, RateLimiterService};

/// Rate limit response headers
const HEADER_RATE_LIMIT: &str = "X-RateLimit-Limit";
const HEADER_RATE_REMAINING: &str = "X-RateLimit-Remaining";
const HEADER_RATE_RESET: &str = "X-RateLimit-Reset";
const HEADER_RETRY_AFTER: &str = "Retry-After";

/// Rate limit exceeded response body
#[derive(Debug, Serialize)]
struct RateLimitExceededResponse {
    error: String,
    message: String,
    retry_after: u64,
}

/// Rate limiting middleware layer
///
/// Wraps services to add rate limiting based on client identity.
/// Exempt paths bypass rate limiting entirely.
#[derive(Clone)]
pub struct RateLimitLayer {
    service: Arc<RateLimiterService>,
    exempt_paths: HashSet<String>,
}

impl RateLimitLayer {
    /// Create a new rate limit layer
    ///
    /// # Arguments
    /// * `service` - The rate limiter service instance
    /// * `exempt_paths` - Paths that bypass rate limiting (prefix matching)
    pub fn new(service: Arc<RateLimiterService>, exempt_paths: Vec<String>) -> Self {
        Self {
            service,
            exempt_paths: exempt_paths.into_iter().collect(),
        }
    }
}

impl<S> Layer<S> for RateLimitLayer {
    type Service = RateLimitMiddleware<S>;

    fn layer(&self, inner: S) -> Self::Service {
        RateLimitMiddleware {
            inner,
            rate_limiter: self.service.clone(),
            exempt_paths: self.exempt_paths.clone(),
        }
    }
}

/// Rate limiting middleware service
///
/// Implements the tower Service trait to intercept requests and apply rate limiting.
#[derive(Clone)]
pub struct RateLimitMiddleware<S> {
    inner: S,
    rate_limiter: Arc<RateLimiterService>,
    exempt_paths: HashSet<String>,
}

impl<S> RateLimitMiddleware<S> {
    /// Extract client IP address from request headers and connection info
    ///
    /// Priority:
    /// 1. X-Forwarded-For header (first IP in chain)
    /// 2. X-Real-IP header
    /// 3. Connection socket address
    fn extract_ip_from_request(headers: &HeaderMap, connect_info: Option<&SocketAddr>) -> IpAddr {
        // Try X-Forwarded-For first (proxy chain, take first IP)
        if let Some(xff) = headers.get("x-forwarded-for") {
            if let Ok(xff_str) = xff.to_str() {
                // X-Forwarded-For can be "client, proxy1, proxy2" - take first
                if let Some(first_ip) = xff_str.split(',').next() {
                    if let Ok(ip) = first_ip.trim().parse::<IpAddr>() {
                        trace!(ip = %ip, "Extracted IP from X-Forwarded-For");
                        return ip;
                    }
                }
            }
        }

        // Try X-Real-IP
        if let Some(xri) = headers.get("x-real-ip") {
            if let Ok(xri_str) = xri.to_str() {
                if let Ok(ip) = xri_str.trim().parse::<IpAddr>() {
                    trace!(ip = %ip, "Extracted IP from X-Real-IP");
                    return ip;
                }
            }
        }

        // Fall back to connection socket address
        if let Some(addr) = connect_info {
            trace!(ip = %addr.ip(), "Using connection IP");
            return addr.ip();
        }

        // Ultimate fallback to localhost (shouldn't happen in normal operation)
        trace!("No IP found, using localhost fallback");
        IpAddr::V4(std::net::Ipv4Addr::LOCALHOST)
    }

    /// Extract user ID from JWT token in Authorization header
    ///
    /// This is a lightweight extraction that doesn't validate the token.
    /// The actual authentication happens later in the request pipeline.
    /// We just need the user ID for rate limiting purposes.
    fn extract_user_id_from_auth_header(headers: &HeaderMap) -> Option<Uuid> {
        let auth_header = headers.get(AUTHORIZATION)?;
        let auth_str = auth_header.to_str().ok()?;
        let token = auth_str.strip_prefix("Bearer ")?;

        // JWT format: header.payload.signature
        // We need to extract the 'sub' claim from the payload
        let parts: Vec<&str> = token.split('.').collect();
        if parts.len() != 3 {
            return None;
        }

        // Decode the payload (base64url)
        use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
        let payload_bytes = URL_SAFE_NO_PAD.decode(parts[1]).ok()?;
        let payload_str = String::from_utf8(payload_bytes).ok()?;

        // Parse as JSON and extract 'sub' field
        let payload: serde_json::Value = serde_json::from_str(&payload_str).ok()?;
        let sub = payload.get("sub")?.as_str()?;

        // Parse as UUID
        Uuid::parse_str(sub).ok()
    }
}

impl<S> Service<Request<Body>> for RateLimitMiddleware<S>
where
    S: Service<Request<Body>, Response = Response<Body>> + Clone + Send + 'static,
    S::Future: Send + 'static,
{
    type Response = Response<Body>;
    type Error = S::Error;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        let path = req.uri().path().to_string();
        let headers = req.headers().clone();
        let rate_limiter = self.rate_limiter.clone();
        let exempt_paths = self.exempt_paths.clone();
        let mut inner = self.inner.clone();

        // Try to extract ConnectInfo from request extensions
        let connect_info = req
            .extensions()
            .get::<ConnectInfo<SocketAddr>>()
            .map(|ci| ci.0);

        Box::pin(async move {
            // Check if rate limiting is enabled
            if !rate_limiter.is_enabled() {
                return inner.call(req).await;
            }

            // Check if path is exempt
            let is_exempt = exempt_paths.contains(&path)
                || exempt_paths.iter().any(|exempt| path.starts_with(exempt));

            if is_exempt {
                trace!(path = %path, "Path exempt from rate limiting");
                return inner.call(req).await;
            }

            // Determine client identity
            let client_id = if let Some(user_id) = Self::extract_user_id_from_auth_header(&headers)
            {
                ClientId::User(user_id)
            } else {
                let ip = Self::extract_ip_from_request(&headers, connect_info.as_ref());
                ClientId::Ip(ip)
            };

            // Check rate limit
            let result = rate_limiter.check_rate_limit(client_id.clone());

            if !result.allowed {
                debug!(
                    client = %client_id,
                    retry_after = result.retry_after_secs,
                    "Rate limit exceeded"
                );

                // Return 429 response
                let body = RateLimitExceededResponse {
                    error: "rate_limit_exceeded".to_string(),
                    message: format!(
                        "Too many requests. Please retry after {} seconds.",
                        result.retry_after_secs
                    ),
                    retry_after: result.retry_after_secs,
                };

                let mut response = (StatusCode::TOO_MANY_REQUESTS, Json(body)).into_response();

                // Add rate limit headers
                let headers = response.headers_mut();
                headers.insert(
                    HEADER_RATE_LIMIT,
                    HeaderValue::from_str(&result.limit.to_string()).unwrap(),
                );
                headers.insert(
                    HEADER_RATE_REMAINING,
                    HeaderValue::from_str(&result.remaining.to_string()).unwrap(),
                );
                headers.insert(
                    HEADER_RATE_RESET,
                    HeaderValue::from_str(&result.retry_after_secs.to_string()).unwrap(),
                );
                headers.insert(
                    HEADER_RETRY_AFTER,
                    HeaderValue::from_str(&result.retry_after_secs.to_string()).unwrap(),
                );

                return Ok(response);
            }

            // Request allowed - call inner service
            let response = inner.call(req).await?;

            // Add rate limit headers to successful response
            let (mut parts, body) = response.into_parts();

            parts.headers.insert(
                HEADER_RATE_LIMIT,
                HeaderValue::from_str(&result.limit.to_string()).unwrap(),
            );
            parts.headers.insert(
                HEADER_RATE_REMAINING,
                HeaderValue::from_str(&result.remaining.to_string()).unwrap(),
            );
            // Reset is 0 for allowed requests (they have tokens available)
            parts
                .headers
                .insert(HEADER_RATE_RESET, HeaderValue::from_static("0"));

            Ok(Response::from_parts(parts, body))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, Ipv6Addr};

    // ===================
    // IP extraction tests
    // ===================

    fn create_headers_with(name: &'static str, value: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(name, HeaderValue::from_str(value).unwrap());
        headers
    }

    #[test]
    fn test_extract_ip_from_x_forwarded_for_single() {
        let headers = create_headers_with("x-forwarded-for", "192.168.1.100");
        let ip = RateLimitMiddleware::<()>::extract_ip_from_request(&headers, None);
        assert_eq!(ip, IpAddr::V4(Ipv4Addr::new(192, 168, 1, 100)));
    }

    #[test]
    fn test_extract_ip_from_x_forwarded_for_chain() {
        // X-Forwarded-For chain: client, proxy1, proxy2
        let headers = create_headers_with("x-forwarded-for", "10.0.0.1, 192.168.1.1, 172.16.0.1");
        let ip = RateLimitMiddleware::<()>::extract_ip_from_request(&headers, None);
        // Should take first IP (the client)
        assert_eq!(ip, IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)));
    }

    #[test]
    fn test_extract_ip_from_x_real_ip() {
        let headers = create_headers_with("x-real-ip", "10.0.0.50");
        let ip = RateLimitMiddleware::<()>::extract_ip_from_request(&headers, None);
        assert_eq!(ip, IpAddr::V4(Ipv4Addr::new(10, 0, 0, 50)));
    }

    #[test]
    fn test_extract_ip_x_forwarded_for_takes_priority() {
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", HeaderValue::from_static("10.0.0.1"));
        headers.insert("x-real-ip", HeaderValue::from_static("10.0.0.2"));

        let ip = RateLimitMiddleware::<()>::extract_ip_from_request(&headers, None);
        // X-Forwarded-For takes priority
        assert_eq!(ip, IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)));
    }

    #[test]
    fn test_extract_ip_from_connect_info() {
        let headers = HeaderMap::new();
        let socket_addr: SocketAddr = "192.168.1.200:12345".parse().unwrap();
        let ip = RateLimitMiddleware::<()>::extract_ip_from_request(&headers, Some(&socket_addr));
        assert_eq!(ip, IpAddr::V4(Ipv4Addr::new(192, 168, 1, 200)));
    }

    #[test]
    fn test_extract_ip_fallback_to_localhost() {
        let headers = HeaderMap::new();
        let ip = RateLimitMiddleware::<()>::extract_ip_from_request(&headers, None);
        assert_eq!(ip, IpAddr::V4(Ipv4Addr::LOCALHOST));
    }

    #[test]
    fn test_extract_ip_ipv6() {
        let headers = create_headers_with("x-forwarded-for", "2001:db8::1");
        let ip = RateLimitMiddleware::<()>::extract_ip_from_request(&headers, None);
        assert_eq!(
            ip,
            IpAddr::V6(Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 1))
        );
    }

    #[test]
    fn test_extract_ip_invalid_header_falls_through() {
        let headers = create_headers_with("x-forwarded-for", "not-an-ip");
        let socket_addr: SocketAddr = "10.0.0.99:8080".parse().unwrap();
        let ip = RateLimitMiddleware::<()>::extract_ip_from_request(&headers, Some(&socket_addr));
        // Should fall through to connect info
        assert_eq!(ip, IpAddr::V4(Ipv4Addr::new(10, 0, 0, 99)));
    }

    // ===================
    // User ID extraction tests
    // ===================

    fn create_jwt_token(sub: &str) -> String {
        use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};

        // Create a minimal JWT with just the 'sub' claim
        let header = URL_SAFE_NO_PAD.encode(r#"{"alg":"HS256","typ":"JWT"}"#);
        let payload = URL_SAFE_NO_PAD.encode(format!(r#"{{"sub":"{}"}}"#, sub));
        let signature = URL_SAFE_NO_PAD.encode("fake-signature");

        format!("{}.{}.{}", header, payload, signature)
    }

    #[test]
    fn test_extract_user_id_from_valid_jwt() {
        let user_id = Uuid::new_v4();
        let token = create_jwt_token(&user_id.to_string());
        let headers = create_headers_with("authorization", &format!("Bearer {}", token));

        let extracted = RateLimitMiddleware::<()>::extract_user_id_from_auth_header(&headers);
        assert_eq!(extracted, Some(user_id));
    }

    #[test]
    fn test_extract_user_id_no_auth_header() {
        let headers = HeaderMap::new();
        let extracted = RateLimitMiddleware::<()>::extract_user_id_from_auth_header(&headers);
        assert_eq!(extracted, None);
    }

    #[test]
    fn test_extract_user_id_not_bearer_token() {
        let headers = create_headers_with("authorization", "Basic dXNlcjpwYXNz");
        let extracted = RateLimitMiddleware::<()>::extract_user_id_from_auth_header(&headers);
        assert_eq!(extracted, None);
    }

    #[test]
    fn test_extract_user_id_invalid_jwt_format() {
        let headers = create_headers_with("authorization", "Bearer not.a.valid.jwt");
        let extracted = RateLimitMiddleware::<()>::extract_user_id_from_auth_header(&headers);
        // Too many parts
        assert_eq!(extracted, None);
    }

    #[test]
    fn test_extract_user_id_invalid_uuid_in_jwt() {
        let token = create_jwt_token("not-a-uuid");
        let headers = create_headers_with("authorization", &format!("Bearer {}", token));

        let extracted = RateLimitMiddleware::<()>::extract_user_id_from_auth_header(&headers);
        assert_eq!(extracted, None);
    }

    // ===================
    // Layer construction tests
    // ===================

    #[test]
    fn test_rate_limit_layer_creation() {
        let config = Arc::new(crate::config::RateLimitConfig::default());
        let service = Arc::new(RateLimiterService::new(config));
        let layer = RateLimitLayer::new(
            service,
            vec!["/health".to_string(), "/api/v1/events".to_string()],
        );

        assert_eq!(layer.exempt_paths.len(), 2);
        assert!(layer.exempt_paths.contains("/health"));
        assert!(layer.exempt_paths.contains("/api/v1/events"));
    }

    #[test]
    fn test_rate_limit_layer_exempt_paths_deduplication() {
        let config = Arc::new(crate::config::RateLimitConfig::default());
        let service = Arc::new(RateLimiterService::new(config));
        let layer = RateLimitLayer::new(
            service,
            vec![
                "/health".to_string(),
                "/health".to_string(), // Duplicate
                "/api/v1/events".to_string(),
            ],
        );

        // HashSet deduplicates
        assert_eq!(layer.exempt_paths.len(), 2);
    }
}
