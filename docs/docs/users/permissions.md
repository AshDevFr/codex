---
sidebar_position: 3
---

# Permissions & Roles

Codex uses a role-based access control (RBAC) system with granular permissions. Users are assigned a role that grants a base set of permissions, which can be extended with custom permissions.

## Roles

Codex has three predefined roles with hierarchical permission sets:

| Role | Description | Use Case |
|------|-------------|----------|
| **Reader** | Read-only content access | Regular users who browse and read |
| **Maintainer** | Content management | Users who organize libraries and metadata |
| **Admin** | Full system access | Server administrators |

### Role Hierarchy

Roles follow a strict hierarchy where each higher role includes all permissions from lower roles:

```
Reader ⊂ Maintainer ⊂ Admin
```

### Reader Role

The default role for new users. Readers can:

- Browse libraries, series, and books
- Read books and view pages
- Track reading progress
- Manage their own API keys
- Check system health

**Permissions (8 total):**
- `LibrariesRead`, `SeriesRead`, `BooksRead`, `PagesRead`
- `ApiKeysRead`, `ApiKeysWrite`, `ApiKeysDelete`
- `SystemHealth`

### Maintainer Role

For users who manage content but not system settings. Maintainers can do everything Readers can, plus:

- Create and modify libraries (but not delete them)
- Full series management (create, edit, delete)
- Full book management (create, edit, delete)
- View and manage background tasks

**Additional Permissions (7 more, 15 total):**
- `LibrariesWrite`
- `SeriesWrite`, `SeriesDelete`
- `BooksWrite`, `BooksDelete`
- `TasksRead`, `TasksWrite`

### Admin Role

Full system access for server administrators. Admins can do everything Maintainers can, plus:

- Delete libraries
- Manage all users
- Access system administration features
- Manage sharing tags and content restrictions

**Additional Permissions (5 more, 20 total):**
- `LibrariesDelete`
- `UsersRead`, `UsersWrite`, `UsersDelete`
- `SystemAdmin`

## All Permissions

### Library Permissions

| Permission | Description |
|------------|-------------|
| `LibrariesRead` | View libraries and their settings |
| `LibrariesWrite` | Create and update libraries, trigger scans |
| `LibrariesDelete` | Delete libraries (Admin only) |

### Series Permissions

| Permission | Description |
|------------|-------------|
| `SeriesRead` | View series and metadata |
| `SeriesWrite` | Update series metadata, manage covers |
| `SeriesDelete` | Delete series |

### Book Permissions

| Permission | Description |
|------------|-------------|
| `BooksRead` | View books, metadata, and reading progress |
| `BooksWrite` | Update book metadata, mark as read |
| `BooksDelete` | Delete books |

### Page Permissions

| Permission | Description |
|------------|-------------|
| `PagesRead` | View page images and thumbnails |

### User Permissions

| Permission | Description |
|------------|-------------|
| `UsersRead` | View user list and details (Admin only) |
| `UsersWrite` | Create and update users (Admin only) |
| `UsersDelete` | Delete users (Admin only) |

### API Key Permissions

| Permission | Description |
|------------|-------------|
| `ApiKeysRead` | View own API keys |
| `ApiKeysWrite` | Create API keys |
| `ApiKeysDelete` | Revoke API keys |

### Task Permissions

| Permission | Description |
|------------|-------------|
| `TasksRead` | View background tasks and queue status |
| `TasksWrite` | Cancel tasks, trigger operations |

### System Permissions

| Permission | Description |
|------------|-------------|
| `SystemHealth` | View health status and metrics |
| `SystemAdmin` | Full administrative access, server settings |

## Effective Permissions

A user's **effective permissions** are calculated by combining their role permissions with any custom permissions:

```
Effective Permissions = Role Permissions ∪ Custom Permissions
```

### Custom Permissions

Custom permissions allow extending a user's access beyond their role. For example, a Reader could be granted `TasksRead` to monitor scan progress without being promoted to Maintainer.

```json
{
  "username": "power-reader",
  "role": "reader",
  "permissions": ["TasksRead"]
}
```

This user would have all Reader permissions plus `TasksRead`.

:::tip
Custom permissions extend roles - they never restrict. To limit access, use [Sharing Tags](./sharing-tags) for content-level restrictions.
:::

## API Token Permissions

When using API keys, effective permissions are further constrained by the token's permission set:

```
API Token Effective = (Role ∪ Custom) ∩ Token Permissions
```

This means:

1. **Tokens cannot exceed user permissions** - You can only grant permissions you have
2. **Tokens can be more restrictive** - Create limited tokens for specific use cases
3. **Changes apply immediately** - If a user's role changes, their tokens are constrained accordingly

See [API Keys](./api-keys) for details on creating tokens with specific permissions.

### Example: Limited Token

An Admin creating a read-only token for OPDS readers:

```json
{
  "name": "OPDS Reader",
  "permissions": ["LibrariesRead", "SeriesRead", "BooksRead", "PagesRead"]
}
```

Even though the Admin has all 20 permissions, this token only grants read access.

## Permission Presets

### Read-Only User (Reader Role)

Default for users who only need to browse and read:

```bash
curl -X POST http://localhost:8080/api/v1/users \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "username": "reader",
    "email": "reader@example.com",
    "password": "secure-password",
    "role": "reader"
  }'
```

### Content Manager (Maintainer Role)

For users who manage libraries and metadata:

```bash
curl -X POST http://localhost:8080/api/v1/users \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "username": "librarian",
    "email": "librarian@example.com",
    "password": "secure-password",
    "role": "maintainer"
  }'
```

### Reader with Task Access

A Reader who can monitor scans:

```bash
curl -X POST http://localhost:8080/api/v1/users \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "username": "power-reader",
    "email": "power@example.com",
    "password": "secure-password",
    "role": "reader",
    "permissions": ["TasksRead"]
  }'
```

## Best Practices

1. **Use roles over custom permissions** - Roles are easier to audit and maintain
2. **Minimal permissions** - Grant only what's necessary
3. **Limit admin accounts** - Only essential personnel should be admins
4. **Use separate accounts** - Admins should have a regular account for daily use
5. **Regular audits** - Review user permissions periodically
6. **Use sharing tags for content** - Don't rely on permissions for content restrictions

## Checking Permissions

### Via API

Get current user's effective permissions:

```bash
curl http://localhost:8080/api/v1/user \
  -H "Authorization: Bearer $TOKEN"
```

Response includes role and permissions:

```json
{
  "id": "uuid",
  "username": "user",
  "role": "maintainer",
  "permissions": ["TasksRead"]
}
```

### Permission Errors

When a request lacks required permissions, the API returns:

```json
{
  "error": "Forbidden",
  "message": "Missing required permission: LibrariesDelete"
}
```
