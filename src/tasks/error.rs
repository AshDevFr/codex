//! Rate-limited error handling for task reschedules
//!
//! This module provides the `RateLimitedError` trait for detecting rate-limited errors
//! and rescheduling tasks without consuming retry attempts.
//!
//! ## Rate Limiting vs Error Handling
//!
//! When a task encounters an error, there are two possible scenarios:
//! 1. **Transient error**: A temporary failure that should be retried (increments `attempts`)
//! 2. **Rate limit**: A deliberate throttling that should wait and retry (increments `reschedule_count`)
//!
//! Rate-limited tasks use a separate counter to avoid exhausting retry attempts on
//! expected throttling behavior.

use crate::services::plugin::PluginManagerError;

/// Default retry delay in seconds for rate-limited tasks
pub const DEFAULT_RATE_LIMIT_RETRY_SECONDS: u64 = 30;

/// Default maximum number of rate limit reschedules before marking as failed
pub const DEFAULT_MAX_RESCHEDULES: i32 = 10;

/// Trait for errors that represent rate limiting
///
/// Implement this trait for error types that can indicate rate limiting,
/// allowing the task worker to detect and handle them specially.
pub trait RateLimitedError {
    /// Check if this error represents a rate limit
    fn is_rate_limited(&self) -> bool;

    /// Suggested delay before retry (in seconds)
    ///
    /// Returns `None` to use the default delay (30 seconds).
    fn retry_after_seconds(&self) -> Option<u64>;
}

/// Check if an anyhow::Error represents a rate limit
///
/// This function attempts to downcast the error to known rate-limited error types
/// and returns the suggested retry delay if it's a rate limit error.
///
/// # Returns
///
/// - `Some(seconds)` if the error is a rate limit, with the retry delay
/// - `None` if the error is not a rate limit
///
/// # Example
///
/// ```ignore
/// let error: anyhow::Error = some_operation().await?;
/// if let Some(retry_after) = check_rate_limited(&error) {
///     // Reschedule task for later
///     task_repo.mark_rate_limited(task_id, retry_after).await?;
/// } else {
///     // Normal error handling
///     task_repo.mark_failed(task_id, error.to_string()).await?;
/// }
/// ```
pub fn check_rate_limited(err: &anyhow::Error) -> Option<u64> {
    // Try downcasting to known rate-limited error types

    // Check PluginManagerError
    if let Some(e) = err.downcast_ref::<PluginManagerError>()
        && e.is_rate_limited()
    {
        return Some(
            e.retry_after_seconds()
                .unwrap_or(DEFAULT_RATE_LIMIT_RETRY_SECONDS),
        );
    }

    // Check wrapped errors (anyhow chains)
    // Walk the error chain looking for rate-limited errors
    for cause in err.chain() {
        if let Some(e) = cause.downcast_ref::<PluginManagerError>()
            && e.is_rate_limited()
        {
            return Some(
                e.retry_after_seconds()
                    .unwrap_or(DEFAULT_RATE_LIMIT_RETRY_SECONDS),
            );
        }
        // Add more error types here as needed
    }

    None
}

/// Implement RateLimitedError for PluginManagerError
///
/// Detects both local token bucket rate limits (`PluginManagerError::RateLimited`)
/// and RPC-level rate limits from the plugin's external API
/// (`PluginManagerError::Plugin(PluginError::Rpc(RpcError::RateLimited { .. }))`).
impl RateLimitedError for PluginManagerError {
    fn is_rate_limited(&self) -> bool {
        matches!(self, PluginManagerError::RateLimited { .. })
            || self.rpc_retry_after_seconds().is_some()
    }

    fn retry_after_seconds(&self) -> Option<u64> {
        match self {
            PluginManagerError::RateLimited {
                requests_per_minute,
                ..
            } => {
                // Calculate retry delay based on rate limit
                // For a rate of N requests/minute, wait 60/N seconds per token
                // Use slightly longer delay to be safe
                if *requests_per_minute > 0 {
                    let seconds_per_request = 60.0 / *requests_per_minute as f64;
                    // Wait for at least 2 token intervals, minimum 5 seconds
                    Some((seconds_per_request * 2.0).max(5.0).ceil() as u64)
                } else {
                    // If rate limit is 0 or negative (shouldn't happen), use default
                    Some(DEFAULT_RATE_LIMIT_RETRY_SECONDS)
                }
            }
            // RPC-level rate limit from the plugin's external API
            _ => self.rpc_retry_after_seconds(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::plugin::handle::PluginError;
    use crate::services::plugin::rpc::RpcError;
    use uuid::Uuid;

    /// Helper to create an RPC rate limit error wrapped in PluginManagerError
    fn rpc_rate_limited(retry_after: u64) -> PluginManagerError {
        PluginManagerError::Plugin(PluginError::Rpc(RpcError::RateLimited {
            retry_after_seconds: retry_after,
        }))
    }

    #[test]
    fn test_plugin_manager_error_is_rate_limited() {
        let rate_limited = PluginManagerError::RateLimited {
            plugin_id: Uuid::new_v4(),
            requests_per_minute: 60,
        };
        assert!(rate_limited.is_rate_limited());

        let not_rate_limited = PluginManagerError::PluginNotFound(Uuid::new_v4());
        assert!(!not_rate_limited.is_rate_limited());

        let not_enabled = PluginManagerError::PluginNotEnabled(Uuid::new_v4());
        assert!(!not_enabled.is_rate_limited());
    }

    #[test]
    fn test_rpc_rate_limited_is_detected() {
        let rpc_rate_limited = rpc_rate_limited(10);
        assert!(rpc_rate_limited.is_rate_limited());
        assert_eq!(rpc_rate_limited.retry_after_seconds(), Some(10));
    }

    #[test]
    fn test_rpc_rate_limited_helper_method() {
        let rpc_err = rpc_rate_limited(15);
        assert_eq!(rpc_err.rpc_retry_after_seconds(), Some(15));

        // Token bucket rate limit should not match the RPC helper
        let token_bucket = PluginManagerError::RateLimited {
            plugin_id: Uuid::new_v4(),
            requests_per_minute: 60,
        };
        assert_eq!(token_bucket.rpc_retry_after_seconds(), None);

        // Non-rate-limit errors should not match
        let not_found = PluginManagerError::PluginNotFound(Uuid::new_v4());
        assert_eq!(not_found.rpc_retry_after_seconds(), None);
    }

    #[test]
    fn test_check_rate_limited_with_rpc_error() {
        let rpc_err = rpc_rate_limited(10);
        let anyhow_err = anyhow::Error::from(rpc_err);

        let retry_after = check_rate_limited(&anyhow_err);
        assert_eq!(retry_after, Some(10));
    }

    #[test]
    fn test_check_rate_limited_with_wrapped_rpc_error() {
        let rpc_err = rpc_rate_limited(10);
        let anyhow_err = anyhow::Error::from(rpc_err).context("Failed to search for metadata");

        let retry_after = check_rate_limited(&anyhow_err);
        assert_eq!(retry_after, Some(10));
    }

    #[test]
    fn test_retry_after_seconds_calculation() {
        // 60 requests/minute = 1 per second, retry_after should be ~2 seconds
        let rate_limited_60 = PluginManagerError::RateLimited {
            plugin_id: Uuid::new_v4(),
            requests_per_minute: 60,
        };
        let retry_60 = rate_limited_60.retry_after_seconds().unwrap();
        assert!(
            (2..=5).contains(&retry_60),
            "Expected 2-5s, got {}",
            retry_60
        );

        // 30 requests/minute = 1 per 2 seconds, retry_after should be ~4 seconds
        let rate_limited_30 = PluginManagerError::RateLimited {
            plugin_id: Uuid::new_v4(),
            requests_per_minute: 30,
        };
        let retry_30 = rate_limited_30.retry_after_seconds().unwrap();
        assert!(
            (4..=5).contains(&retry_30),
            "Expected 4-5s, got {}",
            retry_30
        );

        // 10 requests/minute = 1 per 6 seconds, retry_after should be ~12 seconds
        let rate_limited_10 = PluginManagerError::RateLimited {
            plugin_id: Uuid::new_v4(),
            requests_per_minute: 10,
        };
        let retry_10 = rate_limited_10.retry_after_seconds().unwrap();
        assert!(
            (12..=15).contains(&retry_10),
            "Expected 12-15s, got {}",
            retry_10
        );

        // Non-rate-limited errors return None
        let not_rate_limited = PluginManagerError::PluginNotFound(Uuid::new_v4());
        assert!(not_rate_limited.retry_after_seconds().is_none());
    }

    #[test]
    fn test_check_rate_limited_direct_error() {
        let rate_limited = PluginManagerError::RateLimited {
            plugin_id: Uuid::new_v4(),
            requests_per_minute: 60,
        };
        let anyhow_err = anyhow::Error::from(rate_limited);

        let retry_after = check_rate_limited(&anyhow_err);
        assert!(retry_after.is_some());
        assert!(retry_after.unwrap() >= 2);
    }

    #[test]
    fn test_check_rate_limited_wrapped_error() {
        let rate_limited = PluginManagerError::RateLimited {
            plugin_id: Uuid::new_v4(),
            requests_per_minute: 60,
        };
        // Wrap the error with context
        let anyhow_err = anyhow::Error::from(rate_limited).context("Failed to search for series");

        let retry_after = check_rate_limited(&anyhow_err);
        assert!(retry_after.is_some());
    }

    #[test]
    fn test_check_rate_limited_non_rate_limited_error() {
        let not_found = PluginManagerError::PluginNotFound(Uuid::new_v4());
        let anyhow_err = anyhow::Error::from(not_found);

        let retry_after = check_rate_limited(&anyhow_err);
        assert!(retry_after.is_none());
    }

    #[test]
    fn test_check_rate_limited_unrelated_error() {
        let anyhow_err = anyhow::anyhow!("Some unrelated error");

        let retry_after = check_rate_limited(&anyhow_err);
        assert!(retry_after.is_none());
    }

    #[test]
    fn test_default_constants() {
        assert_eq!(DEFAULT_RATE_LIMIT_RETRY_SECONDS, 30);
        assert_eq!(DEFAULT_MAX_RESCHEDULES, 10);
    }
}
