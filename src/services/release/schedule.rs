//! Cron-schedule resolution for release-source polling.
//!
//! Resolution chain (consumed by [`crate::scheduler::release_sources`]):
//!
//! 1. `release_sources.cron_schedule` (per-source override) wins when set.
//! 2. Otherwise the server-wide `release_tracking.default_cron_schedule`
//!    setting.
//! 3. Otherwise the compile-time fallback ([`DEFAULT_CRON_SCHEDULE`]).
//!
//! Per-host backoff lives in [`super::backoff`] and is consulted at
//! poll-fire time (not at scheduler-load time): a throttled host's tick is
//! short-circuited rather than rewriting the cron expression. This keeps
//! the cron source-of-truth simple: one row, one schedule.

use crate::services::settings::SettingsService;

/// Compile-time fallback when neither the per-source override nor the
/// server-wide setting are present. Daily at midnight (5-field POSIX cron).
pub const DEFAULT_CRON_SCHEDULE: &str = "0 0 * * *";

/// Setting key for the server-wide default.
pub const SETTING_DEFAULT_CRON_SCHEDULE: &str = "release_tracking.default_cron_schedule";

/// Read the server-wide default cron schedule. Falls back to
/// [`DEFAULT_CRON_SCHEDULE`] when the setting is missing or blank.
pub async fn read_default_cron_schedule(settings: &SettingsService) -> String {
    let raw = settings
        .get_string(SETTING_DEFAULT_CRON_SCHEDULE, DEFAULT_CRON_SCHEDULE)
        .await
        .unwrap_or_else(|_| DEFAULT_CRON_SCHEDULE.to_string());
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        DEFAULT_CRON_SCHEDULE.to_string()
    } else {
        trimmed.to_string()
    }
}

/// Resolve the effective cron schedule for a source row.
///
/// `per_source` is `release_sources.cron_schedule` (NULL when the row is
/// inheriting). `server_default` is the resolved server-wide default. The
/// returned string is the raw 5- or 6-field cron expression; callers
/// normalize to the 6-field tokio-cron-scheduler format via
/// [`crate::utils::cron::normalize_cron_expression`].
pub fn resolve_cron_schedule(per_source: Option<&str>, server_default: &str) -> String {
    if let Some(cron) = per_source.map(str::trim).filter(|s| !s.is_empty()) {
        cron.to_string()
    } else if !server_default.trim().is_empty() {
        server_default.trim().to_string()
    } else {
        DEFAULT_CRON_SCHEDULE.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_uses_per_source_when_set() {
        assert_eq!(
            resolve_cron_schedule(Some("0 */6 * * *"), "0 0 * * *"),
            "0 */6 * * *"
        );
    }

    #[test]
    fn resolve_falls_back_to_server_default_when_per_source_blank() {
        assert_eq!(resolve_cron_schedule(None, "0 0 * * *"), "0 0 * * *");
        assert_eq!(resolve_cron_schedule(Some(""), "0 0 * * *"), "0 0 * * *");
        assert_eq!(resolve_cron_schedule(Some("   "), "0 0 * * *"), "0 0 * * *");
    }

    #[test]
    fn resolve_uses_compile_time_default_when_both_blank() {
        assert_eq!(resolve_cron_schedule(None, ""), DEFAULT_CRON_SCHEDULE);
        assert_eq!(resolve_cron_schedule(None, "   "), DEFAULT_CRON_SCHEDULE);
    }
}
