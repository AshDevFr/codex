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
