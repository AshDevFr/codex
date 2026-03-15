---
sidebar_position: 15
---

# Third-Party Apps

Codex supports integration with third-party reading apps through its Komga-compatible API. This allows you to use popular mobile apps designed for Komga with your Codex server.

## Supported Apps

### Komic (iOS)

[Komic](https://apps.apple.com/app/komic/id1512988981) is a popular iOS app for reading comics and manga. It supports Komga servers and works with Codex through the compatibility layer.

**Setup:**

1. Enable the Komga API in your Codex configuration:
   ```yaml
   komga_api:
     enabled: true
   ```

2. In Komic, add a new server:
   - **Server URL**: `http://your-server:8080/komga`
   - **Username**: Your Codex username
   - **Password**: Your Codex password

3. Komic will authenticate using Basic Auth and you'll be able to browse your library.

### Other Komga-Compatible Apps

While Codex is primarily tested with Komic, other Komga-compatible apps may also work:

- **Mihon** (Android) - Tachiyomi fork with Komga extension
- **Tachiyomi forks** - Various forks with Komga support

:::note
Compatibility with apps other than Komic is not officially tested. Your experience may vary.
:::

### KOReader

[KOReader](https://koreader.rocks/) is an open-source e-book reader for E Ink devices and other platforms. Codex supports the KOReader sync protocol, allowing you to sync reading progress between KOReader and Codex.

**Supported formats:** EPUB, PDF, CBZ, CBR

#### Prerequisites

1. **Enable the KOReader API** in your Codex configuration (see [Enabling the KOReader API](#enabling-the-koreader-api) below)
2. **Create an API key** in Codex (see [API Keys](./users/api-keys))
3. **Run a deep scan** so Codex computes KOReader-compatible hashes for your books (see [Deep Scan](./libraries#deep-scan))

#### Setup in KOReader

1. Open a book in KOReader
2. Go to **Top Menu** > **Tools** (🔧) > **Progress sync**
3. Select **Custom sync server**
4. Enter the server settings:
   - **Server URL**: `http://your-server:8080/koreader`
   - **Username**: Your Codex **API key** (e.g., `codex_abc12345_secretpart123456789`)
   - **Password**: Any value (ignored by Codex)
5. Tap **Login** to verify the connection

:::info
KOReader uses the `x-auth-user` header to send the username, which Codex treats as an API key. The password field (`x-auth-key`) is ignored because KOReader MD5-hashes the password before sending it, making direct password verification impossible.
:::

#### How It Works

KOReader identifies books by computing an MD5 hash of the first 4096 bytes of the file. When you enable the KOReader API and run a **deep scan**, Codex computes the same hash for each book and stores it. This allows KOReader to look up books and sync progress.

- **Progress sync is per-user**: Each user's reading progress is tracked independently
- **EPUB progress**: Codex converts between KOReader's DocFragment format and its internal position tracking
- **PDF/CBZ/CBR progress**: Page numbers are synced directly

#### Troubleshooting KOReader

**"Login failed" or 401 Unauthorized:**
- Make sure you're using a Codex **API key** as the username, not your regular username/password
- Verify the API key hasn't expired or been revoked
- Check that `koreader_api.enabled` is `true` in your config

**"Book not found" (404):**
- Run a **deep scan** on your library so Codex computes KOReader hashes
- The book must be in a Codex library; KOReader identifies books by file hash, not filename

**Progress not syncing:**
- Ensure both devices are using the same Codex server and user account
- Check that the book files are identical (same hash) across devices

## Enabling the KOReader API

The KOReader sync API is disabled by default. To enable it:

### Via Configuration File

```yaml
# codex.yaml
koreader_api:
  enabled: true
```

### Via Environment Variables

```bash
CODEX_KOREADER_API_ENABLED=true
```

After enabling, restart Codex and run a **deep scan** on your libraries to compute KOReader-compatible file hashes.

## Enabling the Komga API

The Komga-compatible API is disabled by default for security. To enable it:

### Via Configuration File

```yaml
# codex.yaml
komga_api:
  enabled: true
  prefix: "komga"  # Optional, this is the default
```

### Via Environment Variables

```bash
CODEX_KOMGA_API_ENABLED=true
CODEX_KOMGA_API_PREFIX=komga
```

## API Endpoints

When enabled, the following Komga-compatible endpoints are available at `/{prefix}/api/v1/`:

### Libraries

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/libraries` | GET | List all libraries |
| `/libraries/{id}` | GET | Get library details |
| `/libraries/{id}/thumbnail` | GET | Get library thumbnail |

### Series

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/series` | GET | List series (paginated) |
| `/series/new` | GET | Recently added series |
| `/series/updated` | GET | Recently updated series |
| `/series/{id}` | GET | Get series details |
| `/series/{id}/thumbnail` | GET | Get series thumbnail |
| `/series/{id}/books` | GET | List books in series |

### Books

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/books/ondeck` | GET | Continue reading (in-progress books) |
| `/books/list` | POST | Search/filter books |
| `/books/{id}` | GET | Get book details |
| `/books/{id}/thumbnail` | GET | Get book thumbnail |
| `/books/{id}/file` | GET | Download book file |
| `/books/{id}/next` | GET | Get next book in series |
| `/books/{id}/previous` | GET | Get previous book in series |

### Pages

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/books/{id}/pages` | GET | List pages in book |
| `/books/{id}/pages/{num}` | GET | Get page image |
| `/books/{id}/pages/{num}/thumbnail` | GET | Get page thumbnail |

### Reading Progress

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/books/{id}/read-progress` | PATCH | Update reading progress |
| `/books/{id}/read-progress` | DELETE | Mark book as unread |
| `/series/{id}/read-progress` | POST | Mark all books in series as read |
| `/series/{id}/read-progress` | DELETE | Mark all books in series as unread |

### Users

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/users/me` | GET | Get current user info |

## Authentication

The Komga API supports the same authentication methods as the native Codex API:

- **Basic Auth** (recommended for mobile apps)
- **Bearer Token** (JWT)
- **API Key**

Most Komga-compatible apps use Basic Auth, which is why it's the recommended method.

## Pagination

The Komga API uses Spring Data-style pagination:

- `page` - Page number (0-indexed)
- `size` - Items per page (default: 20, max: 500)

Example:
```
GET /komga/api/v1/series?page=0&size=50
```

## Testing the Connection

You can test the Komga API using curl:

```bash
# Test authentication
curl -u "username:password" http://localhost:8080/komga/api/v1/users/me

# List libraries
curl -u "username:password" http://localhost:8080/komga/api/v1/libraries

# List series
curl -u "username:password" "http://localhost:8080/komga/api/v1/series?page=0&size=10"

# Get a book's pages
curl -u "username:password" http://localhost:8080/komga/api/v1/books/{book-id}/pages
```

## Troubleshooting

### "Connection refused" or "404 Not Found"

1. Ensure `komga_api.enabled` is set to `true` in your configuration
2. Restart Codex after changing the configuration
3. Verify the URL includes the prefix (default: `/komga`)

### "401 Unauthorized"

1. Check your username and password
2. Ensure Basic Auth is being sent (not just in URL)
3. Try logging into the Codex web interface to verify credentials

### "No libraries/series showing"

1. Check that your Codex library has been scanned
2. Verify the user has access to the libraries
3. Try accessing the API directly with curl to see the response

### Thumbnails not loading

1. Ensure the thumbnail directory is writable
2. Check server logs for any errors
3. Try accessing a thumbnail URL directly in your browser

### Reading progress not syncing

1. Verify the PATCH endpoint is working via curl
2. Check that the book ID matches
3. Ensure the app is sending the correct request format

## Limitations

The Komga-compatible API has some limitations compared to the native Komga server:

### Not Supported

- **Metadata editing** - The API is read-only
- **Collections** - Komga collections are not implemented
- **Read lists** - Komga read lists are not implemented
- **Full search syntax** - Only basic search is supported
- **Oneshot detection** - The `oneshot` field is not included in responses
- **WebPub manifests** - Not implemented

### Differences from Komga

- **User permissions** - Uses Codex's permission model (admin, maintainer, reader)
- **Library restrictions** - Not implemented (all libraries are shared)
- **Age restrictions** - Not implemented
- **Content restrictions** - Not implemented

## Security Considerations

:::caution
The Komga API uses Basic Auth, which transmits credentials in base64 encoding (not encrypted). Always use HTTPS in production to protect credentials.
:::

1. **Use HTTPS** - Always use HTTPS when accessing remotely
2. **Strong passwords** - Use strong, unique passwords
3. **API prefix** - Consider changing the default prefix to make it less discoverable
4. **Disable when not needed** - Keep the API disabled if you don't use third-party apps

## Future Enhancements

The following features may be added in future versions:

- Collections/read lists support
- Metadata editing (if requested)
- More apps compatibility testing
- WebPub manifest support
