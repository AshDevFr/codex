---
---

# API Documentation

Codex provides a comprehensive RESTful API for managing your digital library. This guide covers authentication, endpoints, and how to use the interactive API documentation.

## Base URL

All API endpoints are prefixed with `/api/v1`:

```
http://localhost:8080/api/v1
```

## Interactive API Documentation

Codex includes built-in interactive API documentation powered by [Scalar](https://scalar.com/) (OpenAPI).

### Enabling API Docs

API documentation is disabled by default. Enable it in your configuration:

```yaml
api:
  enable_api_docs: true
  api_docs_path: "/docs"  # Optional, defaults to /docs
```

Or via environment variable:

```bash
CODEX_API_ENABLE_API_DOCS=true
```

### Accessing API Docs

Once enabled, access the interactive documentation at:

```
http://localhost:8080/docs
```

### Using the API Documentation

1. **Explore Endpoints**: Browse all available API endpoints organized by category
2. **Try It Out**: Click on any endpoint to make live requests
3. **Authenticate**: Use the authentication section to enter your JWT token
4. **View Schemas**: Explore request/response schemas and data models

### Exporting OpenAPI Specification

Export the OpenAPI spec for client generation or documentation:

```bash
# JSON format
codex openapi --output openapi.json --format json

# YAML format
codex openapi --output openapi.yaml --format yaml
```

:::tip Production Note
Disable API documentation in production environments for security:
```yaml
api:
  enable_api_docs: false
```
:::

## Authentication

Codex supports multiple authentication methods.

### JWT Bearer Token (Recommended)

The primary authentication method for web clients.

#### 1. Login to Get Token

```bash
curl -X POST http://localhost:8080/api/v1/auth/login \
  -H "Content-Type: application/json" \
  -d '{
    "username": "admin",
    "password": "your-password"
  }'
```

Response:

```json
{
  "access_token": "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...",
  "token_type": "Bearer",
  "expires_in": 86400,
  "user": {
    "id": "550e8400-e29b-41d4-a716-446655440000",
    "username": "admin",
    "email": "admin@example.com",
    "role": "admin"
  }
}
```

#### 2. Use Token in Requests

Include the token in the `Authorization` header:

```bash
curl -H "Authorization: Bearer YOUR_TOKEN_HERE" \
  http://localhost:8080/api/v1/libraries
```

### API Keys

For automation and service-to-service communication.

#### Create an API Key

```bash
curl -X POST http://localhost:8080/api/v1/api-keys \
  -H "Authorization: Bearer YOUR_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "My Script Key",
    "permissions": ["LibrariesRead", "BooksRead", "PagesRead"]
  }'
```

Response:

```json
{
  "id": "550e8400-e29b-41d4-a716-446655440001",
  "name": "My Script Key",
  "key": "codex_abc12345_xyzSecretPart123456",
  "key_prefix": "abc12345",
  "permissions": ["LibrariesRead", "BooksRead", "PagesRead"],
  "created_at": "2024-01-15T10:30:00Z"
}
```

:::warning
The full API key is only shown once! Store it securely.
:::

#### Use API Key

```bash
curl -H "X-API-Key: codex_abc12345_xyzSecretPart123456" \
  http://localhost:8080/api/v1/libraries
```

Or as a Bearer token:

```bash
curl -H "Authorization: Bearer codex_abc12345_xyzSecretPart123456" \
  http://localhost:8080/api/v1/libraries
```

### HTTP Basic Auth

For simple clients and legacy systems:

```bash
curl -u "username:password" http://localhost:8080/api/v1/libraries
```

## API Endpoints

### Setup (First Run)

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/setup/status` | Check if initial setup is needed |
| POST | `/setup/initialize` | Create the initial admin user |

### Authentication

| Method | Endpoint | Description |
|--------|----------|-------------|
| POST | `/auth/login` | User login |
| POST | `/auth/register` | User registration |
| POST | `/auth/logout` | User logout |
| GET | `/auth/me` | Get current user info |
| POST | `/auth/verify-email` | Verify email token |
| POST | `/auth/resend-verification` | Resend verification email |

### Libraries

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/libraries` | List all libraries |
| POST | `/libraries` | Create a library |
| GET | `/libraries/{id}` | Get library details |
| PUT | `/libraries/{id}` | Update a library |
| DELETE | `/libraries/{id}` | Delete a library |
| GET | `/libraries/{id}/thumbnail` | Get library cover image |

#### Create Library Example

```bash
curl -X POST http://localhost:8080/api/v1/libraries \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "My Comics",
    "path": "/library/comics",
    "scanning_config": {
      "enabled": true,
      "cron_schedule": "0 0 * * *",
      "default_mode": "normal",
      "scan_on_start": true
    }
  }'
```

### Scanning

| Method | Endpoint | Description |
|--------|----------|-------------|
| POST | `/libraries/{id}/scan` | Start a library scan |
| GET | `/libraries/{id}/scan-status` | Get current scan status |
| POST | `/libraries/{id}/scan/cancel` | Cancel running scan |
| GET | `/scans/active` | List all active scans |
| GET | `/scans/stream` | SSE: Scan progress events |

#### Trigger Scan Example

```bash
# Normal scan (only new/changed files)
curl -X POST "http://localhost:8080/api/v1/libraries/{id}/scan?mode=normal" \
  -H "Authorization: Bearer $TOKEN"

# Deep scan (re-analyze all files)
curl -X POST "http://localhost:8080/api/v1/libraries/{id}/scan?mode=deep" \
  -H "Authorization: Bearer $TOKEN"
```

### Series

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/series` | List all series |
| POST | `/series/list` | Advanced filtering with conditions |
| POST | `/series/search` | Full-text search |
| GET | `/series/in-progress` | Series with in-progress books |
| GET | `/series/recently-added` | Recently added series |
| GET | `/series/recently-updated` | Recently updated series |
| GET | `/series/{id}` | Get series details |
| GET | `/series/{id}/books` | Get books in series |
| GET | `/series/{id}/thumbnail` | Get series cover |
| POST | `/series/{id}/analyze` | Analyze all books in series |
| POST | `/series/{id}/read` | Mark series as read |
| POST | `/series/{id}/unread` | Mark series as unread |

#### Series Sorting

Use the `sort` parameter with format `field,direction`. Available fields: `name`, `date_added`, `date_updated`, `release_date`, `date_read`, `book_count`, `file_size`, `page_count`.

```bash
curl "http://localhost:8080/api/v1/series?sort=date_added,desc" \
  -H "Authorization: Bearer $TOKEN"
```

### Books

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/books` | List all books |
| POST | `/books/list` | Advanced filtering with conditions |
| GET | `/books/{id}` | Get book details |
| GET | `/books/{id}/thumbnail` | Get book cover |
| POST | `/books/{id}/analyze` | Trigger book analysis |
| GET | `/books/in-progress` | Get books currently being read |
| GET | `/books/on-deck` | Next unread book in started series |
| GET | `/books/recently-added` | Get recently added books |
| GET | `/books/recently-read` | Get recently read books |

#### On Deck

The "On Deck" endpoint returns the next unread book in series where you have completed at least one book but have no books currently in progress.

```bash
curl "http://localhost:8080/api/v1/books/on-deck" \
  -H "Authorization: Bearer $TOKEN"
```

### Pages

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/books/{id}/pages` | List pages in a book |
| GET | `/books/{book_id}/pages/{page_number}` | Get page image |

#### Page Image Parameters

| Parameter | Type | Description |
|-----------|------|-------------|
| `width` | integer | Maximum width for resizing |
| `height` | integer | Maximum height for resizing |
| `format` | string | Output format: `jpeg`, `png`, `webp` |

Example:

```bash
curl -H "Authorization: Bearer $TOKEN" \
  "http://localhost:8080/api/v1/books/{id}/pages/1?width=800&format=webp"
```

### Reading Progress

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/books/{id}/progress` | Get reading progress |
| PUT | `/books/{id}/progress` | Update reading progress |
| DELETE | `/books/{id}/progress` | Clear reading progress |
| GET | `/progress` | Get all user progress |
| GET | `/progress/currently-reading` | Get currently reading books |
| POST | `/books/{id}/read` | Mark book as read |
| POST | `/books/{id}/unread` | Mark book as unread |

#### Update Progress Example

```bash
curl -X PUT http://localhost:8080/api/v1/books/{id}/progress \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "current_page": 15,
    "is_completed": false
  }'
```

### Users (Admin Only)

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/users` | List all users |
| POST | `/users` | Create a user |
| GET | `/users/{id}` | Get user details |
| PUT | `/users/{id}` | Update a user |
| DELETE | `/users/{id}` | Delete a user |

### API Keys

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/api-keys` | List your API keys |
| POST | `/api-keys` | Create an API key |
| GET | `/api-keys/{id}` | Get API key details |
| PUT | `/api-keys/{id}` | Update an API key |
| DELETE | `/api-keys/{id}` | Revoke an API key |

### Tasks

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/tasks` | List background tasks |
| GET | `/tasks/{id}` | Get task details |
| POST | `/tasks/{id}/cancel` | Cancel a running task |
| GET | `/tasks/stats` | Get task statistics |
| GET | `/tasks/stream` | SSE: Task progress events |

### Duplicates

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/duplicates` | List duplicate book groups |
| POST | `/duplicates/scan` | Scan for duplicates |
| DELETE | `/duplicates/{id}` | Dismiss a duplicate group |

### User Preferences

Per-user settings (theme, language, reader options). See [User Management](./users/user-management#user-preferences) for details.

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/user/preferences` | List all preferences |
| PUT | `/user/preferences/{key}` | Set a preference |
| DELETE | `/user/preferences/{key}` | Reset to default |

### Settings (Admin Only)

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/admin/settings` | List all settings |
| GET | `/admin/settings/{key}` | Get a specific setting |
| PUT | `/admin/settings/{key}` | Update a setting |
| POST | `/admin/settings/bulk` | Bulk update settings |
| POST | `/admin/settings/{key}/reset` | Reset to default |
| GET | `/admin/settings/{key}/history` | Get setting change history |

### Filesystem (Admin Only)

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/filesystem/browse` | Browse server directories |
| GET | `/filesystem/drives` | List available drives (Windows) |

### Health & Metrics

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/health` | Health check |
| GET | `/metrics` | Server metrics |

## Real-Time Events (SSE)

Codex uses Server-Sent Events (SSE) for real-time updates.

### Entity Events Stream

```bash
curl -H "Authorization: Bearer $TOKEN" \
  -H "Accept: text/event-stream" \
  http://localhost:8080/api/v1/events/stream
```

Event types:

| Event | Description |
|-------|-------------|
| `book_created` | New book added |
| `book_updated` | Book metadata changed |
| `book_deleted` | Book removed |
| `series_created` | New series discovered |
| `series_updated` | Series metadata changed |
| `series_deleted` | Series removed |
| `library_created` | New library added |
| `library_updated` | Library settings changed |
| `cover_updated` | Thumbnail regenerated |

Event format:

```
event: book_created
data: {"type":"book_created","book_id":"uuid","series_id":"uuid","library_id":"uuid","timestamp":"2024-01-15T10:30:00Z"}
```

### Task Progress Stream

```bash
curl -H "Authorization: Bearer $TOKEN" \
  -H "Accept: text/event-stream" \
  http://localhost:8080/api/v1/tasks/stream
```

Event format:

```
event: task_progress
data: {"task_id":"uuid","task_type":"scan_library","status":"running","progress":{"current":25,"total":100,"message":"Scanning file 25/100"}}
```

### SSE Keep-Alive

SSE connections send a keep-alive comment every 15 seconds:

```
: keep-alive
```

## Pagination

List endpoints support pagination with the following conventions:

### Query Parameters

All endpoints (GET and POST) use query parameters for pagination:

| Parameter | Default | Max | Description |
|-----------|---------|-----|-------------|
| `page` | `1` | - | Page number (1-indexed) |
| `pageSize` | `50` | `500` | Items per page |

#### GET Requests

```bash
curl -H "Authorization: Bearer $TOKEN" \
  "http://localhost:8080/api/v1/books?page=2&pageSize=25"
```

#### POST Requests (Filtering Endpoints)

POST endpoints like `/api/v1/books/list` and `/api/v1/series/list` use:
- **Query parameters** for pagination (`page`, `pageSize`, `sort`)
- **Request body** for filter criteria only

```bash
curl -X POST "http://localhost:8080/api/v1/series/list?page=1&pageSize=25&sort=name,asc" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "condition": { "genre": { "operator": "is", "value": "Action" } },
    "fullTextSearch": "batman"
  }'
```

### Response Format

All paginated responses use **camelCase** and include HATEOAS navigation links:

```json
{
  "data": [...],
  "page": 2,
  "pageSize": 25,
  "total": 150,
  "totalPages": 6,
  "links": {
    "self": "/api/v1/series/list?page=2&pageSize=25",
    "first": "/api/v1/series/list?page=1&pageSize=25",
    "prev": "/api/v1/series/list?page=1&pageSize=25",
    "next": "/api/v1/series/list?page=3&pageSize=25",
    "last": "/api/v1/series/list?page=6&pageSize=25"
  }
}
```

:::note
GET endpoints also support `page_size` (snake_case) for backwards compatibility, but `pageSize` (camelCase) is preferred for consistency.
:::

## Filtering & Sorting

### Common Query Parameters

| Parameter | Description | Example |
|-----------|-------------|---------|
| `library_id` | Filter by library | `library_id=uuid` |
| `series_id` | Filter by series | `series_id=uuid` |
| `sort` | Sort field | `sort=created_at` |
| `order` | Sort direction | `order=desc` |

### Sort Fields

**Books**: `title`, `number`, `file_size`, `page_count`, `created_at`, `modified_at`

**Series**: `name`, `date_added`, `date_updated`, `release_date`, `date_read`, `book_count`, `file_size`, `filename`, `page_count`

Example:

```bash
curl -H "Authorization: Bearer $TOKEN" \
  "http://localhost:8080/api/v1/books?library_id=uuid&sort=title&order=asc"
```

### Advanced Filtering

Use `POST /series/list` or `POST /books/list` for condition-based filtering.

#### Condition Structure

Filters use a tree structure with `allOf` (AND) and `anyOf` (OR) combinators.

**Request URL with pagination:**
```
POST /api/v1/series/list?page=1&pageSize=20&sort=name,asc
```

**Request body with filter criteria:**
```json
{
  "condition": {
    "allOf": [
      { "genre": { "operator": "is", "value": "Action" } },
      { "genre": { "operator": "isNot", "value": "Horror" } }
    ]
  },
  "fullTextSearch": "batman"
}
```

#### Operators

| Operator | Description |
|----------|-------------|
| `is` | Equals value |
| `isNot` | Not equals value |
| `isNull` | Field is null |
| `isNotNull` | Field has value |
| `contains` | Contains substring |
| `beginsWith` | Starts with |

#### Filter Fields

**Series**: `libraryId`, `genre`, `tag`, `status`, `publisher`, `language`, `name`, `readStatus`

**Books**: `libraryId`, `seriesId`, `genre`, `tag`, `title`, `readStatus`, `hasError`

#### Example: Genre filter with exclusion

```bash
curl -X POST "http://localhost:8080/api/v1/series/list?page=1&pageSize=20" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "condition": {
      "allOf": [
        { "genre": { "operator": "is", "value": "Action" } },
        { "genre": { "operator": "isNot", "value": "Horror" } }
      ]
    }
  }'
```

See the [Filtering & Search](./filtering) guide for more examples.

## Error Responses

All errors follow a consistent format:

```json
{
  "error": "NotFound",
  "message": "Book not found",
  "details": null
}
```

### HTTP Status Codes

| Code | Meaning | Description |
|------|---------|-------------|
| 200 | OK | Request successful |
| 201 | Created | Resource created |
| 204 | No Content | Success, no response body |
| 400 | Bad Request | Invalid request data |
| 401 | Unauthorized | Authentication required |
| 403 | Forbidden | Insufficient permissions |
| 404 | Not Found | Resource not found |
| 409 | Conflict | Resource conflict |
| 500 | Internal Error | Server error |

## Permissions

API endpoints require specific permissions:

| Permission | Description |
|------------|-------------|
| `LibrariesRead` | View libraries |
| `LibrariesWrite` | Create/update libraries |
| `LibrariesDelete` | Delete libraries |
| `SeriesRead` | View series |
| `SeriesWrite` | Update series |
| `SeriesDelete` | Delete series |
| `BooksRead` | View books |
| `BooksWrite` | Update books/progress |
| `BooksDelete` | Delete books |
| `PagesRead` | View page images |
| `UsersRead` | View users (admin) |
| `UsersWrite` | Manage users (admin) |
| `UsersDelete` | Delete users (admin) |
| `ApiKeysRead` | View API keys |
| `ApiKeysWrite` | Manage API keys |
| `ApiKeysDelete` | Delete API keys |
| `TasksRead` | View tasks |
| `TasksWrite` | Manage tasks |
| `SystemHealth` | View metrics |
| `SystemAdmin` | Full admin access |

## Code Examples

### JavaScript/TypeScript

```typescript
const API_URL = 'http://localhost:8080/api/v1';

// Login
const loginResponse = await fetch(`${API_URL}/auth/login`, {
  method: 'POST',
  headers: { 'Content-Type': 'application/json' },
  body: JSON.stringify({ username: 'admin', password: 'password' })
});
const { access_token } = await loginResponse.json();

// Get libraries
const libraries = await fetch(`${API_URL}/libraries`, {
  headers: { 'Authorization': `Bearer ${access_token}` }
}).then(r => r.json());

// Subscribe to events
const eventSource = new EventSource(
  `${API_URL}/events/stream`,
  { headers: { 'Authorization': `Bearer ${access_token}` } }
);

eventSource.onmessage = (event) => {
  console.log('Event:', JSON.parse(event.data));
};
```

### Python

```python
import requests

API_URL = 'http://localhost:8080/api/v1'

# Login
response = requests.post(f'{API_URL}/auth/login', json={
    'username': 'admin',
    'password': 'password'
})
token = response.json()['access_token']

headers = {'Authorization': f'Bearer {token}'}

# Get libraries
libraries = requests.get(f'{API_URL}/libraries', headers=headers).json()

# Create a library
new_library = requests.post(f'{API_URL}/libraries', headers=headers, json={
    'name': 'My Library',
    'path': '/media/comics'
}).json()
```

### cURL

```bash
# Login and save token
TOKEN=$(curl -s -X POST http://localhost:8080/api/v1/auth/login \
  -H "Content-Type: application/json" \
  -d '{"username":"admin","password":"password"}' | jq -r '.access_token')

# List libraries
curl -H "Authorization: Bearer $TOKEN" \
  http://localhost:8080/api/v1/libraries

# Create library
curl -X POST http://localhost:8080/api/v1/libraries \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"name":"Comics","path":"/media/comics"}'

# Get book cover
curl -H "Authorization: Bearer $TOKEN" \
  http://localhost:8080/api/v1/books/{id}/thumbnail > cover.jpg
```

## Next Steps

- [Set up libraries](./libraries)
- [Configure OPDS](./opds)
- [Manage users](./users)
