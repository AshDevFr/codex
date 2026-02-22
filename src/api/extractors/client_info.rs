use axum::{extract::FromRequestParts, http::request::Parts};

/// Client information extracted from request headers
///
/// This extractor provides information about the client making the request,
/// primarily for audit logging purposes.
#[derive(Debug, Clone)]
pub struct ClientInfo {
    /// The client's IP address extracted from headers
    ///
    /// Extraction priority:
    /// 1. X-Forwarded-For (leftmost IP - the original client)
    /// 2. X-Real-IP
    /// 3. None (connection IP requires ConnectInfo layer)
    ///
    /// Note: X-Forwarded-For can be spoofed. Only trust this header when
    /// behind a properly configured reverse proxy (nginx, Apache, etc.)
    pub ip_address: Option<String>,
}

impl ClientInfo {
    /// Extract IP address from X-Forwarded-For header
    ///
    /// Format: "X-Forwarded-For: client, proxy1, proxy2"
    /// Returns the leftmost (original client) IP address
    fn extract_from_forwarded_for(value: &str) -> Option<String> {
        value
            .split(',')
            .next()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    }

    /// Extract IP address from X-Real-IP header
    fn extract_from_real_ip(value: &str) -> Option<String> {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    }
}

impl<S> FromRequestParts<S> for ClientInfo
where
    S: Send + Sync,
{
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        // Try X-Forwarded-For first (most common behind reverse proxy)
        let ip_address = parts
            .headers
            .get("x-forwarded-for")
            .and_then(|h| h.to_str().ok())
            .and_then(Self::extract_from_forwarded_for)
            // Fallback to X-Real-IP
            .or_else(|| {
                parts
                    .headers
                    .get("x-real-ip")
                    .and_then(|h| h.to_str().ok())
                    .and_then(Self::extract_from_real_ip)
            });

        Ok(ClientInfo { ip_address })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::{HeaderMap, HeaderValue, Request};

    #[test]
    fn test_extract_from_forwarded_for_single_ip() {
        let ip = ClientInfo::extract_from_forwarded_for("192.168.1.100");
        assert_eq!(ip, Some("192.168.1.100".to_string()));
    }

    #[test]
    fn test_extract_from_forwarded_for_multiple_ips() {
        let ip = ClientInfo::extract_from_forwarded_for("192.168.1.100, 10.0.0.1, 172.16.0.1");
        assert_eq!(ip, Some("192.168.1.100".to_string()));
    }

    #[test]
    fn test_extract_from_forwarded_for_with_spaces() {
        let ip = ClientInfo::extract_from_forwarded_for("  192.168.1.100  ,  10.0.0.1  ");
        assert_eq!(ip, Some("192.168.1.100".to_string()));
    }

    #[test]
    fn test_extract_from_forwarded_for_empty() {
        let ip = ClientInfo::extract_from_forwarded_for("");
        assert_eq!(ip, None);
    }

    #[test]
    fn test_extract_from_real_ip() {
        let ip = ClientInfo::extract_from_real_ip("192.168.1.100");
        assert_eq!(ip, Some("192.168.1.100".to_string()));
    }

    #[test]
    fn test_extract_from_real_ip_with_spaces() {
        let ip = ClientInfo::extract_from_real_ip("  192.168.1.100  ");
        assert_eq!(ip, Some("192.168.1.100".to_string()));
    }

    #[test]
    fn test_extract_from_real_ip_empty() {
        let ip = ClientInfo::extract_from_real_ip("");
        assert_eq!(ip, None);
    }

    #[tokio::test]
    async fn test_extractor_with_x_forwarded_for() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-forwarded-for",
            HeaderValue::from_static("192.168.1.100, 10.0.0.1"),
        );

        let mut parts = Request::builder().uri("/").body(()).unwrap().into_parts().0;
        parts.headers = headers;

        let client_info = ClientInfo::from_request_parts(&mut parts, &())
            .await
            .unwrap();

        assert_eq!(client_info.ip_address, Some("192.168.1.100".to_string()));
    }

    #[tokio::test]
    async fn test_extractor_with_x_real_ip() {
        let mut headers = HeaderMap::new();
        headers.insert("x-real-ip", HeaderValue::from_static("192.168.1.100"));

        let mut parts = Request::builder().uri("/").body(()).unwrap().into_parts().0;
        parts.headers = headers;

        let client_info = ClientInfo::from_request_parts(&mut parts, &())
            .await
            .unwrap();

        assert_eq!(client_info.ip_address, Some("192.168.1.100".to_string()));
    }

    #[tokio::test]
    async fn test_extractor_forwarded_for_takes_priority() {
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", HeaderValue::from_static("192.168.1.100"));
        headers.insert("x-real-ip", HeaderValue::from_static("10.0.0.1"));

        let mut parts = Request::builder().uri("/").body(()).unwrap().into_parts().0;
        parts.headers = headers;

        let client_info = ClientInfo::from_request_parts(&mut parts, &())
            .await
            .unwrap();

        // X-Forwarded-For should take priority
        assert_eq!(client_info.ip_address, Some("192.168.1.100".to_string()));
    }

    #[tokio::test]
    async fn test_extractor_no_headers() {
        let headers = HeaderMap::new();

        let mut parts = Request::builder().uri("/").body(()).unwrap().into_parts().0;
        parts.headers = headers;

        let client_info = ClientInfo::from_request_parts(&mut parts, &())
            .await
            .unwrap();

        assert_eq!(client_info.ip_address, None);
    }
}
