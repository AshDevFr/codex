---
sidebar_position: 5
---

# Authentication

Codex supports multiple authentication methods for different use cases.

## Authentication Methods

### JWT Token

Primary method for web interface and API clients:

```bash
# Login
curl -X POST http://localhost:8080/api/v1/auth/login \
  -H "Content-Type: application/json" \
  -d '{"username":"user","password":"pass"}'
```

Response:
```json
{
  "token": "eyJhbGciOiJIUzI1NiIs...",
  "expires_at": "2024-01-16T10:00:00Z"
}
```

Use the token:
```bash
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

See [API Keys](./api-keys) for details.

### HTTP Basic Auth

For simple clients and OPDS:

```bash
curl -u "username:password" \
  http://localhost:8080/api/v1/libraries
```

### OIDC / Single Sign-On

For enterprise and homelab SSO via external identity providers (Authentik, Keycloak, etc.):

- Users click an OIDC provider button on the login page
- They authenticate at the external IdP
- Codex creates or links their account automatically

See [OIDC / Single Sign-On](./oidc) for setup instructions.

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

## JWT Configuration

```yaml
auth:
  jwt_secret: "your-secret-key"  # Required, use strong random value
  jwt_expiry_hours: 24           # Token lifetime
```

Generate a secure secret:
```bash
openssl rand -base64 32
```

## Security Best Practices

1. **Use HTTPS**: Always use TLS in production
2. **Strong JWT secret**: Use cryptographically random values
3. **Token expiry**: Set appropriate expiry times
4. **Secure cookies**: Use HttpOnly and Secure flags

## Troubleshooting

### Login Failed

1. Check username/password case sensitivity
2. Verify user account exists
3. Check email verification status (if enabled)
4. Review server logs for errors

### Token Expired

1. Re-authenticate to get new token
2. Consider longer expiry if needed
3. Implement token refresh in your client

### Invalid Token

1. Verify token is complete (not truncated)
2. Check JWT secret hasn't changed
3. Verify server clock is accurate
