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
- **API Bearer Tokens**: Provider-issued access tokens authenticate API requests directly (see [API Bearer Tokens](#api-bearer-tokens-resource-server))

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
| `accepted_audiences` | No | `[client_id]` | Audiences accepted on API bearer tokens from this provider (see [API Bearer Tokens](#api-bearer-tokens-resource-server)) |

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
CODEX_AUTH_OIDC_PROVIDERS_AUTHENTIK_ACCEPTED_AUDIENCES="codex-client, other-trusted-client"

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

## API Bearer Tokens (Resource Server)

Besides web sign-in, Codex accepts access tokens issued by your configured providers as API credentials:

```bash
curl -H "Authorization: Bearer $IDP_ACCESS_TOKEN" https://codex.example.com/api/v1/auth/me
```

This makes Codex an OAuth2 resource server: applications that already hold a user's IdP token (a reverse proxy doing forward-auth, an MCP service, another app in your SSO ecosystem) can call the Codex API as that user without managing Codex API keys.

### How Validation Works

1. The token's signing algorithm is checked. Only asymmetric algorithms (RS256, ES256) are accepted on this path; Codex's own session tokens are HS256 and verified separately, so the two can never be confused.
2. The token's `iss` claim selects the matching configured provider (trailing slashes are tolerated).
3. The signature is verified against the provider's published JWKS, fetched via OIDC discovery and cached. Key rotation at the IdP is picked up automatically, no restart needed.
4. The `aud` claim must match one of the provider's `accepted_audiences`, and `exp`/`nbf` are enforced with a small clock-skew allowance. Tokens without `aud` or `exp` are rejected.
5. The token's `sub` is resolved to the Codex user who linked that identity.

The feature needs no extra configuration: it is active whenever `auth.oidc.enabled` is `true` with at least one provider. With OIDC disabled, bearer authentication behaves exactly as before.

### Linking Requirement

There is **no auto-provisioning from API tokens**: the user must have signed into Codex web via SSO at least once so that the identity link exists. A valid token for an unlinked identity gets a `401` asking the user to sign in via SSO once. This is deliberate: a valid IdP token proves who the caller is at the IdP, but it should not silently create Codex accounts for everyone in your organization.

Two related limitations:

- Role/group sync happens only at web login. A bearer token's `groups` claim is ignored; role changes at the IdP take effect on the user's next web sign-in.
- Tokens are accepted from configured providers only. There is no way to accept tokens from an issuer you have not configured.

### Audience Rules

OAuth2 access tokens carry an `aud` (audience) claim naming the client they were issued to. By default Codex only accepts tokens whose audience is the provider's own `client_id`, which covers tokens obtained through Codex's sign-in flow.

If another application obtains tokens under its own client ID and forwards them to Codex, add that client ID to `accepted_audiences`:

```yaml
auth:
  oidc:
    enabled: true
    providers:
      authentik:
        issuer_url: "https://authentik.example.com/application/o/codex/"
        client_id: "codex-client-id"
        accepted_audiences:
          - "codex-client-id"        # keep accepting Codex's own tokens
          - "shared-apps-client-id"  # tokens minted for a trusted app
```

Only list clients you trust to act on behalf of their users: accepting an audience means any valid token minted for that client authenticates against Codex.

### Troubleshooting

Rejected tokens return a generic `401 Invalid bearer token`; the precise reason is logged server-side (debug level) and never echoed to the caller. Look for `Rejected IdP bearer token` in the logs:

| Logged reason | Likely cause | Fix |
|---------------|--------------|-----|
| `unsupported signing algorithm` | Token is HS256 or another symmetric/unsupported algorithm | Configure the IdP to sign access tokens with RS256 or ES256 |
| `token issuer does not match any configured provider` | The token's `iss` is not a configured `issuer_url` | Add the provider, or fix `issuer_url` (compare with the token's `iss` claim) |
| `wrong audience` or `token missing required claim` for `aud` | Token minted for a client not in `accepted_audiences`, or the IdP omits `aud` | Add the requesting app's client ID to `accepted_audiences`; ensure the IdP stamps an audience |
| `token expired` | The access token's `exp` has passed | Obtain a fresh token; check for clock skew beyond ~30s |
| `no JWKS key matches the token's key id` | Token signed with a key the IdP no longer publishes | Re-issue the token; verify the issuer's `jwks_uri` is reachable from Codex |
| `signature verification failed` | Token tampered with, or signed by a different issuer's key | Verify the token actually comes from the configured provider |
| `No Codex account is linked to this identity` (response message) | The user never signed into Codex web via SSO | Sign into the Codex web UI through the provider once |

If the IdP itself is unreachable (discovery or JWKS fetch fails), Codex returns `503 Identity provider is unreachable` rather than `401`, and logs `IdP bearer validation failed on the IdP side` at warn level.

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
