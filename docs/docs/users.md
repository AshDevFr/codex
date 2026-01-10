---
---

# Users & Permissions

Codex includes a comprehensive user management system with fine-grained permissions. This guide covers user creation, permission management, and API keys.

## User Management

### Creating the Admin User

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

### Creating Additional Users

Admin users can create new accounts:

#### Via Web Interface

1. Go to **Settings** > **Users**
2. Click **Add User**
3. Fill in username, email, password
4. Select permissions
5. Save

#### Via API

```bash
curl -X POST http://localhost:8080/api/v1/users \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "username": "reader",
    "email": "reader@example.com",
    "password": "user-password",
    "is_admin": false,
    "permissions": ["LibrariesRead", "SeriesRead", "BooksRead", "PagesRead"]
  }'
```

### User Properties

| Property | Description |
|----------|-------------|
| `username` | Unique login name |
| `email` | Email address (optional verification) |
| `password` | Hashed with Argon2 |
| `is_admin` | Full system access |
| `permissions` | Granular permission list |
| `email_verified` | Email verification status |
| `created_at` | Account creation date |
| `updated_at` | Last modification date |

### Updating Users

```bash
curl -X PUT http://localhost:8080/api/v1/users/{id} \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "email": "newemail@example.com",
    "permissions": ["LibrariesRead", "BooksRead"]
  }'
```

### Deleting Users

```bash
curl -X DELETE http://localhost:8080/api/v1/users/{id} \
  -H "Authorization: Bearer $TOKEN"
```

:::warning
Deleting a user also deletes their reading progress and API keys.
:::

## Permission System

Codex uses a granular permission system for access control.

### Permission Categories

#### Library Permissions

| Permission | Description |
|------------|-------------|
| `LibrariesRead` | View libraries |
| `LibrariesWrite` | Create and update libraries |
| `LibrariesDelete` | Delete libraries |

#### Series Permissions

| Permission | Description |
|------------|-------------|
| `SeriesRead` | View series |
| `SeriesWrite` | Update series metadata |
| `SeriesDelete` | Delete series |

#### Book Permissions

| Permission | Description |
|------------|-------------|
| `BooksRead` | View books and metadata |
| `BooksWrite` | Update book metadata, reading progress |
| `BooksDelete` | Delete books |

#### Page Permissions

| Permission | Description |
|------------|-------------|
| `PagesRead` | View page images |

#### User Permissions (Admin)

| Permission | Description |
|------------|-------------|
| `UsersRead` | View user list |
| `UsersWrite` | Create and update users |
| `UsersDelete` | Delete users |

#### API Key Permissions

| Permission | Description |
|------------|-------------|
| `ApiKeysRead` | View own API keys |
| `ApiKeysWrite` | Create and update API keys |
| `ApiKeysDelete` | Delete API keys |

#### Task Permissions

| Permission | Description |
|------------|-------------|
| `TasksRead` | View background tasks |
| `TasksWrite` | Manage/cancel tasks |

#### System Permissions

| Permission | Description |
|------------|-------------|
| `SystemHealth` | View health/metrics |
| `SystemAdmin` | Full administrative access |

### Admin Users

Admin users (`is_admin: true`) have full access to all features regardless of individual permissions. Use sparingly for security.

### Permission Presets

#### Read-Only User

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

#### Power User

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

#### Library Manager

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

## API Keys

API keys provide authentication for automation, scripts, and third-party applications.

### Creating API Keys

#### Via Web Interface

1. Go to **Profile** > **API Keys**
2. Click **Create API Key**
3. Enter a name
4. Select permissions
5. Copy the generated key (shown only once!)

#### Via API

```bash
curl -X POST http://localhost:8080/api/v1/api-keys \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "Automation Script",
    "permissions": ["LibrariesRead", "BooksRead"]
  }'
```

Response:

```json
{
  "id": "uuid",
  "name": "Automation Script",
  "key": "codex_abc12345_secretpart123456789",
  "key_prefix": "abc12345",
  "permissions": ["LibrariesRead", "BooksRead"],
  "created_at": "2024-01-15T10:00:00Z"
}
```

:::danger
The full API key is only shown once! Store it securely immediately.
:::

### Using API Keys

#### As Bearer Token

```bash
curl -H "Authorization: Bearer codex_abc12345_secretpart" \
  http://localhost:8080/api/v1/libraries
```

#### As X-API-Key Header

```bash
curl -H "X-API-Key: codex_abc12345_secretpart" \
  http://localhost:8080/api/v1/libraries
```

#### As Basic Auth (for OPDS)

```
Username: api
Password: codex_abc12345_secretpart
```

### API Key Permissions

API keys can only have permissions that the creating user has. You cannot create an API key with more permissions than your account.

### Managing API Keys

#### List API Keys

```bash
curl http://localhost:8080/api/v1/api-keys \
  -H "Authorization: Bearer $TOKEN"
```

#### Revoke API Key

```bash
curl -X DELETE http://localhost:8080/api/v1/api-keys/{id} \
  -H "Authorization: Bearer $TOKEN"
```

### API Key Best Practices

1. **Minimal permissions**: Only grant permissions the key needs
2. **Descriptive names**: Name keys by their purpose
3. **Regular rotation**: Regenerate keys periodically
4. **Secure storage**: Never commit keys to version control
5. **Revoke unused keys**: Delete keys no longer in use

## Authentication Methods

### JWT Token

Primary method for web interface and API clients:

```bash
# Login
curl -X POST http://localhost:8080/api/v1/auth/login \
  -H "Content-Type: application/json" \
  -d '{"username":"user","password":"pass"}'

# Use token
curl -H "Authorization: Bearer $TOKEN" \
  http://localhost:8080/api/v1/libraries
```

Token properties:
- Default expiry: 24 hours (configurable)
- Stateless (no server-side storage)
- Contains user ID and permissions

### API Key

For automation and service accounts:

```bash
curl -H "Authorization: Bearer codex_key_here" \
  http://localhost:8080/api/v1/libraries
```

### HTTP Basic Auth

For simple clients and OPDS:

```bash
curl -u "username:password" \
  http://localhost:8080/api/v1/libraries
```

## Email Verification

Optional email verification can be enabled:

```yaml
auth:
  email_confirmation_required: true
```

### Verification Flow

1. User registers
2. Verification email sent
3. User clicks verification link
4. Account activated

### Email Configuration

```yaml
email:
  smtp_host: smtp.example.com
  smtp_port: 587
  smtp_username: noreply@example.com
  smtp_password: smtp-password
  smtp_from_email: noreply@example.com
  smtp_from_name: Codex
  verification_token_expiry_hours: 24
  verification_url_base: http://localhost:8080
```

### Resend Verification

```bash
curl -X POST http://localhost:8080/api/v1/auth/resend-verification \
  -H "Content-Type: application/json" \
  -d '{"email":"user@example.com"}'
```

## Password Security

### Password Hashing

Passwords are hashed using Argon2id with configurable parameters:

```yaml
auth:
  argon2_memory_cost: 19456   # 19 MB
  argon2_time_cost: 2         # Iterations
  argon2_parallelism: 1       # Threads
```

### Password Requirements

Default requirements:
- Minimum 8 characters
- Recommended: mix of letters, numbers, symbols

### Password Reset

Currently, password reset is admin-managed:

```bash
# Admin updates user password
curl -X PUT http://localhost:8080/api/v1/users/{id} \
  -H "Authorization: Bearer $ADMIN_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"password":"new-password"}'
```

## Security Best Practices

### User Management

1. **Minimal permissions**: Grant only necessary permissions
2. **Regular audits**: Review user permissions periodically
3. **Disable unused accounts**: Remove or disable inactive users
4. **Strong passwords**: Enforce password complexity

### API Keys

1. **Purpose-specific keys**: Create separate keys for different uses
2. **Limited scope**: Minimize permissions per key
3. **Rotation policy**: Regenerate keys periodically
4. **Secure transmission**: Always use HTTPS

### Admin Accounts

1. **Limit admin users**: Only essential personnel
2. **Strong credentials**: Use unique, complex passwords
3. **Monitor access**: Review admin activity
4. **Separate duties**: Use non-admin accounts for daily use

## Troubleshooting

### Login Failed

1. Check username/password case sensitivity
2. Verify user account exists
3. Check email verification status (if enabled)
4. Review server logs for errors

### Permission Denied

1. Check user has required permission
2. Verify token hasn't expired
3. Check API key has necessary permissions
4. Admin override may be needed

### API Key Not Working

1. Verify key is copied correctly (no extra spaces)
2. Check key hasn't been revoked
3. Verify key has required permissions
4. Try different auth method (header vs Bearer)

### Email Verification Issues

1. Check SMTP configuration
2. Verify email isn't in spam
3. Check verification token hasn't expired
4. Try resending verification email

## Next Steps

- [API documentation](./api)
- [Configure OPDS](./opds)
- [Troubleshooting](./troubleshooting)
