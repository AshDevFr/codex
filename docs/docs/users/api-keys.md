---
sidebar_position: 4
---

# API Keys

API keys provide authentication for automation, scripts, and third-party applications.

## Creating API Keys

### Via Web Interface

1. Go to **Profile** > **API Keys**
2. Click **Create API Key**
3. Enter a name
4. Select permissions
5. Copy the generated key (shown only once!)

### Via API

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

## Using API Keys

### As Bearer Token

```bash
curl -H "Authorization: Bearer codex_abc12345_secretpart" \
  http://localhost:8080/api/v1/libraries
```

### As X-API-Key Header

```bash
curl -H "X-API-Key: codex_abc12345_secretpart" \
  http://localhost:8080/api/v1/libraries
```

### As Basic Auth (for OPDS)

```
Username: api
Password: codex_abc12345_secretpart
```

## API Key Permissions

API keys can only have permissions that the creating user has. You cannot create an API key with more permissions than your account.

## Managing API Keys

### List API Keys

```bash
curl http://localhost:8080/api/v1/api-keys \
  -H "Authorization: Bearer $TOKEN"
```

### Revoke API Key

```bash
curl -X DELETE http://localhost:8080/api/v1/api-keys/{id} \
  -H "Authorization: Bearer $TOKEN"
```

## Best Practices

1. **Minimal permissions**: Only grant permissions the key needs
2. **Descriptive names**: Name keys by their purpose
3. **Regular rotation**: Regenerate keys periodically
4. **Secure storage**: Never commit keys to version control
5. **Revoke unused keys**: Delete keys no longer in use
6. **Separate keys**: Use different keys for different applications

## Common Use Cases

### OPDS Reader

```json
{
  "name": "OPDS Reader",
  "permissions": ["LibrariesRead", "SeriesRead", "BooksRead", "PagesRead"]
}
```

### Automation Script

```json
{
  "name": "Library Scanner Script",
  "permissions": ["LibrariesRead", "LibrariesWrite", "TasksRead", "TasksWrite"]
}
```

### Mobile App

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

## Troubleshooting

### API Key Not Working

1. Verify key is copied correctly (no extra spaces)
2. Check key hasn't been revoked
3. Verify key has required permissions
4. Try different auth method (header vs Bearer)

### Permission Denied

1. Check key has the required permission
2. Verify endpoint requires the permission you expect
3. Check server logs for details
