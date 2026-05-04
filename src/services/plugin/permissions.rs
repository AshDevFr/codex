//! Permission enforcement for reverse-RPC method dispatch.
//!
//! Plugins declare their capabilities in their manifest. When a plugin makes
//! a reverse-RPC call (e.g. `storage/get`, `releases/record`), the host
//! resolves the method namespace to a [`RequiredCapability`] and checks the
//! manifest before dispatching. Calls without the right capability are
//! rejected with [`PermissionError`].
//!
//! ## Why this exists
//!
//! The plugin survey identified that capabilities were declared but not
//! actually enforced at dispatch — a metadata-only plugin could still call
//! `sync/*` methods if the host happened to wire them up. Adding the
//! `release_source` capability is the forcing function for closing that gap
//! uniformly across every reverse-RPC namespace.
//!
//! ## Mapping
//!
//! The mapping table here is the single source of truth for "which capability
//! is needed to call this method." Adding a new namespace means adding a
//! mapping here AND wiring the handler — the dispatcher won't route a method
//! that has no mapping.

use super::protocol::{PluginCapabilities, methods};

/// Capability required to call a particular reverse-RPC namespace.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)] // Variants below are part of the mapping vocabulary; not all
// namespaces (sync/recommendations) have reverse-RPC methods yet.
pub enum RequiredCapability {
    /// `metadata_provider` (any non-empty content type list).
    MetadataProvider,
    /// `user_read_sync = true`.
    UserReadSync,
    /// `user_recommendation_provider = true`.
    UserRecommendationProvider,
    /// `release_source` capability declared.
    ReleaseSource,
    /// Always allowed; e.g. `storage/*` is scoped per user-plugin instance,
    /// so any plugin that's been spawned has implicit storage access.
    AlwaysAllowed,
}

/// Why a permission check failed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PermissionError {
    #[error(
        "method `{method}` requires the `{required:?}` capability, which the plugin did not declare"
    )]
    Denied {
        method: String,
        required: RequiredCapability,
    },
    #[error("unknown method `{method}` (no permission mapping)")]
    UnknownMethod { method: String },
}

/// Resolve the method name to its required capability.
///
/// Returns `None` if the method has no permission mapping. The dispatcher
/// treats `None` as "method not found" — adding new methods requires
/// updating this mapping.
pub fn required_capability(method: &str) -> Option<RequiredCapability> {
    match method {
        // Storage is scoped per (user, plugin) at handler-construction time;
        // any plugin that has been started can call storage. The mapping is
        // explicit so it doesn't fall through to UnknownMethod.
        methods::STORAGE_GET
        | methods::STORAGE_SET
        | methods::STORAGE_DELETE
        | methods::STORAGE_LIST
        | methods::STORAGE_CLEAR => Some(RequiredCapability::AlwaysAllowed),

        // Releases — gated on the `release_source` capability.
        methods::RELEASES_LIST_TRACKED
        | methods::RELEASES_RECORD
        | methods::RELEASES_SOURCE_STATE_GET
        | methods::RELEASES_SOURCE_STATE_SET => Some(RequiredCapability::ReleaseSource),

        _ => None,
    }
}

/// Check whether `caps` satisfies `required`.
pub fn capability_satisfied(caps: &PluginCapabilities, required: RequiredCapability) -> bool {
    match required {
        RequiredCapability::MetadataProvider => !caps.metadata_provider.is_empty(),
        RequiredCapability::UserReadSync => caps.user_read_sync,
        RequiredCapability::UserRecommendationProvider => caps.user_recommendation_provider,
        RequiredCapability::ReleaseSource => caps.is_release_source(),
        RequiredCapability::AlwaysAllowed => true,
    }
}

/// Convenience: enforce a method against a manifest's capabilities.
///
/// Returns `Ok(())` if the call should proceed, or [`PermissionError`] if
/// the dispatcher should refuse it.
pub fn enforce(method: &str, caps: &PluginCapabilities) -> Result<(), PermissionError> {
    let Some(required) = required_capability(method) else {
        return Err(PermissionError::UnknownMethod {
            method: method.to_string(),
        });
    };
    if !capability_satisfied(caps, required) {
        return Err(PermissionError::Denied {
            method: method.to_string(),
            required,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::protocol::{
        MetadataContentType, PluginCapabilities, ReleaseSourceCapability, ReleaseSourceKind,
    };
    use super::*;

    fn metadata_caps() -> PluginCapabilities {
        PluginCapabilities {
            metadata_provider: vec![MetadataContentType::Series],
            ..Default::default()
        }
    }

    fn release_caps() -> PluginCapabilities {
        PluginCapabilities {
            release_source: Some(ReleaseSourceCapability {
                kinds: vec![ReleaseSourceKind::RssUploader],
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    fn sync_caps() -> PluginCapabilities {
        PluginCapabilities {
            user_read_sync: true,
            external_id_source: Some("api:anilist".to_string()),
            ..Default::default()
        }
    }

    #[test]
    fn storage_methods_always_allowed() {
        for m in [
            methods::STORAGE_GET,
            methods::STORAGE_SET,
            methods::STORAGE_DELETE,
            methods::STORAGE_LIST,
            methods::STORAGE_CLEAR,
        ] {
            assert!(enforce(m, &PluginCapabilities::default()).is_ok());
            assert!(enforce(m, &metadata_caps()).is_ok());
        }
    }

    #[test]
    fn releases_methods_require_release_source_capability() {
        for m in [
            methods::RELEASES_LIST_TRACKED,
            methods::RELEASES_RECORD,
            methods::RELEASES_SOURCE_STATE_GET,
            methods::RELEASES_SOURCE_STATE_SET,
        ] {
            // Release-source plugin: allowed.
            assert!(enforce(m, &release_caps()).is_ok(), "{m} should be allowed");
            // Other capability set: denied.
            let err = enforce(m, &metadata_caps()).unwrap_err();
            assert!(
                matches!(
                    err,
                    PermissionError::Denied {
                        required: RequiredCapability::ReleaseSource,
                        ..
                    }
                ),
                "{m}: expected Denied(ReleaseSource), got {err:?}"
            );
            let err = enforce(m, &sync_caps()).unwrap_err();
            assert!(matches!(err, PermissionError::Denied { .. }));
        }
    }

    #[test]
    fn unmapped_method_is_unknown() {
        let err = enforce("frobnicate/zap", &release_caps()).unwrap_err();
        assert!(matches!(err, PermissionError::UnknownMethod { .. }));
    }

    #[test]
    fn capability_satisfied_for_metadata_provider() {
        assert!(capability_satisfied(
            &metadata_caps(),
            RequiredCapability::MetadataProvider
        ));
        assert!(!capability_satisfied(
            &PluginCapabilities::default(),
            RequiredCapability::MetadataProvider
        ));
    }

    #[test]
    fn required_capability_returns_some_for_known_methods() {
        assert_eq!(
            required_capability(methods::RELEASES_RECORD),
            Some(RequiredCapability::ReleaseSource)
        );
        assert_eq!(
            required_capability(methods::STORAGE_GET),
            Some(RequiredCapability::AlwaysAllowed)
        );
        assert_eq!(required_capability("not/a/method"), None);
    }
}
