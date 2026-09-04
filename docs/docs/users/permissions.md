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
- Browse collections and read lists
- Manage their own API keys
- Check system health

**Permissions (12 total):**
- `libraries-read`, `series-read`, `books-read`, `pages-read`
- `progress-read`, `progress-write`
- `collections-read`, `read-lists-read`
- `api-keys-read`, `api-keys-write`, `api-keys-delete`
- `system-health`

### Maintainer Role

For users who manage content but not system settings. Maintainers can do everything Readers can, plus:

- Create and modify libraries (but not delete them)
- Full series management (create, edit, delete)
- Full book management (create, edit, delete)
- Create, edit, and delete collections and read lists
- View and manage background tasks

**Additional Permissions (11 more, 23 total):**
- `libraries-write`
- `series-write`, `series-delete`
- `books-write`, `books-delete`
- `collections-write`, `collections-delete`
- `read-lists-write`, `read-lists-delete`
- `tasks-read`, `tasks-write`

### Admin Role

Full system access for server administrators. Admins can do everything Maintainers can, plus:

- Delete libraries
- Manage all users
- Manage metadata plugins
- Access system administration features
- Manage sharing tags and content restrictions

**Additional Permissions (6 more, 29 total):**
- `libraries-delete`
- `users-read`, `users-write`, `users-delete`
- `plugins-manage`
- `system-admin`

## All Permissions

Permission names on the wire are kebab-case (`libraries-read`). The API also
accepts the colon form (`libraries:read`) in requests, but always serves and
stores the kebab-case names.

### Library Permissions

| Permission | Description |
|------------|-------------|
| `libraries-read` | View libraries and their settings |
| `libraries-write` | Create and update libraries, trigger scans |
| `libraries-delete` | Delete libraries (Admin only) |

### Series Permissions

| Permission | Description |
|------------|-------------|
| `series-read` | View series and metadata; browse the want-to-read queue, recommendations, and series exports |
| `series-write` | Update series metadata, manage covers |
| `series-delete` | Delete series |

### Book Permissions

| Permission | Description |
|------------|-------------|
| `books-read` | View books and metadata |
| `books-write` | Update book metadata |
| `books-delete` | Delete books |

### Progress Permissions

Reading progress is user-scoped data with its own permission pair. A key
with only `progress-read` is the recipe for a stats-only integration: it can
call `/api/v1/reading-stats` and read progress and history, and nothing else.

| Permission | Description |
|------------|-------------|
| `progress-read` | View reading progress, read history, and reading stats |
| `progress-write` | Update progress, mark books/series read or unread, record reading sessions, clear history |

### Collection & Read List Permissions

Shared groupings of series (collections) and books (read lists). See [Collections & Read Lists](../collections-readlists.md). Read is part of the Reader role; write/delete are part of Maintainer.

| Permission | Description |
|------------|-------------|
| `collections-read` | Browse collections |
| `collections-write` | Create, rename, and manage collection members |
| `collections-delete` | Delete collections |
| `read-lists-read` | Browse read lists |
| `read-lists-write` | Create, edit, and manage read list members |
| `read-lists-delete` | Delete read lists |

### Page Permissions

| Permission | Description |
|------------|-------------|
| `pages-read` | View page images and thumbnails |

### User Permissions

| Permission | Description |
|------------|-------------|
| `users-read` | View user list and details (Admin only) |
| `users-write` | Create and update users (Admin only) |
| `users-delete` | Delete users (Admin only) |

### API Key Permissions

| Permission | Description |
|------------|-------------|
| `api-keys-read` | View own API keys |
| `api-keys-write` | Create API keys |
| `api-keys-delete` | Revoke API keys |

### Task Permissions

| Permission | Description |
|------------|-------------|
| `tasks-read` | View background tasks and queue status |
| `tasks-write` | Cancel tasks, trigger operations |

### Plugin Permissions

| Permission | Description |
|------------|-------------|
| `plugins-manage` | Install, configure, and manage metadata plugins (Admin only) |

### System Permissions

| Permission | Description |
|------------|-------------|
| `system-health` | View health status and metrics |
| `system-admin` | Full administrative access, server settings |

## Effective Permissions

A user's **effective permissions** are calculated by combining their role permissions with any custom permissions:

```
Effective Permissions = Role Permissions ∪ Custom Permissions
```

### Custom Permissions

Custom permissions allow extending a user's access beyond their role. For example, a Reader could be granted `tasks-read` to monitor scan progress without being promoted to Maintainer.

```json
{
  "username": "power-reader",
  "role": "reader",
  "permissions": ["tasks-read"]
}
```

This user would have all Reader permissions plus `tasks-read`.

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
  "permissions": ["libraries-read", "series-read", "books-read", "pages-read"]
}
```

Even though the Admin has all 29 permissions, this token only grants read access.

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
    "permissions": ["tasks-read"]
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
  "permissions": ["tasks-read"]
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
