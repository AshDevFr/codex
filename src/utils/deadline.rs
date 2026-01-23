//! Database operation deadline utilities
//!
//! Provides helpers for wrapping async database operations with configurable timeouts
//! to prevent indefinite connection holds from slow queries or stuck operations.

use std::future::Future;
use std::time::Duration;
use tokio::time::timeout;

/// Result of a deadline-wrapped operation
#[derive(Debug)]
pub enum DeadlineResult<T, E> {
    /// Operation completed successfully within deadline
    Ok(T),
    /// Operation returned an error within deadline
    Err(E),
    /// Operation timed out (deadline exceeded)
    TimedOut,
}

impl<T, E> DeadlineResult<T, E> {
    /// Returns true if the operation completed successfully
    #[allow(dead_code)]
    pub fn is_ok(&self) -> bool {
        matches!(self, DeadlineResult::Ok(_))
    }

    /// Returns true if the operation timed out
    #[allow(dead_code)]
    pub fn is_timed_out(&self) -> bool {
        matches!(self, DeadlineResult::TimedOut)
    }

    /// Converts to Option, returning None for errors and timeouts
    #[allow(dead_code)]
    pub fn ok(self) -> Option<T> {
        match self {
            DeadlineResult::Ok(v) => Some(v),
            _ => None,
        }
    }
}

/// Execute an async operation with a deadline
///
/// Wraps an async operation in a timeout. If the operation completes within
/// the deadline, returns the result. If it exceeds the deadline, returns
/// `DeadlineResult::TimedOut`.
///
/// # Arguments
///
/// * `deadline_secs` - Maximum time in seconds before timing out
/// * `operation` - The async operation to execute
///
/// # Examples
///
/// ```text
/// use codex::utils::deadline::{with_deadline, DeadlineResult};
///
/// let result = with_deadline(5, async {
///     // Some database operation
///     repository.save(&entity).await
/// }).await;
///
/// match result {
///     DeadlineResult::Ok(entity) => println!("Saved successfully"),
///     DeadlineResult::Err(e) => eprintln!("Save failed: {}", e),
///     DeadlineResult::TimedOut => eprintln!("Operation timed out"),
/// }
/// ```
pub async fn with_deadline<T, E, F>(deadline_secs: u64, operation: F) -> DeadlineResult<T, E>
where
    F: Future<Output = Result<T, E>>,
{
    match timeout(Duration::from_secs(deadline_secs), operation).await {
        Ok(Ok(result)) => DeadlineResult::Ok(result),
        Ok(Err(e)) => DeadlineResult::Err(e),
        Err(_elapsed) => DeadlineResult::TimedOut,
    }
}

/// Execute an async operation with a deadline, returning a standard Result
///
/// Similar to `with_deadline`, but converts timeout to the provided error value.
/// Useful when you want to treat timeout as a specific error type.
///
/// # Arguments
///
/// * `deadline_secs` - Maximum time in seconds before timing out
/// * `timeout_err` - Error value to return if operation times out
/// * `operation` - The async operation to execute
#[allow(dead_code)]
pub async fn with_deadline_or_err<T, E, F>(
    deadline_secs: u64,
    timeout_err: E,
    operation: F,
) -> Result<T, E>
where
    F: Future<Output = Result<T, E>>,
{
    match timeout(Duration::from_secs(deadline_secs), operation).await {
        Ok(result) => result,
        Err(_elapsed) => Err(timeout_err),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    #[tokio::test]
    async fn test_with_deadline_success() {
        let result: DeadlineResult<i32, &str> = with_deadline(5, async { Ok(42) }).await;

        assert!(result.is_ok());
        assert_eq!(result.ok(), Some(42));
    }

    #[tokio::test]
    async fn test_with_deadline_error() {
        let result: DeadlineResult<i32, &str> =
            with_deadline(5, async { Err("operation failed") }).await;

        assert!(!result.is_ok());
        assert!(!result.is_timed_out());
        match result {
            DeadlineResult::Err(e) => assert_eq!(e, "operation failed"),
            _ => panic!("Expected error result"),
        }
    }

    #[tokio::test]
    async fn test_with_deadline_timeout() {
        let start = Instant::now();

        let result: DeadlineResult<i32, &str> = with_deadline(1, async {
            tokio::time::sleep(Duration::from_secs(10)).await;
            Ok(42)
        })
        .await;

        let elapsed = start.elapsed();
        assert!(result.is_timed_out());
        // Should timeout around 1 second, not wait 10 seconds
        assert!(elapsed < Duration::from_secs(2));
    }

    #[tokio::test]
    async fn test_with_deadline_or_err_success() {
        let result: Result<i32, &str> = with_deadline_or_err(5, "timeout", async { Ok(42) }).await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
    }

    #[tokio::test]
    async fn test_with_deadline_or_err_timeout() {
        let result: Result<i32, &str> = with_deadline_or_err(1, "timeout", async {
            tokio::time::sleep(Duration::from_secs(10)).await;
            Ok(42)
        })
        .await;

        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "timeout");
    }

    #[tokio::test]
    async fn test_with_deadline_or_err_original_error() {
        let result: Result<i32, &str> =
            with_deadline_or_err(5, "timeout", async { Err("original error") }).await;

        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "original error");
    }
}
