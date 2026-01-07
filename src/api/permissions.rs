use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::str::FromStr;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
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

    // System
    SystemHealth,
    SystemAdmin,
}

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
            Permission::UsersRead => "users:read",
            Permission::UsersWrite => "users:write",
            Permission::UsersDelete => "users:delete",
            Permission::ApiKeysRead => "api-keys:read",
            Permission::ApiKeysWrite => "api-keys:write",
            Permission::ApiKeysDelete => "api-keys:delete",
            Permission::TasksRead => "tasks:read",
            Permission::TasksWrite => "tasks:write",
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
            "users:read" => Ok(Permission::UsersRead),
            "users:write" => Ok(Permission::UsersWrite),
            "users:delete" => Ok(Permission::UsersDelete),
            "api-keys:read" => Ok(Permission::ApiKeysRead),
            "api-keys:write" => Ok(Permission::ApiKeysWrite),
            "api-keys:delete" => Ok(Permission::ApiKeysDelete),
            "tasks:read" => Ok(Permission::TasksRead),
            "tasks:write" => Ok(Permission::TasksWrite),
            "system:health" => Ok(Permission::SystemHealth),
            "system:admin" => Ok(Permission::SystemAdmin),
            _ => Err(format!("Unknown permission: {}", s)),
        }
    }
}

/// Parse permissions from JSON string
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
    /// Read-only permissions (all read permissions)
    pub static ref READONLY_PERMISSIONS: HashSet<Permission> = {
        let mut set = HashSet::new();
        set.insert(Permission::LibrariesRead);
        set.insert(Permission::SeriesRead);
        set.insert(Permission::BooksRead);
        set.insert(Permission::PagesRead);
        set.insert(Permission::SystemHealth);
        set
    };

    /// Admin permissions (all permissions)
    pub static ref ADMIN_PERMISSIONS: HashSet<Permission> = {
        let mut set = HashSet::new();
        // Libraries
        set.insert(Permission::LibrariesRead);
        set.insert(Permission::LibrariesWrite);
        set.insert(Permission::LibrariesDelete);
        // Series
        set.insert(Permission::SeriesRead);
        set.insert(Permission::SeriesWrite);
        set.insert(Permission::SeriesDelete);
        // Books
        set.insert(Permission::BooksRead);
        set.insert(Permission::BooksWrite);
        set.insert(Permission::BooksDelete);
        // Pages
        set.insert(Permission::PagesRead);
        // Users
        set.insert(Permission::UsersRead);
        set.insert(Permission::UsersWrite);
        set.insert(Permission::UsersDelete);
        // API Keys
        set.insert(Permission::ApiKeysRead);
        set.insert(Permission::ApiKeysWrite);
        set.insert(Permission::ApiKeysDelete);
        // Tasks
        set.insert(Permission::TasksRead);
        set.insert(Permission::TasksWrite);
        // System
        set.insert(Permission::SystemHealth);
        set.insert(Permission::SystemAdmin);
        set
    };

    /// Reader permissions (read books, series, pages)
    pub static ref READER_PERMISSIONS: HashSet<Permission> = {
        let mut set = HashSet::new();
        set.insert(Permission::LibrariesRead);
        set.insert(Permission::SeriesRead);
        set.insert(Permission::BooksRead);
        set.insert(Permission::PagesRead);
        set.insert(Permission::SystemHealth);
        set
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_permission_as_str() {
        assert_eq!(Permission::LibrariesRead.as_str(), "libraries:read");
        assert_eq!(Permission::BooksWrite.as_str(), "books:write");
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
    }

    #[test]
    fn test_admin_permissions() {
        assert!(ADMIN_PERMISSIONS.contains(&Permission::SystemAdmin));
        assert!(ADMIN_PERMISSIONS.contains(&Permission::UsersWrite));
        assert_eq!(ADMIN_PERMISSIONS.len(), 20); // All permissions
    }

    #[test]
    fn test_reader_permissions() {
        assert!(READER_PERMISSIONS.contains(&Permission::PagesRead));
        assert!(!READER_PERMISSIONS.contains(&Permission::BooksWrite));
        assert_eq!(READER_PERMISSIONS.len(), 5);
    }
}
