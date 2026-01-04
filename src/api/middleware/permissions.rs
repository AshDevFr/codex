use crate::api::{error::ApiError, extractors::AuthContext, permissions::Permission};

/// Macro for declarative permission checking in handlers
///
/// Usage:
/// ```no_run
/// # use codex::{require_permission, api::{extractors::AuthContext, permissions::Permission}};
/// # let auth: AuthContext = todo!();
/// require_permission!(auth, Permission::LibrariesRead)?;
/// # Ok::<(), codex::api::error::ApiError>(())
/// ```
#[macro_export]
macro_rules! require_permission {
    ($auth:expr, $permission:expr) => {
        $auth.require_permission(&$permission)
    };
}

/// Macro for requiring admin access
///
/// Usage:
/// ```no_run
/// # use codex::{require_admin, api::extractors::AuthContext};
/// # let auth: AuthContext = todo!();
/// require_admin!(auth)?;
/// # Ok::<(), codex::api::error::ApiError>(())
/// ```
#[macro_export]
macro_rules! require_admin {
    ($auth:expr) => {
        $auth.require_admin()
    };
}

/// Helper function to check if user has permission
pub fn check_permission(auth: &AuthContext, permission: &Permission) -> Result<(), ApiError> {
    auth.require_permission(permission)
}

/// Helper function to check if user has any of the given permissions
pub fn check_any_permission(
    auth: &AuthContext,
    permissions: &[Permission],
) -> Result<(), ApiError> {
    if auth.has_any_permission(permissions) {
        Ok(())
    } else {
        Err(ApiError::Forbidden(
            "Missing required permissions".to_string(),
        ))
    }
}

/// Helper function to check admin access
pub fn check_admin(auth: &AuthContext) -> Result<(), ApiError> {
    auth.require_admin()
}
