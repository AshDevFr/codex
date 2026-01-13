---
sidebar_position: 1
---

# Users & Permissions Overview

Codex includes a comprehensive user management system with fine-grained permissions. This guide covers user creation, permission management, and API keys.

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
4. Select permissions
5. Save

### Create API Key

1. Go to **Profile** > **API Keys**
2. Click **Create API Key**
3. Name the key and select permissions
4. Copy the key (shown only once!)

## Core Concepts

### User Types

| Type | Access | Use Case |
|------|--------|----------|
| Admin | Full system access | Server administrators |
| User | Permission-based | Regular readers |
| API Key | Permission-based | Automation, scripts |

### Authentication Methods

| Method | Use Case | Example |
|--------|----------|---------|
| JWT Token | Web UI, API clients | `Authorization: Bearer token` |
| API Key | Automation, services | `Authorization: Bearer codex_key` |
| Basic Auth | Simple clients, OPDS | `curl -u user:pass` |

## In This Section

- [User Management](./user-management) - Creating and managing user accounts
- [Permissions](./permissions) - Understanding the permission system
- [API Keys](./api-keys) - Creating and managing API keys
- [Authentication](./authentication) - Login methods and security
