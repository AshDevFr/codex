use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fmt;
use std::str::FromStr;
use utoipa::ToSchema;

/// User roles for role-based access control (RBAC)
///
/// Roles define a base set of permissions that users inherit.
/// Custom permissions can be added on top of role permissions (union behavior).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema, Default)]
#[serde(rename_all = "lowercase")]
pub enum UserRole {
    /// Basic read access - can browse and read content
    #[default]
    Reader,
    /// Content management - can modify series, books, run scans
    Maintainer,
    /// Full system access - can manage users, system settings
    Admin,
}

impl UserRole {
    /// Get the permission set associated with this role
    pub fn permissions(&self) -> &'static HashSet<Permission> {
        match self {
            UserRole::Reader => &READER_PERMISSIONS,
            UserRole::Maintainer => &MAINTAINER_PERMISSIONS,
            UserRole::Admin => &ADMIN_PERMISSIONS,
        }
    }

    /// Check if this role can assign another role to a user
    ///
    /// Admin can assign any role, Maintainer can only assign Reader,
    /// Reader cannot assign roles.
    #[allow(dead_code)] // Used in Phase 2 for user role assignment API
    pub fn can_assign(&self, target: UserRole) -> bool {
        match self {
            UserRole::Admin => true,
            UserRole::Maintainer => target == UserRole::Reader,
            UserRole::Reader => false,
        }
    }

    /// Returns all possible role values
    #[allow(dead_code)] // Used in Phase 2 for user role assignment API
    pub fn all() -> &'static [UserRole] {
        &[UserRole::Reader, UserRole::Maintainer, UserRole::Admin]
    }
}

impl fmt::Display for UserRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UserRole::Reader => write!(f, "reader"),
            UserRole::Maintainer => write!(f, "maintainer"),
            UserRole::Admin => write!(f, "admin"),
        }
    }
}

impl FromStr for UserRole {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "reader" => Ok(UserRole::Reader),
            "maintainer" => Ok(UserRole::Maintainer),
            "admin" => Ok(UserRole::Admin),
            _ => Err(format!("Unknown role: {}", s)),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Permission {
    // Libraries
    LibrariesRead,
    LibrariesWrite,
    LibrariesDelete,

    // Series
    SeriesRead,
    SeriesWrite,
    SeriesDelete,

    // Books
    BooksRead,
    BooksWrite,
    BooksDelete,

    // Pages (image serving)
    PagesRead,

    // Progress (reading progress tracking)
    ProgressRead,
    ProgressWrite,

    // Users (admin only)
    UsersRead,
    UsersWrite,
    UsersDelete,

    // API Keys (admin only)
    ApiKeysRead,
    ApiKeysWrite,
    ApiKeysDelete,

    // Tasks
    TasksRead,
    TasksWrite,

    // Plugins (admin configuration)
    PluginsManage,

    // System
    SystemHealth,
    SystemAdmin,
}

#[allow(dead_code)] // Public API for permission string representation
impl Permission {
    /// Convert permission to string format: "resource:action"
    pub fn as_str(&self) -> &'static str {
        match self {
            Permission::LibrariesRead => "libraries:read",
            Permission::LibrariesWrite => "libraries:write",
            Permission::LibrariesDelete => "libraries:delete",
            Permission::SeriesRead => "series:read",
            Permission::SeriesWrite => "series:write",
            Permission::SeriesDelete => "series:delete",
            Permission::BooksRead => "books:read",
            Permission::BooksWrite => "books:write",
            Permission::BooksDelete => "books:delete",
            Permission::PagesRead => "pages:read",
            Permission::ProgressRead => "progress:read",
            Permission::ProgressWrite => "progress:write",
            Permission::UsersRead => "users:read",
            Permission::UsersWrite => "users:write",
            Permission::UsersDelete => "users:delete",
            Permission::ApiKeysRead => "api-keys:read",
            Permission::ApiKeysWrite => "api-keys:write",
            Permission::ApiKeysDelete => "api-keys:delete",
            Permission::TasksRead => "tasks:read",
            Permission::TasksWrite => "tasks:write",
            Permission::PluginsManage => "plugins:manage",
            Permission::SystemHealth => "system:health",
            Permission::SystemAdmin => "system:admin",
        }
    }
}

impl FromStr for Permission {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "libraries:read" => Ok(Permission::LibrariesRead),
            "libraries:write" => Ok(Permission::LibrariesWrite),
            "libraries:delete" => Ok(Permission::LibrariesDelete),
            "series:read" => Ok(Permission::SeriesRead),
            "series:write" => Ok(Permission::SeriesWrite),
            "series:delete" => Ok(Permission::SeriesDelete),
            "books:read" => Ok(Permission::BooksRead),
            "books:write" => Ok(Permission::BooksWrite),
            "books:delete" => Ok(Permission::BooksDelete),
            "pages:read" => Ok(Permission::PagesRead),
            "progress:read" => Ok(Permission::ProgressRead),
            "progress:write" => Ok(Permission::ProgressWrite),
            "users:read" => Ok(Permission::UsersRead),
            "users:write" => Ok(Permission::UsersWrite),
            "users:delete" => Ok(Permission::UsersDelete),
            "api-keys:read" => Ok(Permission::ApiKeysRead),
            "api-keys:write" => Ok(Permission::ApiKeysWrite),
            "api-keys:delete" => Ok(Permission::ApiKeysDelete),
            "tasks:read" => Ok(Permission::TasksRead),
            "tasks:write" => Ok(Permission::TasksWrite),
            "plugins:manage" => Ok(Permission::PluginsManage),
            "system:health" => Ok(Permission::SystemHealth),
            "system:admin" => Ok(Permission::SystemAdmin),
            _ => Err(format!("Unknown permission: {}", s)),
        }
    }
}

/// Parse permissions from JSON string
#[allow(dead_code)] // Public API for permission parsing
pub fn parse_permissions(json: &str) -> Result<HashSet<Permission>, serde_json::Error> {
    let perms: Vec<Permission> = serde_json::from_str(json)?;
    Ok(perms.into_iter().collect())
}

/// Serialize permissions to JSON string
pub fn serialize_permissions(permissions: &HashSet<Permission>) -> String {
    let perms: Vec<Permission> = permissions.iter().cloned().collect();
    serde_json::to_string(&perms).unwrap_or_else(|_| "[]".to_string())
}

// Preset permission sets
lazy_static::lazy_static! {
    /// Read-only permissions (basic read access - legacy, kept for backwards compatibility)
    pub static ref READONLY_PERMISSIONS: HashSet<Permission> = {
        let mut set = HashSet::new();
        set.insert(Permission::LibrariesRead);
        set.insert(Permission::SeriesRead);
        set.insert(Permission::BooksRead);
        set.insert(Permission::PagesRead);
        set.insert(Permission::ProgressRead);
        set.insert(Permission::ProgressWrite);
        set.insert(Permission::SystemHealth);
        set
    };

    /// Reader role permissions
    ///
    /// Reader can:
    /// - Browse libraries, series, and books
    /// - Read pages/content
    /// - Manage their own API keys
    /// - View system health
    pub static ref READER_PERMISSIONS: HashSet<Permission> = {
        let mut set = HashSet::new();
        // Content access
        set.insert(Permission::LibrariesRead);
        set.insert(Permission::SeriesRead);
        set.insert(Permission::BooksRead);
        set.insert(Permission::PagesRead);
        // Progress tracking
        set.insert(Permission::ProgressRead);
        set.insert(Permission::ProgressWrite);
        // Own API keys
        set.insert(Permission::ApiKeysRead);
        set.insert(Permission::ApiKeysWrite);
        set.insert(Permission::ApiKeysDelete);
        // System
        set.insert(Permission::SystemHealth);
        set
    };

    /// Maintainer role permissions
    ///
    /// Maintainer can do everything Reader can, plus:
    /// - Create/modify libraries (but not delete)
    /// - Create/modify/delete series
    /// - Create/modify/delete books
    /// - View and manage tasks
    pub static ref MAINTAINER_PERMISSIONS: HashSet<Permission> = {
        let mut set = READER_PERMISSIONS.clone();
        // Libraries (create/modify, but not delete)
        set.insert(Permission::LibrariesWrite);
        // Series (full control)
        set.insert(Permission::SeriesWrite);
        set.insert(Permission::SeriesDelete);
        // Books (full control)
        set.insert(Permission::BooksWrite);
        set.insert(Permission::BooksDelete);
        // Tasks (view and manage)
        set.insert(Permission::TasksRead);
        set.insert(Permission::TasksWrite);
        set
    };

    /// Admin role permissions (all permissions)
    ///
    /// Admin can do everything, including:
    /// - Delete libraries
    /// - Manage users
    /// - Manage plugins
    /// - System administration
    pub static ref ADMIN_PERMISSIONS: HashSet<Permission> = {
        let mut set = MAINTAINER_PERMISSIONS.clone();
        // Libraries (full control including delete)
        set.insert(Permission::LibrariesDelete);
        // Users (full control)
        set.insert(Permission::UsersRead);
        set.insert(Permission::UsersWrite);
        set.insert(Permission::UsersDelete);
        // Plugins (configuration)
        set.insert(Permission::PluginsManage);
        // System admin
        set.insert(Permission::SystemAdmin);
        set
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    // ============== Permission tests ==============

    #[test]
    fn test_permission_as_str() {
        assert_eq!(Permission::LibrariesRead.as_str(), "libraries:read");
        assert_eq!(Permission::BooksWrite.as_str(), "books:write");
        assert_eq!(Permission::PluginsManage.as_str(), "plugins:manage");
        assert_eq!(Permission::SystemAdmin.as_str(), "system:admin");
    }

    #[test]
    fn test_permission_from_str() {
        assert_eq!(
            Permission::from_str("libraries:read").unwrap(),
            Permission::LibrariesRead
        );
        assert_eq!(
            Permission::from_str("books:write").unwrap(),
            Permission::BooksWrite
        );
        assert_eq!(
            Permission::from_str("plugins:manage").unwrap(),
            Permission::PluginsManage
        );
        assert!(Permission::from_str("invalid:permission").is_err());
    }

    #[test]
    fn test_parse_permissions() {
        let json = r#"["libraries-read", "books-read", "pages-read"]"#;
        let perms = parse_permissions(json).unwrap();

        assert_eq!(perms.len(), 3);
        assert!(perms.contains(&Permission::LibrariesRead));
        assert!(perms.contains(&Permission::BooksRead));
        assert!(perms.contains(&Permission::PagesRead));
    }

    #[test]
    fn test_serialize_permissions() {
        let mut perms = HashSet::new();
        perms.insert(Permission::LibrariesRead);
        perms.insert(Permission::BooksRead);

        let json = serialize_permissions(&perms);
        let parsed = parse_permissions(&json).unwrap();

        assert_eq!(parsed.len(), 2);
        assert!(parsed.contains(&Permission::LibrariesRead));
        assert!(parsed.contains(&Permission::BooksRead));
    }

    #[test]
    fn test_readonly_permissions() {
        assert!(READONLY_PERMISSIONS.contains(&Permission::LibrariesRead));
        assert!(READONLY_PERMISSIONS.contains(&Permission::BooksRead));
        assert!(!READONLY_PERMISSIONS.contains(&Permission::LibrariesWrite));
        assert_eq!(READONLY_PERMISSIONS.len(), 7);
    }

    // ============== Role permission preset tests ==============

    #[test]
    fn test_reader_permissions() {
        // Reader has basic content access
        assert!(READER_PERMISSIONS.contains(&Permission::LibrariesRead));
        assert!(READER_PERMISSIONS.contains(&Permission::SeriesRead));
        assert!(READER_PERMISSIONS.contains(&Permission::BooksRead));
        assert!(READER_PERMISSIONS.contains(&Permission::PagesRead));
        // Reader has API key management for themselves
        assert!(READER_PERMISSIONS.contains(&Permission::ApiKeysRead));
        assert!(READER_PERMISSIONS.contains(&Permission::ApiKeysWrite));
        assert!(READER_PERMISSIONS.contains(&Permission::ApiKeysDelete));
        // Reader has system health
        assert!(READER_PERMISSIONS.contains(&Permission::SystemHealth));
        // Reader cannot view or manage tasks
        assert!(!READER_PERMISSIONS.contains(&Permission::TasksRead));
        assert!(!READER_PERMISSIONS.contains(&Permission::TasksWrite));
        // Reader can track reading progress
        assert!(READER_PERMISSIONS.contains(&Permission::ProgressRead));
        assert!(READER_PERMISSIONS.contains(&Permission::ProgressWrite));
        // Reader cannot modify content
        assert!(!READER_PERMISSIONS.contains(&Permission::BooksWrite));
        assert!(!READER_PERMISSIONS.contains(&Permission::SeriesWrite));
        assert!(!READER_PERMISSIONS.contains(&Permission::LibrariesWrite));
        // Reader cannot manage users or system
        assert!(!READER_PERMISSIONS.contains(&Permission::UsersRead));
        assert!(!READER_PERMISSIONS.contains(&Permission::SystemAdmin));

        assert_eq!(READER_PERMISSIONS.len(), 10);
    }

    #[test]
    fn test_maintainer_permissions() {
        // Maintainer is a superset of Reader
        for perm in READER_PERMISSIONS.iter() {
            assert!(
                MAINTAINER_PERMISSIONS.contains(perm),
                "Maintainer missing Reader permission: {:?}",
                perm
            );
        }
        // Maintainer can modify libraries (but not delete)
        assert!(MAINTAINER_PERMISSIONS.contains(&Permission::LibrariesWrite));
        assert!(!MAINTAINER_PERMISSIONS.contains(&Permission::LibrariesDelete));
        // Maintainer can fully manage series and books
        assert!(MAINTAINER_PERMISSIONS.contains(&Permission::SeriesWrite));
        assert!(MAINTAINER_PERMISSIONS.contains(&Permission::SeriesDelete));
        assert!(MAINTAINER_PERMISSIONS.contains(&Permission::BooksWrite));
        assert!(MAINTAINER_PERMISSIONS.contains(&Permission::BooksDelete));
        // Maintainer can manage tasks
        assert!(MAINTAINER_PERMISSIONS.contains(&Permission::TasksWrite));
        // Maintainer cannot manage users or system admin
        assert!(!MAINTAINER_PERMISSIONS.contains(&Permission::UsersRead));
        assert!(!MAINTAINER_PERMISSIONS.contains(&Permission::SystemAdmin));

        assert_eq!(MAINTAINER_PERMISSIONS.len(), 17);
    }

    #[test]
    fn test_admin_permissions() {
        // Admin is a superset of Maintainer
        for perm in MAINTAINER_PERMISSIONS.iter() {
            assert!(
                ADMIN_PERMISSIONS.contains(perm),
                "Admin missing Maintainer permission: {:?}",
                perm
            );
        }
        // Admin has library delete
        assert!(ADMIN_PERMISSIONS.contains(&Permission::LibrariesDelete));
        // Admin has full user management
        assert!(ADMIN_PERMISSIONS.contains(&Permission::UsersRead));
        assert!(ADMIN_PERMISSIONS.contains(&Permission::UsersWrite));
        assert!(ADMIN_PERMISSIONS.contains(&Permission::UsersDelete));
        // Admin has plugin management
        assert!(ADMIN_PERMISSIONS.contains(&Permission::PluginsManage));
        // Admin has system admin
        assert!(ADMIN_PERMISSIONS.contains(&Permission::SystemAdmin));

        assert_eq!(ADMIN_PERMISSIONS.len(), 23); // All permissions
    }

    // ============== UserRole tests ==============

    #[test]
    fn test_user_role_from_str() {
        assert_eq!(UserRole::from_str("reader").unwrap(), UserRole::Reader);
        assert_eq!(UserRole::from_str("Reader").unwrap(), UserRole::Reader);
        assert_eq!(UserRole::from_str("READER").unwrap(), UserRole::Reader);
        assert_eq!(
            UserRole::from_str("maintainer").unwrap(),
            UserRole::Maintainer
        );
        assert_eq!(UserRole::from_str("admin").unwrap(), UserRole::Admin);
        assert!(UserRole::from_str("invalid").is_err());
    }

    #[test]
    fn test_user_role_display() {
        assert_eq!(UserRole::Reader.to_string(), "reader");
        assert_eq!(UserRole::Maintainer.to_string(), "maintainer");
        assert_eq!(UserRole::Admin.to_string(), "admin");
    }

    #[test]
    fn test_user_role_default() {
        assert_eq!(UserRole::default(), UserRole::Reader);
    }

    #[test]
    fn test_user_role_permissions() {
        assert_eq!(UserRole::Reader.permissions(), &*READER_PERMISSIONS);
        assert_eq!(UserRole::Maintainer.permissions(), &*MAINTAINER_PERMISSIONS);
        assert_eq!(UserRole::Admin.permissions(), &*ADMIN_PERMISSIONS);
    }

    #[test]
    fn test_user_role_can_assign() {
        // Admin can assign any role
        assert!(UserRole::Admin.can_assign(UserRole::Reader));
        assert!(UserRole::Admin.can_assign(UserRole::Maintainer));
        assert!(UserRole::Admin.can_assign(UserRole::Admin));

        // Maintainer can only assign Reader
        assert!(UserRole::Maintainer.can_assign(UserRole::Reader));
        assert!(!UserRole::Maintainer.can_assign(UserRole::Maintainer));
        assert!(!UserRole::Maintainer.can_assign(UserRole::Admin));

        // Reader cannot assign any role
        assert!(!UserRole::Reader.can_assign(UserRole::Reader));
        assert!(!UserRole::Reader.can_assign(UserRole::Maintainer));
        assert!(!UserRole::Reader.can_assign(UserRole::Admin));
    }

    #[test]
    fn test_user_role_all() {
        let all_roles = UserRole::all();
        assert_eq!(all_roles.len(), 3);
        assert!(all_roles.contains(&UserRole::Reader));
        assert!(all_roles.contains(&UserRole::Maintainer));
        assert!(all_roles.contains(&UserRole::Admin));
    }

    #[test]
    fn test_user_role_serialization() {
        // Test serialization
        let role = UserRole::Admin;
        let json = serde_json::to_string(&role).unwrap();
        assert_eq!(json, "\"admin\"");

        // Test deserialization
        let deserialized: UserRole = serde_json::from_str("\"maintainer\"").unwrap();
        assert_eq!(deserialized, UserRole::Maintainer);
    }

    #[test]
    fn test_role_hierarchy_is_proper_superset() {
        // Each role should be a proper superset of the previous one
        // Reader < Maintainer < Admin

        let reader = &*READER_PERMISSIONS;
        let maintainer = &*MAINTAINER_PERMISSIONS;
        let admin = &*ADMIN_PERMISSIONS;

        // Maintainer is a proper superset of Reader
        assert!(reader.is_subset(maintainer));
        assert!(!maintainer.is_subset(reader));
        assert!(maintainer.len() > reader.len());

        // Admin is a proper superset of Maintainer
        assert!(maintainer.is_subset(admin));
        assert!(!admin.is_subset(maintainer));
        assert!(admin.len() > maintainer.len());
    }
}
