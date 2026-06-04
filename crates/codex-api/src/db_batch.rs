//! Bounded fan-out for per-request batched reads.
//!
//! List endpoints enrich their rows from many related tables. Running those
//! queries with an unbounded `tokio::join!` lets a single request hold one
//! pool connection *per query* simultaneously (14 for the full series DTO).
//! Under concurrent load that exhausts the connection pool — acutely on SQLite,
//! whose pool is small — so requests block for seconds on `acquire()`.
//!
//! Gating each query on a shared [`Semaphore`] caps how many run, and therefore
//! how many connections one request holds, at once. The concurrency benefit is
//! preserved up to the bound while the pathological amplification is removed.
//!
//! Each `tokio::join!` arm keeps its own return type — this gates heterogeneous
//! futures (the arms return different map types) without the boxing/type-erasure
//! a homogeneous `buffer_unordered` stream would require.

use std::future::Future;
use std::sync::OnceLock;
use tokio::sync::Semaphore;

/// Fallback per-request fan-out bound used until [`set_fan_out`] is called
/// (e.g. in tests that exercise handlers without going through `serve`).
pub const DEFAULT_BATCH_FAN_OUT: usize = 4;

/// Process-wide resolved fan-out bound, set once at startup from the per-backend
/// database config. It is a process constant (the backend never changes at
/// runtime), so a global avoids threading the value through every converter and
/// call site. Unset → [`DEFAULT_BATCH_FAN_OUT`].
static CONFIGURED_FAN_OUT: OnceLock<usize> = OnceLock::new();

/// Set the process-wide fan-out bound from configuration. Called once during
/// startup. Clamped to at least 1. Subsequent calls are ignored (the first
/// value wins), which keeps it stable across the process lifetime.
pub fn set_fan_out(bound: usize) {
    let _ = CONFIGURED_FAN_OUT.set(bound.max(1));
}

/// The configured fan-out bound, or [`DEFAULT_BATCH_FAN_OUT`] if unset.
pub fn configured_fan_out() -> usize {
    CONFIGURED_FAN_OUT
        .get()
        .copied()
        .unwrap_or(DEFAULT_BATCH_FAN_OUT)
}

/// Build a [`Semaphore`] that bounds concurrent batched reads to `bound`.
///
/// `bound` is clamped to at least 1 so a misconfigured `0` cannot deadlock the
/// request (which would otherwise never acquire a permit).
pub fn fan_out_limiter(bound: usize) -> Semaphore {
    Semaphore::new(bound.max(1))
}

/// Run `fut` once a permit is available, holding the permit for the whole query.
///
/// Used to wrap each arm of a `tokio::join!` so the number of arms actively
/// executing (and thus pool connections held) never exceeds the `limiter`'s
/// bound. Arms beyond the bound are parked on `acquire()` holding no connection.
pub async fn with_permit<F: Future>(limiter: &Semaphore, fut: F) -> F::Output {
    let _permit = limiter
        .acquire()
        .await
        .expect("fan-out limiter semaphore is never closed");
    fut.await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering::SeqCst};

    #[test]
    fn fan_out_limiter_clamps_zero_to_one() {
        assert_eq!(fan_out_limiter(0).available_permits(), 1);
        assert_eq!(fan_out_limiter(4).available_permits(), 4);
    }

    #[test]
    fn configured_fan_out_is_always_positive() {
        // Whether or not set_fan_out has run in this process, the bound must be
        // >= 1 so a request can always acquire a permit. (We avoid calling
        // set_fan_out here: CONFIGURED_FAN_OUT is process-global and a write
        // would leak into other tests in this binary.)
        assert!(configured_fan_out() >= 1);
    }

    /// `with_permit` must (a) return each arm's value in order and (b) never let
    /// more than `bound` arms execute concurrently.
    #[tokio::test]
    async fn with_permit_bounds_concurrency_and_preserves_results() {
        let limiter = fan_out_limiter(3);
        let current = Arc::new(AtomicUsize::new(0));
        let max_seen = Arc::new(AtomicUsize::new(0));

        let task = |i: usize| {
            let current = current.clone();
            let max_seen = max_seen.clone();
            async move {
                let now = current.fetch_add(1, SeqCst) + 1;
                max_seen.fetch_max(now, SeqCst);
                // Yield repeatedly so other arms get a chance to run; this is
                // what would let concurrency exceed the bound if it were unbounded.
                for _ in 0..8 {
                    tokio::task::yield_now().await;
                }
                current.fetch_sub(1, SeqCst);
                i
            }
        };

        let results = tokio::join!(
            with_permit(&limiter, task(0)),
            with_permit(&limiter, task(1)),
            with_permit(&limiter, task(2)),
            with_permit(&limiter, task(3)),
            with_permit(&limiter, task(4)),
            with_permit(&limiter, task(5)),
            with_permit(&limiter, task(6)),
            with_permit(&limiter, task(7)),
        );

        assert_eq!(results, (0, 1, 2, 3, 4, 5, 6, 7));
        assert!(
            max_seen.load(SeqCst) <= 3,
            "observed {} concurrent arms, expected <= 3",
            max_seen.load(SeqCst)
        );
    }
}
