---
---

# OPDS Catalog

Codex supports the Open Publication Distribution System (OPDS), allowing you to browse and download your library from compatible e-reader apps and devices.

## What is OPDS?

OPDS (Open Publication Distribution System) is a standard protocol for distributing ebooks and other digital content. It uses Atom XML feeds to create browsable catalogs that work with many e-reader applications.

### Benefits

- **Universal compatibility**: Works with dozens of apps
- **Direct downloads**: Download books directly to your device
- **Browse anywhere**: Access your library from any device
- **No special app needed**: Use your favorite reading app

## OPDS Endpoints

### Catalog Root

```
http://localhost:8080/opds
```

The root catalog provides navigation links to browse your library.

### Available Feeds

| Endpoint | Description |
|----------|-------------|
| `/opds` | Root catalog with navigation |
| `/opds/all` | All books (paginated) |
| `/opds/recent` | Recently added books |
| `/opds/series` | Browse by series |
| `/opds/series/{id}` | Books in a specific series |
| `/opds/libraries` | Browse by library |
| `/opds/libraries/{id}` | Books in a specific library |
| `/opds/search?q={query}` | Search books |

## Authentication

OPDS endpoints require authentication. Use one of these methods:

### HTTP Basic Auth

Most OPDS clients support HTTP Basic authentication:

```
Username: your-username
Password: your-password

URL: http://localhost:8080/opds
```

### API Key

Use an API key as the password with username `api`:

```
Username: api
Password: your-api-key

URL: http://localhost:8080/opds
```

### Creating an API Key for OPDS

Create a dedicated API key with minimal permissions:

```bash
curl -X POST http://localhost:8080/api/v1/api-keys \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "OPDS Reader",
    "permissions": ["LibrariesRead", "SeriesRead", "BooksRead", "PagesRead"]
  }'
```

## Compatible Apps

### iOS

| App | Notes |
|-----|-------|
| **Panels** | Excellent comic reader, full OPDS support |
| **Chunky** | Comic reader with OPDS streaming |
| **KyBook 3** | Ebook reader with OPDS support |
| **Marvin** | Feature-rich ebook reader |

### Android

| App | Notes |
|-----|-------|
| **Moon+ Reader** | Popular reader, OPDS catalog support |
| **FBReader** | Open source, good OPDS support |
| **Librera** | Feature-rich, OPDS browsing |
| **Tachiyomi** | Manga reader with extensions |

### Desktop

| App | Notes |
|-----|-------|
| **Calibre** | Full OPDS support via plugin |
| **Thorium Reader** | Modern EPUB reader with OPDS |

### E-Readers

| Device | Notes |
|--------|-------|
| **Kobo** | Built-in OPDS support (via Pocket) |
| **PocketBook** | Native OPDS browser |

## Setting Up OPDS Clients

### General Setup

1. Open your reading app's library/catalog settings
2. Add a new OPDS catalog
3. Enter the URL: `http://your-server:8080/opds`
4. Enter your username and password
5. Save and browse your library

### Panels (iOS) Example

1. Open Panels
2. Go to Library > Sources
3. Tap "Add Source"
4. Select "OPDS"
5. Enter:
   - Name: My Codex
   - URL: `http://your-server:8080/opds`
   - Username: your-username
   - Password: your-password
6. Tap Save

### Moon+ Reader (Android) Example

1. Open Moon+ Reader
2. Tap the library icon
3. Select "Net Library"
4. Tap "+" to add new
5. Select "OPDS Catalog"
6. Enter:
   - Name: Codex
   - URL: `http://your-server:8080/opds`
   - Enable authentication
   - Enter username and password
7. Tap OK

### Calibre Example

1. Install the "OPDS Client" plugin
2. Go to Preferences > Plugins > Get new plugins
3. Search for "OPDS Client" and install
4. Restart Calibre
5. Click the OPDS icon in toolbar
6. Add new server:
   - URL: `http://your-server:8080/opds`
   - Username/password

## OPDS Features

### Browse by Series

Navigate to `/opds/series` to see all series, then drill down into individual series to see their books.

### Search

Search your entire library:

```
http://localhost:8080/opds/search?q=batman
```

Most OPDS clients have a built-in search feature that uses this endpoint.

### Download Formats

When viewing a book, you'll see acquisition links for downloading:

- Original file format (CBZ, CBR, EPUB, PDF)
- Cover image thumbnail

### Pagination

Large catalogs are paginated. OPDS clients handle pagination automatically through `next` and `previous` links in the feed.

## Network Configuration

### Local Network Access

For devices on your local network:

```
http://192.168.1.100:8080/opds
```

Replace with your server's IP address.

### Remote Access

For access outside your network, you'll need:

1. **Port forwarding**: Forward port 8080 (or your custom port)
2. **Dynamic DNS**: If you don't have a static IP
3. **HTTPS**: Strongly recommended for security

Example with reverse proxy:

```
https://codex.yourdomain.com/opds
```

### HTTPS Requirement

Some apps require HTTPS for security. Use a reverse proxy with SSL:

```nginx
server {
    listen 443 ssl;
    server_name codex.yourdomain.com;

    ssl_certificate /etc/letsencrypt/live/codex.yourdomain.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/codex.yourdomain.com/privkey.pem;

    location /opds {
        proxy_pass http://localhost:8080;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
    }
}
```

## Troubleshooting

### Connection Failed

1. **Check URL**: Ensure the URL is correct and includes `/opds`
2. **Check credentials**: Verify username/password
3. **Check network**: Ensure device can reach the server
4. **Check firewall**: Ensure port is open

### Authentication Failed

1. **Check username/password**: Case-sensitive
2. **Try API key**: Use `api` as username, API key as password
3. **Check user permissions**: Ensure user has read permissions

### No Books Showing

1. **Scan library**: Ensure library has been scanned
2. **Check permissions**: User needs `BooksRead` permission
3. **Check filters**: Some apps filter by format

### Downloads Failing

1. **Check file permissions**: Codex needs read access to library
2. **Check file size**: Very large files may timeout
3. **Check network**: Unstable connections cause failures

### Slow Browsing

1. **Enable caching**: OPDS responses are cached
2. **Check network**: Slow network = slow browsing
3. **Reduce library size**: Very large libraries take longer

## OPDS Versions

Codex supports multiple OPDS versions:

- **OPDS 1.2**: Standard catalog format (Atom XML-based)
- **OPDS 2.0**: Modern JSON-based catalog format
- **OPDS PSE 1.0**: Page Streaming Extension for comics

Most clients work with OPDS 1.2. PSE enables page-by-page streaming in supported apps.

### OPDS 2.0

OPDS 2.0 is the next-generation standard using JSON instead of XML. It provides better tooling support and richer metadata via schema.org.

#### OPDS 2.0 Endpoints

| Endpoint | Description |
|----------|-------------|
| `/opds/v2` | Root catalog (JSON) |
| `/opds/v2/libraries` | List all libraries |
| `/opds/v2/libraries/{id}` | Series in a library |
| `/opds/v2/series/{id}` | Books in a series (publications feed) |
| `/opds/v2/recent` | Recent additions |
| `/opds/v2/search?query=...` | Search books and series |

#### Content Type

OPDS 2.0 feeds use `application/opds+json` content type.

#### Example Response

```json
{
  "metadata": {
    "title": "Codex OPDS 2.0 Catalog",
    "modified": "2026-01-10T12:00:00Z"
  },
  "links": [
    {"rel": "self", "href": "/opds/v2", "type": "application/opds+json"},
    {"rel": "search", "href": "/opds/v2/search{?query}", "type": "application/opds+json", "templated": true}
  ],
  "navigation": [
    {"href": "/opds/v2/libraries", "title": "All Libraries", "type": "application/opds+json"}
  ]
}
```

#### Reading Progress in OPDS 2.0

OPDS 2.0 feeds include reading progress for each book (user-specific). Progress is included as a `readingProgress` object on each publication.

#### Client Support

Modern OPDS clients are increasingly supporting OPDS 2.0. If your client doesn't support 2.0, use the standard OPDS 1.2 endpoint at `/opds`.

## Security Considerations

### Use HTTPS

Always use HTTPS for remote access to protect credentials.

### Dedicated API Keys

Create API keys with minimal permissions for OPDS access:

```json
{
  "name": "OPDS Reader",
  "permissions": ["LibrariesRead", "SeriesRead", "BooksRead", "PagesRead"]
}
```

### Network Segmentation

Consider keeping your media server on a separate VLAN if exposing to the internet.

## Best Practices

### Organize Your Library

Well-organized libraries make OPDS browsing easier:

```
/library/
├── Comics/
│   └── [Series Name]/
│       └── files...
└── Manga/
    └── [Series Name]/
        └── files...
```

### Use Consistent Metadata

Good metadata improves the OPDS browsing experience:

- Include ComicInfo.xml in comics
- Ensure EPUBs have complete metadata
- Use consistent series naming

### Test Your Setup

Before configuring mobile apps:

1. Test in a browser: `http://localhost:8080/opds`
2. Verify authentication works
3. Check books are visible
4. Test downloads

## Next Steps

- [Configure libraries](./libraries)
- [Manage users](./users)
- [API documentation](./api)
