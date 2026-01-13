---
sidebar_position: 3
---

# Permissions

Codex uses a granular permission system for access control.

## Permission Categories

### Library Permissions

| Permission | Description |
|------------|-------------|
| `LibrariesRead` | View libraries |
| `LibrariesWrite` | Create and update libraries |
| `LibrariesDelete` | Delete libraries |

### Series Permissions

| Permission | Description |
|------------|-------------|
| `SeriesRead` | View series |
| `SeriesWrite` | Update series metadata |
| `SeriesDelete` | Delete series |

### Book Permissions

| Permission | Description |
|------------|-------------|
| `BooksRead` | View books and metadata |
| `BooksWrite` | Update book metadata, reading progress |
| `BooksDelete` | Delete books |

### Page Permissions

| Permission | Description |
|------------|-------------|
| `PagesRead` | View page images |

### User Permissions (Admin)

| Permission | Description |
|------------|-------------|
| `UsersRead` | View user list |
| `UsersWrite` | Create and update users |
| `UsersDelete` | Delete users |

### API Key Permissions

| Permission | Description |
|------------|-------------|
| `ApiKeysRead` | View own API keys |
| `ApiKeysWrite` | Create and update API keys |
| `ApiKeysDelete` | Delete API keys |

### Task Permissions

| Permission | Description |
|------------|-------------|
| `TasksRead` | View background tasks |
| `TasksWrite` | Manage/cancel tasks |

### System Permissions

| Permission | Description |
|------------|-------------|
| `SystemHealth` | View health/metrics |
| `SystemAdmin` | Full administrative access |

## Admin Users

Admin users (`is_admin: true`) have full access to all features regardless of individual permissions. Use sparingly for security.

## Permission Presets

### Read-Only User

For users who only need to browse and read:

```json
{
  "permissions": [
    "LibrariesRead",
    "SeriesRead",
    "BooksRead",
    "PagesRead"
  ]
}
```

### Power User

For users who can manage their own content:

```json
{
  "permissions": [
    "LibrariesRead",
    "SeriesRead",
    "SeriesWrite",
    "BooksRead",
    "BooksWrite",
    "PagesRead",
    "ApiKeysRead",
    "ApiKeysWrite",
    "ApiKeysDelete"
  ]
}
```

### Library Manager

For users who manage libraries but not users:

```json
{
  "permissions": [
    "LibrariesRead",
    "LibrariesWrite",
    "SeriesRead",
    "SeriesWrite",
    "BooksRead",
    "BooksWrite",
    "BooksDelete",
    "PagesRead",
    "TasksRead",
    "TasksWrite"
  ]
}
```

## Best Practices

1. **Minimal permissions**: Grant only necessary permissions
2. **Regular audits**: Review user permissions periodically
3. **Disable unused accounts**: Remove or disable inactive users
4. **Limit admin users**: Only essential personnel should be admins
5. **Separate duties**: Use non-admin accounts for daily use
