//! Validators for [`super::types::LibraryJobConfig`] and the row-level
//! fields ([`crate::db::entities::library_jobs`] common fields like name and
//! cron).
//!
//! Validators are typed as `Result<_, ValidationError>` so callers can map
//! to HTTP 400 / 422 responses without losing the precise reason. The
//! validator does not perform DB writes; it queries plugin metadata to
//! cross-check provider capabilities and otherwise inspects the input.

use sea_orm::DatabaseConnection;
use thiserror::Error;

use std::str::FromStr;

use crate::db::repositories::PluginsRepository;
use crate::services::metadata::FieldGroup;
use crate::utils::cron::{validate_cron_expression, validate_timezone};

use super::types::{
    LibraryJobConfig, MAX_CONCURRENCY_HARD_CAP, MetadataRefreshJobConfig, RefreshScope,
};

/// Stable error taxonomy for the library-jobs validators.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ValidationError {
    #[error("name must be 1..=200 characters")]
    NameOutOfRange,
    #[error("invalid cron expression: {0}")]
    InvalidCron(String),
    #[error("invalid timezone: {0}")]
    InvalidTimezone(String),
    #[error("provider must be 'plugin:<name>', got '{0}'")]
    ProviderFormat(String),
    #[error("plugin '{0}' not installed")]
    ProviderNotInstalled(String),
    #[error("plugin '{provider}' does not support the chosen scope; required: {required}")]
    ProviderScopeMismatch { provider: String, required: String },
    #[error("unknown field group '{0}'")]
    UnknownFieldGroup(String),
    #[error("max_concurrency must be between 1 and {0}")]
    MaxConcurrencyOutOfRange(u8),
    #[error("scope '{0}' not yet implemented; only 'series_only' is supported in this release")]
    ScopeNotImplemented(String),
    #[error("book_field_groups / book_extra_fields must be empty when scope is series_only")]
    BookFieldsRequireBookScope,
}

/// Validate a job's common fields plus its type-specific config.
///
/// `config` is borrowed; the caller is responsible for serialisation. The
/// validator returns the **normalised cron** and **normalised timezone** so
/// the caller can persist canonical strings.
pub struct ValidatedJobInputs {
    pub cron_schedule: String,
    pub timezone: Option<String>,
}

pub async fn validate_metadata_refresh_config(
    db: &DatabaseConnection,
    name: &str,
    cron_schedule: &str,
    timezone: Option<&str>,
    config: &MetadataRefreshJobConfig,
) -> Result<ValidatedJobInputs, ValidationError> {
    if name.is_empty() || name.len() > 200 {
        return Err(ValidationError::NameOutOfRange);
    }

    let cron = validate_cron_expression(cron_schedule)
        .map_err(|e| ValidationError::InvalidCron(e.to_string()))?;
    let tz = if let Some(t) = timezone {
        Some(validate_timezone(t).map_err(|e| ValidationError::InvalidTimezone(e.to_string()))?)
    } else {
        None
    };

    // Phase 9: only series_only is honoured at runtime.
    match config.scope {
        RefreshScope::SeriesOnly => {
            if !config.book_field_groups.is_empty() || !config.book_extra_fields.is_empty() {
                return Err(ValidationError::BookFieldsRequireBookScope);
            }
        }
        RefreshScope::BooksOnly | RefreshScope::SeriesAndBooks => {
            return Err(ValidationError::ScopeNotImplemented(
                config.scope.as_str().to_string(),
            ));
        }
    }

    if config.max_concurrency < 1 || config.max_concurrency > MAX_CONCURRENCY_HARD_CAP {
        return Err(ValidationError::MaxConcurrencyOutOfRange(
            MAX_CONCURRENCY_HARD_CAP,
        ));
    }

    for g in &config.field_groups {
        if FieldGroup::from_str(g).is_err() {
            return Err(ValidationError::UnknownFieldGroup(g.clone()));
        }
    }
    // book_field_groups already enforced empty above for series_only.

    let plugin_name = parse_provider_string(&config.provider)?;
    let plugin = PluginsRepository::get_by_name(db, plugin_name)
        .await
        .map_err(|e| ValidationError::InvalidCron(e.to_string()))?
        .ok_or_else(|| ValidationError::ProviderNotInstalled(config.provider.clone()))?;

    // Cross-check capabilities. Disabled plugins are accepted (the operator
    // may be staging the schedule while the plugin admin enables it).
    let manifest = plugin
        .cached_manifest()
        .ok_or_else(|| ValidationError::ProviderNotInstalled(config.provider.clone()))?;

    let needs_series = config.scope.writes_series();
    let needs_books = config.scope.writes_books();
    let supports_series = manifest.capabilities.can_provide_series_metadata();
    let supports_books = manifest.capabilities.can_provide_book_metadata();
    if needs_series && !supports_series {
        return Err(ValidationError::ProviderScopeMismatch {
            provider: config.provider.clone(),
            required: "series".to_string(),
        });
    }
    if needs_books && !supports_books {
        return Err(ValidationError::ProviderScopeMismatch {
            provider: config.provider.clone(),
            required: "books".to_string(),
        });
    }

    Ok(ValidatedJobInputs {
        cron_schedule: cron,
        timezone: tz,
    })
}

/// Helper: validate either variant of [`LibraryJobConfig`].
#[allow(dead_code)]
pub async fn validate_library_job_config(
    db: &DatabaseConnection,
    name: &str,
    cron_schedule: &str,
    timezone: Option<&str>,
    config: &LibraryJobConfig,
) -> Result<ValidatedJobInputs, ValidationError> {
    match config {
        LibraryJobConfig::MetadataRefresh(c) => {
            validate_metadata_refresh_config(db, name, cron_schedule, timezone, c).await
        }
    }
}

/// Parse a `"plugin:<name>"` string and return the inner name.
fn parse_provider_string(s: &str) -> Result<&str, ValidationError> {
    s.strip_prefix("plugin:")
        .filter(|rest| !rest.is_empty())
        .ok_or_else(|| ValidationError::ProviderFormat(s.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_provider_string_ok() {
        assert_eq!(
            parse_provider_string("plugin:mangabaka").unwrap(),
            "mangabaka"
        );
    }

    #[test]
    fn parse_provider_string_rejects_bad_format() {
        for bad in ["mangabaka", "plugin:", "external:mangabaka", ""] {
            let err = parse_provider_string(bad).unwrap_err();
            assert!(matches!(err, ValidationError::ProviderFormat(_)));
        }
    }
}
