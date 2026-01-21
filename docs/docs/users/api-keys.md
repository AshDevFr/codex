---
sidebar_position: 4
---

# API Keys

API keys provide authentication for automation, scripts, and third-party applications like OPDS readers.

## Key Concepts

- **API keys are scoped to permissions** - Each key has a specific set of permissions
- **Keys cannot exceed user permissions** - You can only grant permissions you have
- **Keys are constrained at request time** - If your role changes, your keys are automatically limited
- **Keys can have expiration dates** - Optional time-limited access

## Creating API Keys

### Via Web Interface

1. Go to **Settings** > **Profile** > **API Keys**
2. Click **Create API Key**
3. Enter a descriptive name
4. Select a permission preset or customize:
   - **Full Access** - All permissions you have
   - **Read Only** - Browse and read content
   - **OPDS/Reader Apps** - Minimal read-only access
   - **Custom** - Select individual permissions
5. Optionally set an expiration date
6. Click **Create**
7. **Copy the key immediately** - it's only shown once!

### Via API

```bash
curl -X POST http://localhost:8080/api/v1/api-keys \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "Automation Script",
    "permissions": ["LibrariesRead", "BooksRead", "PagesRead"]
  }'
```

Response:

```json
{
  "id": "uuid",
  "name": "Automation Script",
  "key": "codex_abc12345_secretpart123456789",
  "key_prefix": "codex_abc12345",
  "permissions": ["LibrariesRead", "BooksRead", "PagesRead"],
  "expires_at": null,
  "created_at": "2024-01-15T10:00:00Z"
}
```

:::danger
The full API key is only shown once! Store it securely immediately. If lost, you must create a new key.
:::

## Permission Constraints

### You Can Only Grant What You Have

API keys are constrained by your effective permissions:

```
Token Effective Permissions = (Your Role ∪ Your Custom) ∩ Token Requested
```

**Example:** A Maintainer (15 permissions) cannot create a key with `UsersRead` because that permission is Admin-only.

### Keys Are Validated at Request Time

If your role is downgraded, your existing keys are automatically constrained:

1. Admin creates key with `UsersRead` permission
2. Admin is demoted to Maintainer
3. Key requests to `/api/v1/users` now fail - `UsersRead` is no longer in the user's effective permissions

This ensures keys never grant more access than the user currently has.

### No Permissions = User's Full Access

If you don't specify permissions when creating a key, it inherits your full effective permissions:

```bash
# Creates key with all your permissions
curl -X POST http://localhost:8080/api/v1/api-keys \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"name": "Full Access Key"}'
```

## Using API Keys

### X-API-Key Header (Recommended)

```bash
curl -H "X-API-Key: codex_abc12345_secretpart" \
  http://localhost:8080/api/v1/libraries
```

### Bearer Token

```bash
curl -H "Authorization: Bearer codex_abc12345_secretpart" \
  http://localhost:8080/api/v1/libraries
```

### Basic Auth (for OPDS readers)

Many OPDS reader apps only support Basic Auth:

```
Username: api
Password: codex_abc12345_secretpart
```

Or with curl:

```bash
curl -u "api:codex_abc12345_secretpart" \
  http://localhost:8080/opds/v1.2/catalog
```

## Key Format

API keys follow the format: `codex_<prefix>_<secret>`

| Part | Description |
|------|-------------|
| `codex_` | Fixed prefix identifying Codex keys |
| `<prefix>` | 8-character identifier for quick lookup |
| `<secret>` | Random secret (hashed in database) |

The prefix is stored in plaintext for lookup, but the secret is hashed - Codex cannot recover lost keys.

## Permission Presets

### OPDS / Reader Apps

Minimal permissions for read-only access:

```json
{
  "name": "OPDS Reader",
  "permissions": ["LibrariesRead", "SeriesRead", "BooksRead", "PagesRead"]
}
```

### Automation Script

For scripts that trigger scans and monitor progress:

```json
{
  "name": "Scanner Script",
  "permissions": [
    "LibrariesRead",
    "LibrariesWrite",
    "TasksRead",
    "TasksWrite"
  ]
}
```

### Mobile App

Full reading experience with progress tracking:

```json
{
  "name": "Mobile App",
  "permissions": [
    "LibrariesRead",
    "SeriesRead",
    "BooksRead",
    "BooksWrite",
    "PagesRead"
  ]
}
```

### Metadata Manager

For tools that update metadata:

```json
{
  "name": "Metadata Tool",
  "permissions": [
    "LibrariesRead",
    "SeriesRead",
    "SeriesWrite",
    "BooksRead",
    "BooksWrite"
  ]
}
```

## Managing API Keys

### List Your Keys

```bash
curl http://localhost:8080/api/v1/api-keys \
  -H "Authorization: Bearer $TOKEN"
```

Response shows key metadata (not the secret):

```json
[
  {
    "id": "uuid",
    "name": "OPDS Reader",
    "key_prefix": "codex_abc12345",
    "permissions": ["LibrariesRead", "BooksRead"],
    "last_used_at": "2024-01-15T12:30:00Z",
    "expires_at": null,
    "created_at": "2024-01-01T10:00:00Z"
  }
]
```

### Revoke a Key

```bash
curl -X DELETE http://localhost:8080/api/v1/api-keys/{id} \
  -H "Authorization: Bearer $TOKEN"
```

Revoked keys are immediately invalid.

### Key with Expiration

```bash
curl -X POST http://localhost:8080/api/v1/api-keys \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "Temporary Access",
    "permissions": ["LibrariesRead", "BooksRead"],
    "expires_at": "2024-02-01T00:00:00Z"
  }'
```

Expired keys are automatically rejected.

## Best Practices

1. **Use minimal permissions** - Only grant what the application needs
2. **Descriptive names** - Name keys by purpose (e.g., "Panels iOS App", "Scan Script")
3. **Separate keys per app** - Don't share keys between applications
4. **Set expiration dates** - For temporary or guest access
5. **Monitor last_used_at** - Identify unused keys for cleanup
6. **Rotate periodically** - Create new keys and revoke old ones
7. **Never commit to VCS** - Use environment variables or secret managers
8. **Revoke compromised keys** - Delete immediately if exposed

## Troubleshooting

### Key Not Working

1. **Check the full key** - Ensure no truncation or extra whitespace
2. **Verify not revoked** - Check key still exists in your profile
3. **Check expiration** - Key may have expired
4. **Try different header** - Some clients handle headers differently

### Permission Denied

1. **Check key permissions** - View key in profile to see its permissions
2. **Check your role** - Your role may have changed since key creation
3. **Verify endpoint permissions** - Confirm which permission the endpoint requires

### "Invalid API key format"

The key must start with `codex_` and have the correct structure. Verify you're using the complete key.
