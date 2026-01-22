use crate::api::error::{ApiError, ErrorResponse};
use crate::api::extractors::{AppState, AuthContext};
use crate::api::permissions::Permission;
use crate::require_permission;
use axum::{
    extract::{Query, State},
    Json,
};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema)]
#[schema(example = json!({
    "name": "Documents",
    "path": "/home/user/Documents",
    "is_directory": true,
    "is_readable": true
}))]
pub struct FileSystemEntry {
    /// Name of the file or directory
    pub name: String,
    /// Full path to the entry
    pub path: String,
    /// Whether this is a directory
    pub is_directory: bool,
    /// Whether the entry is readable
    pub is_readable: bool,
}

#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema)]
#[schema(example = json!({
    "current_path": "/home/user/Documents",
    "parent_path": "/home/user",
    "entries": [
        {"name": "Comics", "path": "/home/user/Documents/Comics", "is_directory": true, "is_readable": true},
        {"name": "Manga", "path": "/home/user/Documents/Manga", "is_directory": true, "is_readable": true}
    ]
}))]
pub struct BrowseResponse {
    /// Current directory path
    pub current_path: String,
    /// Parent directory path (None if at root)
    pub parent_path: Option<String>,
    /// List of entries in the current directory
    pub entries: Vec<FileSystemEntry>,
}

#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct BrowseQuery {
    /// Path to browse (defaults to user's home directory)
    path: Option<String>,
}

/// Browse filesystem directories
///
/// Returns a list of directories and files in the specified path
#[utoipa::path(
    get,
    path = "/api/v1/filesystem/browse",
    params(BrowseQuery),
    responses(
        (status = 200, description = "Directory contents", body = BrowseResponse),
        (status = 400, description = "Invalid path", body = ErrorResponse),
        (status = 403, description = "Forbidden", body = ErrorResponse),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "filesystem"
)]
pub async fn browse_filesystem(
    State(_state): State<Arc<AppState>>,
    auth: AuthContext,
    Query(query): Query<BrowseQuery>,
) -> Result<Json<BrowseResponse>, ApiError> {
    // Require admin permission to browse filesystem
    require_permission!(auth, Permission::LibrariesWrite)?;

    // Determine the path to browse
    let path = if let Some(p) = query.path {
        PathBuf::from(p)
    } else {
        // Default to home directory or root
        dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"))
    };

    // Validate path exists and is a directory
    if !path.exists() {
        return Err(ApiError::BadRequest(format!(
            "Path does not exist: {}",
            path.display()
        )));
    }

    if !path.is_dir() {
        return Err(ApiError::BadRequest(format!(
            "Path is not a directory: {}",
            path.display()
        )));
    }

    // Get parent directory
    let parent_path = path.parent().map(|p| p.to_string_lossy().to_string());

    // Read directory contents
    let mut entries = Vec::new();

    match fs::read_dir(&path) {
        Ok(read_dir) => {
            for entry in read_dir.flatten() {
                let entry_path = entry.path();
                let is_directory = entry_path.is_dir();

                // Skip hidden files (starting with .)
                if let Some(name) = entry_path.file_name() {
                    let name_str = name.to_string_lossy();
                    if name_str.starts_with('.') {
                        continue;
                    }

                    entries.push(FileSystemEntry {
                        name: name_str.to_string(),
                        path: entry_path.to_string_lossy().to_string(),
                        is_directory,
                        is_readable: entry_path
                            .metadata()
                            .map(|m| !m.permissions().readonly())
                            .unwrap_or(false),
                    });
                }
            }
        }
        Err(e) => {
            return Err(ApiError::Internal(format!(
                "Failed to read directory: {}",
                e
            )));
        }
    }

    // Sort entries: directories first, then alphabetically
    entries.sort_by(|a, b| match (a.is_directory, b.is_directory) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
    });

    Ok(Json(BrowseResponse {
        current_path: path.to_string_lossy().to_string(),
        parent_path,
        entries,
    }))
}

/// Get system drives/volumes
///
/// Returns a list of available drives or mount points on the system
#[utoipa::path(
    get,
    path = "/api/v1/filesystem/drives",
    responses(
        (status = 200, description = "Available drives", body = Vec<FileSystemEntry>),
        (status = 403, description = "Forbidden", body = ErrorResponse),
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "filesystem"
)]
pub async fn list_drives(
    State(_state): State<Arc<AppState>>,
    auth: AuthContext,
) -> Result<Json<Vec<FileSystemEntry>>, ApiError> {
    require_permission!(auth, Permission::LibrariesWrite)?;

    let mut drives = Vec::new();

    #[cfg(target_os = "windows")]
    {
        // On Windows, list all available drives
        for letter in b'A'..=b'Z' {
            let drive_path = format!("{}:\\", letter as char);
            let path = Path::new(&drive_path);
            if path.exists() {
                drives.push(FileSystemEntry {
                    name: drive_path.clone(),
                    path: drive_path,
                    is_directory: true,
                    is_readable: true,
                });
            }
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        // On Unix-like systems, start with common locations
        let common_locations = vec![
            ("/", "Root"),
            ("/home", "Home"),
            ("/mnt", "Mount Points"),
            ("/media", "Media"),
            ("/Volumes", "Volumes"), // macOS
        ];

        for (path_str, name) in common_locations {
            let path = Path::new(path_str);
            if path.exists() {
                drives.push(FileSystemEntry {
                    name: name.to_string(),
                    path: path_str.to_string(),
                    is_directory: true,
                    is_readable: path
                        .metadata()
                        .map(|m| !m.permissions().readonly())
                        .unwrap_or(false),
                });
            }
        }

        // Add user's home directory
        if let Some(home) = dirs::home_dir() {
            drives.push(FileSystemEntry {
                name: "Home Directory".to_string(),
                path: home.to_string_lossy().to_string(),
                is_directory: true,
                is_readable: true,
            });
        }
    }

    Ok(Json(drives))
}
