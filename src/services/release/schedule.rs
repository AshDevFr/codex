//! Polling-interval resolution and jitter for the release-source scheduler.
//!
//! The scheduler fires one tick per enabled `release_sources` row. Each
//! row's effective poll interval is resolved once at scheduler-load time:
//!
//! 1. `release_sources.poll_interval_s` (per-source override) wins when
//!    the column is non-default.
//! 2. Otherwise the global server default
//!    `release_tracking.default_poll_interval_s` is used (default
//!    `86400` = once daily).
//!
//! Per-series overrides (`series_tracking.poll_interval_override`) are
//! consulted by plugins that opt into per-series polling — they don't
//! apply at the scheduler level, since scheduler ticks are per-source,
//! not per-series.
//!
//! Jitter is ±10% of the interval, applied at scheduler load. It spreads
//! load across many sources so a fresh restart doesn't fire all sources
//! in lockstep.
//!
//! Per-host backoff is applied multiplicatively on top of the resolved
//! interval — see [`super::backoff`].

use rand::RngExt;

/// Default poll interval if no setting is configured: 24 hours.
pub const DEFAULT_POLL_INTERVAL_S: u32 = 86_400;

/// Minimum interval the scheduler will accept. Sub-minute polling is
/// pointless for release feeds and risks rate-limit hits.
pub const MIN_POLL_INTERVAL_S: u32 = 60;

/// Setting key for the global default.
pub const SETTING_DEFAULT_POLL_INTERVAL_S: &str = "release_tracking.default_poll_interval_s";

/// Resolve the effective interval (in seconds) for a source row.
///
/// `per_source` is `release_sources.poll_interval_s` (may be the row
/// default of `0` if unset). `global_default` is the configured server
/// default; `0` falls back to [`DEFAULT_POLL_INTERVAL_S`]. The chosen
/// value is clamped to [`MIN_POLL_INTERVAL_S`].
pub fn resolve_interval_s(per_source: i32, global_default: u32) -> u32 {
    let global = if global_default == 0 {
        DEFAULT_POLL_INTERVAL_S
    } else {
        global_default
    };
    let chosen = if per_source > 0 {
        per_source as u32
    } else {
        global
    };
    chosen.max(MIN_POLL_INTERVAL_S)
}

/// Apply ±10% jitter to a base interval. Returns a value in
/// `[0.9 * base, 1.1 * base]`, clamped to [`MIN_POLL_INTERVAL_S`].
pub fn jitter_interval_s(base_s: u32) -> u32 {
    if base_s == 0 {
        return MIN_POLL_INTERVAL_S;
    }
    let mut rng = rand::rng();
    let factor: f64 = rng.random_range(0.9_f64..1.1_f64);
    let jittered = (base_s as f64 * factor).round() as u32;
    jittered.max(MIN_POLL_INTERVAL_S)
}

/// Apply a backoff multiplier (from [`super::backoff`]) to a base interval.
/// Returns the post-backoff interval, clamped to [`MIN_POLL_INTERVAL_S`].
pub fn apply_backoff(base_s: u32, multiplier: f64) -> u32 {
    let mult = if multiplier.is_finite() && multiplier >= 1.0 {
        multiplier
    } else {
        1.0
    };
    let scaled = (base_s as f64 * mult).round() as u32;
    scaled.max(MIN_POLL_INTERVAL_S)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_uses_per_source_when_set() {
        assert_eq!(resolve_interval_s(7200, 86_400), 7_200);
    }

    #[test]
    fn resolve_falls_back_to_global_when_per_source_zero_or_negative() {
        assert_eq!(resolve_interval_s(0, 3_600), 3_600);
        assert_eq!(resolve_interval_s(-1, 3_600), 3_600);
    }

    #[test]
    fn resolve_uses_default_when_global_zero() {
        assert_eq!(resolve_interval_s(0, 0), DEFAULT_POLL_INTERVAL_S);
    }

    #[test]
    fn resolve_clamps_to_minimum() {
        assert_eq!(resolve_interval_s(10, 86_400), MIN_POLL_INTERVAL_S);
    }

    #[test]
    fn jitter_stays_within_band() {
        let base = 3_600u32;
        for _ in 0..200 {
            let j = jitter_interval_s(base);
            assert!(
                j >= (base as f64 * 0.9).round() as u32 - 1,
                "j too low: {}",
                j
            );
            assert!(
                j <= (base as f64 * 1.1).round() as u32 + 1,
                "j too high: {}",
                j
            );
        }
    }

    #[test]
    fn jitter_clamps_to_minimum() {
        // base = 30, jitter 0.9..1.1 → 27..33; clamped to 60.
        for _ in 0..50 {
            assert!(jitter_interval_s(30) >= MIN_POLL_INTERVAL_S);
        }
    }

    #[test]
    fn apply_backoff_scales_when_active() {
        assert_eq!(apply_backoff(3_600, 2.0), 7_200);
        assert_eq!(apply_backoff(3_600, 4.0), 14_400);
    }

    #[test]
    fn apply_backoff_passes_through_when_inactive() {
        assert_eq!(apply_backoff(3_600, 1.0), 3_600);
    }

    #[test]
    fn apply_backoff_rejects_invalid_multipliers() {
        assert_eq!(apply_backoff(3_600, 0.5), 3_600);
        assert_eq!(apply_backoff(3_600, f64::NAN), 3_600);
        assert_eq!(apply_backoff(3_600, -1.0), 3_600);
    }

    #[test]
    fn apply_backoff_clamps_to_min() {
        assert_eq!(apply_backoff(20, 1.0), MIN_POLL_INTERVAL_S);
    }
}
