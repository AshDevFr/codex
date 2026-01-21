---
sidebar_position: 1
---

# Users & Permissions Overview

Codex includes a comprehensive user management system with role-based access control (RBAC), granular permissions, and content-level sharing restrictions.

## Quick Start

### Create Admin User

```bash
# Via CLI during setup
codex seed --config codex.yaml
```

### Add Users

1. Go to **Settings** > **Users**
2. Click **Add User**
3. Set username, email, password
4. Select a role (Reader, Maintainer, or Admin)
5. Save

### Create API Key

1. Go to **Settings** > **Profile** > **API Keys**
2. Click **Create API Key**
3. Name the key and select permissions
4. Copy the key (shown only once!)

## Core Concepts

### Roles

Codex uses three hierarchical roles:

| Role | Description | Permissions |
|------|-------------|-------------|
| **Reader** | Browse and read content | 8 permissions |
| **Maintainer** | Manage content and libraries | 15 permissions |
| **Admin** | Full system access | 20 permissions |

Each higher role includes all permissions from lower roles. See [Permissions & Roles](./permissions) for details.

### Permission Types

| Category | Permissions | Description |
|----------|-------------|-------------|
| Libraries | Read, Write, Delete | Access to library management |
| Series | Read, Write, Delete | Access to series metadata |
| Books | Read, Write, Delete | Access to books and reading |
| Pages | Read | Access to page images |
| Users | Read, Write, Delete | User management (Admin) |
| API Keys | Read, Write, Delete | API key management |
| Tasks | Read, Write | Background task access |
| System | Health, Admin | System-level access |

### Effective Permissions

A user's effective permissions combine their role with any custom permissions:

```
Effective = Role Permissions ∪ Custom Permissions
```

For API keys, permissions are further constrained:

```
API Key Effective = Effective ∩ Token Permissions
```

### Sharing Tags

For content-level restrictions (e.g., family sharing), use [Sharing Tags](./sharing-tags):

- **Allow grants** - User only sees content with allowed tags (whitelist mode)
- **Deny grants** - User sees everything except denied content
- Deny always overrides allow

### Authentication Methods

| Method | Use Case | Example |
|--------|----------|---------|
| JWT Token | Web UI, API clients | `Authorization: Bearer <token>` |
| API Key | Automation, OPDS | `X-API-Key: codex_...` or `Authorization: Bearer codex_...` |
| Basic Auth | Simple clients, OPDS | `curl -u user:pass` or `curl -u api:<api-key>` |

## In This Section

- [User Management](./user-management) - Creating and managing user accounts
- [Permissions & Roles](./permissions) - Understanding the role-based permission system
- [API Keys](./api-keys) - Creating and managing API keys with scoped permissions
- [Authentication](./authentication) - Login methods and security
- [Sharing Tags](./sharing-tags) - Content-level access control for family sharing
