---
sidebar_position: 6
---

# OIDC / Single Sign-On

Codex supports OpenID Connect (OIDC) authentication, enabling users to sign in via external identity providers (IdPs) like Authentik, Keycloak, or any OIDC-compliant provider.

## Overview

OIDC authentication allows you to:

- **Single Sign-On**: Users authenticate with their existing IdP credentials
- **Multiple Providers**: Configure one or more identity providers simultaneously
- **Automatic User Creation**: New users are created on first OIDC login
- **Group-to-Role Mapping**: Map IdP groups to Codex roles (Admin, Maintainer, Reader)
- **Hybrid Mode**: OIDC and local authentication work side by side

## Configuration

Add the `oidc` section under `auth` in your `codex.yaml`:

```yaml
auth:
  oidc:
    enabled: true
    auto_create_users: true
    default_role: reader

    providers:
      my-provider:
        display_name: "My Identity Provider"
        issuer_url: "https://idp.example.com/application/o/codex/"
        client_id: "codex-client-id"
        client_secret: "codex-client-secret"
        scopes:
          - email
          - profile
          - groups
        role_mapping:
          admin:
            - codex-admins
          maintainer:
            - codex-editors
          reader:
            - codex-users
        groups_claim: "groups"
```

### Configuration Reference

#### Global OIDC Settings

| Setting | Default | Description |
|---------|---------|-------------|
| `enabled` | `false` | Enable OIDC authentication |
| `auto_create_users` | `true` | Create new Codex users on first OIDC login |
| `default_role` | `reader` | Default role when no group mapping matches (`reader`, `maintainer`, `admin`) |
| `redirect_uri_base` | auto-detected | Override the base URL for OAuth callbacks. Falls back to `application.base_url` if set. |

#### Provider Settings

| Setting | Required | Default | Description |
|---------|----------|---------|-------------|
| `display_name` | Yes | - | Name shown on the login button |
| `issuer_url` | Yes | - | OIDC discovery URL (provider's issuer URL) |
| `client_id` | Yes | - | OAuth2 client ID |
| `client_secret` | No | - | OAuth2 client secret |
| `client_secret_env` | No | - | Environment variable name containing the client secret |
| `scopes` | No | `[]` | Additional scopes to request (openid is always included) |
| `role_mapping` | No | `{}` | Map IdP groups to Codex roles |
| `groups_claim` | No | `groups` | JWT claim containing user's groups |
| `username_claim` | No | `preferred_username` | JWT claim for the username |
| `email_claim` | No | `email` | JWT claim for the email address |

### Environment Variable Overrides

All OIDC settings can be configured via environment variables:

```bash
# Global settings
CODEX_AUTH_OIDC_ENABLED=true
CODEX_AUTH_OIDC_AUTO_CREATE_USERS=true
CODEX_AUTH_OIDC_DEFAULT_ROLE=reader
CODEX_AUTH_OIDC_REDIRECT_URI_BASE="https://codex.example.com"

# Provider-specific settings
CODEX_AUTH_OIDC_PROVIDERS_AUTHENTIK_DISPLAY_NAME="Authentik"
CODEX_AUTH_OIDC_PROVIDERS_AUTHENTIK_ISSUER_URL="https://authentik.example.com/application/o/codex/"
CODEX_AUTH_OIDC_PROVIDERS_AUTHENTIK_CLIENT_ID="codex"
CODEX_AUTH_OIDC_PROVIDERS_AUTHENTIK_CLIENT_SECRET="your-secret"
CODEX_AUTH_OIDC_PROVIDERS_AUTHENTIK_CLIENT_SECRET_ENV="MY_OIDC_SECRET"
CODEX_AUTH_OIDC_PROVIDERS_AUTHENTIK_SCOPES="email, profile, groups"
CODEX_AUTH_OIDC_PROVIDERS_AUTHENTIK_GROUPS_CLAIM="groups"
CODEX_AUTH_OIDC_PROVIDERS_AUTHENTIK_USERNAME_CLAIM="preferred_username"
CODEX_AUTH_OIDC_PROVIDERS_AUTHENTIK_EMAIL_CLAIM="email"

# Role mapping (comma-separated group names per role)
CODEX_AUTH_OIDC_PROVIDERS_AUTHENTIK_ROLE_MAPPING_ADMIN="codex-admins, administrators"
CODEX_AUTH_OIDC_PROVIDERS_AUTHENTIK_ROLE_MAPPING_MAINTAINER="codex-editors"
CODEX_AUTH_OIDC_PROVIDERS_AUTHENTIK_ROLE_MAPPING_READER="codex-users, users"
```

Providers can also be created entirely via environment variables (no YAML needed). Setting `CODEX_AUTH_OIDC_PROVIDERS_<NAME>_ISSUER_URL` is sufficient to create a new provider entry.

:::tip
Use `client_secret_env` instead of `client_secret` to avoid storing secrets in configuration files:

```yaml
providers:
  authentik:
    client_secret_env: "CODEX_OIDC_AUTHENTIK_SECRET"
```

Then set the environment variable: `CODEX_OIDC_AUTHENTIK_SECRET=your-secret`
:::

## Provider Setup Guides

### Authentik

1. **Create an OAuth2/OpenID Provider** in Authentik:
   - Go to **Applications** > **Providers** > **Create**
   - Select **OAuth2/OpenID Provider**
   - Set **Name** to `Codex`
   - Set **Client type** to `Confidential`
   - Note the **Client ID** and **Client Secret**
   - Set **Redirect URIs** to: `https://your-codex-url/api/v1/auth/oidc/authentik/callback`
   - Under **Advanced protocol settings**, ensure **Scopes** include: `openid`, `email`, `profile`

2. **Create an Application** in Authentik:
   - Go to **Applications** > **Applications** > **Create**
   - Set **Name** to `Codex`
   - Select the provider created above
   - Note the **Slug** - it determines the issuer URL

3. **Configure groups** (optional):
   - Create groups like `codex-admins`, `codex-editors`, `codex-users`
   - Assign users to appropriate groups
   - Ensure the `groups` scope is enabled on the provider

4. **Configure Codex**:

```yaml
auth:
  oidc:
    enabled: true
    providers:
      authentik:
        display_name: "Authentik"
        issuer_url: "https://authentik.example.com/application/o/codex/"
        client_id: "your-client-id"
        client_secret: "your-client-secret"
        scopes:
          - email
          - profile
          - groups
        role_mapping:
          admin:
            - codex-admins
          maintainer:
            - codex-editors
          reader:
            - codex-users
        groups_claim: "groups"
```

### Keycloak

1. **Create a Realm** (or use an existing one)

2. **Create a Client**:
   - Go to **Clients** > **Create client**
   - Set **Client ID** to `codex`
   - Set **Client authentication** to **On** (confidential)
   - Set **Valid redirect URIs** to: `https://your-codex-url/api/v1/auth/oidc/keycloak/callback`
   - Under **Credentials**, note the **Client Secret**

3. **Configure groups** (optional):
   - Go to **Groups** > **Create group**
   - Create groups like `codex-admins`, `codex-editors`
   - Assign users to groups
   - Create a **Client Scope** that includes the `groups` claim in tokens:
     - Go to **Client scopes** > **Create client scope**
     - Name it `groups`, Protocol: `openid-connect`
     - Add a **Group Membership** mapper (mapper type: Group Membership, token claim name: `groups`, full group path: OFF)
     - Assign this scope to your client under **Client scopes** > **Default scopes**

4. **Configure Codex**:

```yaml
auth:
  oidc:
    enabled: true
    providers:
      keycloak:
        display_name: "Keycloak"
        issuer_url: "https://keycloak.example.com/realms/your-realm"
        client_id: "codex"
        client_secret: "your-client-secret"
        scopes:
          - email
          - profile
          - groups
        role_mapping:
          admin:
            - codex-admins
          maintainer:
            - codex-editors
          reader:
            - codex-users
        groups_claim: "groups"
```

## How It Works

### Authentication Flow

1. User visits the Codex login page and clicks an OIDC provider button
2. Codex generates an authorization URL and redirects the user to the IdP
3. User authenticates at the IdP (password, MFA, etc.)
4. IdP redirects back to Codex with an authorization code
5. Codex exchanges the code for tokens (with PKCE verification)
6. Codex validates the ID token and extracts user information
7. Codex finds or creates a local user account
8. Codex generates a local JWT and logs the user in

### User Matching

When a user authenticates via OIDC:

- **First login**: Codex looks for an existing user by email address. If found, the OIDC identity is linked. If not found and `auto_create_users` is enabled, a new user is created.
- **Subsequent logins**: Codex uses the stored OIDC connection (provider + subject ID) for fast lookup.

### Role Mapping

On every OIDC login, Codex syncs the user's role based on their IdP groups:

- Groups are checked against the `role_mapping` configuration
- The **highest privilege** matching role is assigned (admin > maintainer > reader)
- If no groups match, the `default_role` is used
- Role changes take effect immediately on the next login

### Username Generation

For new OIDC users, Codex generates a username using this priority:

1. `preferred_username` claim from the IdP
2. `name` (display name) claim
3. Email address prefix (before `@`)
4. Random username (`user_xxxx`)

If the username is already taken, a numeric suffix is appended (e.g., `johndoe_1`).

## Multiple Providers

You can configure multiple OIDC providers simultaneously. Each provider appears as a separate button on the login page:

```yaml
auth:
  oidc:
    enabled: true
    providers:
      authentik:
        display_name: "Company SSO"
        issuer_url: "https://sso.company.com/application/o/codex/"
        client_id: "codex"
        client_secret: "secret1"
        # ...

      keycloak:
        display_name: "Lab SSO"
        issuer_url: "https://auth.lab.local/realms/homelab"
        client_id: "codex"
        client_secret: "secret2"
        # ...
```

A user can link their account to multiple providers. The same Codex account is used as long as the email address matches.

## Security

### PKCE

Codex always uses PKCE (Proof Key for Code Exchange) for all OIDC flows, providing protection against authorization code interception attacks.

### State Parameter

A random state parameter is generated for each login attempt to prevent CSRF attacks. The state expires after 5 minutes.

### Token Handling

- OIDC tokens from the IdP are used only during the authentication exchange
- After authentication, Codex issues its own JWT token
- OIDC tokens are not stored long-term

## Troubleshooting

### "OIDC authentication is not enabled"

Ensure `auth.oidc.enabled` is set to `true` in your configuration and that at least one provider is configured.

### Discovery document fetch fails

- Verify the `issuer_url` is correct and accessible from the Codex server
- Check that the URL points to the OIDC discovery endpoint (usually `/.well-known/openid-configuration`)
- Ensure there are no firewall rules blocking the connection

### "Email is required for OIDC authentication"

The IdP must include the `email` claim in the ID token. Ensure:
- The `email` scope is requested
- The user has an email address set in the IdP
- The IdP is configured to include email in tokens

### "Account creation via OIDC is disabled"

Set `auto_create_users: true` in the OIDC configuration, or have an administrator create the user account first with a matching email address.

### User gets wrong role

Check your `role_mapping` configuration:
- Group names are **case-sensitive**
- Verify the groups are included in the ID token (check the `groups_claim` setting)
- Use the `groups` scope if your IdP requires it
- Check that the user is assigned to the correct groups in the IdP

### Redirect URI mismatch

The redirect URI registered in your IdP must exactly match:
```
https://your-codex-url/api/v1/auth/oidc/{provider-name}/callback
```

If Codex is behind a reverse proxy, set `application.base_url` to your public URL:
```yaml
application:
  base_url: "https://codex.example.com"
```

Alternatively, you can override the OIDC redirect URI base specifically:
```yaml
auth:
  oidc:
    redirect_uri_base: "https://codex.example.com"
```
