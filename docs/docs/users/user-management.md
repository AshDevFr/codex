---
sidebar_position: 2
---

# User Management

Create and manage user accounts in Codex.

## Creating the Admin User

The first user is created during initial setup:

```bash
# Via CLI
codex seed --config codex.yaml
```

Or via the setup API:

```bash
curl -X POST http://localhost:8080/api/v1/setup/initialize \
  -H "Content-Type: application/json" \
  -d '{
    "username": "admin",
    "email": "admin@example.com",
    "password": "secure-password"
  }'
```

## Creating Additional Users

Admin users can create new accounts.

### Via Web Interface

1. Go to **Settings** > **Users**
2. Click **Add User**
3. Fill in username, email, password
4. Select permissions
5. Save

### Via API

```bash
curl -X POST http://localhost:8080/api/v1/users \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "username": "reader",
    "email": "reader@example.com",
    "password": "user-password",
    "role": "reader"
  }'
```

## User Properties

| Property | Description |
|----------|-------------|
| `username` | Unique login name |
| `email` | Email address (optional verification) |
| `password` | Hashed with Argon2 |
| `role` | User role: `reader`, `maintainer`, or `admin` |
| `permissions` | Custom permission overrides (optional) |
| `email_verified` | Email verification status |
| `created_at` | Account creation date |
| `updated_at` | Last modification date |

## Updating Users

```bash
curl -X PUT http://localhost:8080/api/v1/users/{id} \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "email": "newemail@example.com",
    "permissions": ["LibrariesRead", "BooksRead"]
  }'
```

## Deleting Users

```bash
curl -X DELETE http://localhost:8080/api/v1/users/{id} \
  -H "Authorization: Bearer $TOKEN"
```

:::warning
Deleting a user also deletes their reading progress and API keys.
:::

## User Preferences

Codex supports per-user preferences for customizing the user experience.

### Available Preferences

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `ui.theme` | string | `"system"` | Theme: "light", "dark", "system" |
| `ui.language` | string | `"en"` | UI language (BCP47 code) |
| `ui.sidebar_collapsed` | boolean | `false` | Sidebar state |
| `reader.default_zoom` | integer | `100` | Default zoom percentage |
| `reader.reading_direction` | string | `"auto"` | Reading direction |
| `reader.page_fit` | string | `"width"` | Page fit mode |
| `library.default_view` | string | `"grid"` | Default view mode |
| `library.default_page_size` | integer | `24` | Items per page |

### Managing via Web Interface

1. Go to **Settings** > **Profile**
2. Navigate to the **Preferences** tab
3. Adjust settings as needed
4. Changes are saved automatically

### Managing via API

```bash
# Get all preferences
curl http://localhost:8080/api/v1/user/preferences \
  -H "Authorization: Bearer $TOKEN"

# Set a preference
curl -X PUT http://localhost:8080/api/v1/user/preferences/ui.theme \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"value": "dark"}'

# Reset to default
curl -X DELETE http://localhost:8080/api/v1/user/preferences/ui.theme \
  -H "Authorization: Bearer $TOKEN"
```

