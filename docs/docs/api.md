---
sidebar_position: 7
---

# API Documentation

Codex provides a RESTful API for managing your digital library. The API is built with Axum and uses JSON for request/response bodies.

## Base URL

All API endpoints are prefixed with `/api/v1`:

```
http://localhost:8080/api/v1
```

## Authentication

Codex uses JWT (JSON Web Tokens) for authentication.

### Getting an API Token

1. Create an admin user (if not exists):
```bash
codex seed --config codex.yaml
```

2. Login to get a token:
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
  "token": "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...",
  "expires_at": "2024-01-02T12:00:00Z"
}
```

### Using the Token

Include the token in the `Authorization` header:

```bash
curl -H "Authorization: Bearer YOUR_TOKEN_HERE" \
  http://localhost:8080/api/v1/libraries
```

## API Endpoints

### Libraries

#### List Libraries

```http
GET /api/v1/libraries
```

Response:
```json
[
  {
    "id": "uuid",
    "name": "My Library",
    "path": "/path/to/library",
    "scan_strategy": "auto",
    "created_at": "2024-01-01T00:00:00Z",
    "updated_at": "2024-01-01T00:00:00Z"
  }
]
```

#### Get Library

```http
GET /api/v1/libraries/{id}
```

#### Create Library

```http
POST /api/v1/libraries
Content-Type: application/json

{
  "name": "New Library",
  "path": "/path/to/library",
  "scan_strategy": "auto"
}
```

#### Update Library

```http
PUT /api/v1/libraries/{id}
Content-Type: application/json

{
  "name": "Updated Name",
  "scan_strategy": "manual"
}
```

#### Delete Library

```http
DELETE /api/v1/libraries/{id}
```

### Series

#### List Series

```http
GET /api/v1/series?library_id={library_id}&page=1&page_size=20
```

Query Parameters:
- `library_id` (optional): Filter by library
- `page` (optional): Page number (default: 1)
- `page_size` (optional): Items per page (default: 20, max: 100)

#### Get Series

```http
GET /api/v1/series/{id}
```

#### Get Series Books

```http
GET /api/v1/series/{id}/books
```

### Books

#### List Books

```http
GET /api/v1/books?series_id={series_id}&library_id={library_id}&page=1
```

#### Get Book

```http
GET /api/v1/books/{id}
```

#### Get Book Pages

```http
GET /api/v1/books/{id}/pages
```

Response:
```json
[
  {
    "id": "uuid",
    "book_id": "uuid",
    "page_number": 1,
    "file_path": "page001.jpg",
    "width": 1920,
    "height": 2560,
    "file_size": 245760,
    "created_at": "2024-01-01T00:00:00Z"
  }
]
```

### Users

#### List Users

```http
GET /api/v1/users
```

Requires admin permissions.

#### Create User

```http
POST /api/v1/users
Content-Type: application/json

{
  "username": "newuser",
  "password": "secure-password",
  "email": "user@example.com"
}
```

#### Update User

```http
PUT /api/v1/users/{id}
Content-Type: application/json

{
  "email": "updated@example.com",
  "permissions": ["read", "write"]
}
```

### API Keys

#### List API Keys

```http
GET /api/v1/api-keys
```

#### Create API Key

```http
POST /api/v1/api-keys
Content-Type: application/json

{
  "name": "My API Key",
  "permissions": ["read"]
}
```

Response:
```json
{
  "id": "uuid",
  "name": "My API Key",
  "key": "codex_xxxxxxxxxxxxx",
  "created_at": "2024-01-01T00:00:00Z"
}
```

**Important:** The key is only shown once. Store it securely.

#### Revoke API Key

```http
DELETE /api/v1/api-keys/{id}
```

## Error Responses

All errors follow this format:

```json
{
  "error": "Error type",
  "message": "Human-readable error message",
  "details": {}
}
```

### Common Status Codes

- `200 OK`: Request succeeded
- `201 Created`: Resource created
- `400 Bad Request`: Invalid request data
- `401 Unauthorized`: Authentication required
- `403 Forbidden`: Insufficient permissions
- `404 Not Found`: Resource not found
- `500 Internal Server Error`: Server error

### Example Error Response

```json
{
  "error": "ValidationError",
  "message": "Invalid library path",
  "details": {
    "field": "path",
    "reason": "Path does not exist"
  }
}
```

## Pagination

List endpoints support pagination:

```http
GET /api/v1/books?page=2&page_size=50
```

Response includes pagination metadata:

```json
{
  "data": [...],
  "pagination": {
    "page": 2,
    "page_size": 50,
    "total": 150,
    "total_pages": 3
  }
}
```

## Filtering and Sorting

Some endpoints support filtering and sorting (implementation varies by endpoint):

```http
GET /api/v1/books?sort=created_at&order=desc&format=cbz
```

## Rate Limiting

API rate limiting may be configured. Check response headers:

```
X-RateLimit-Limit: 100
X-RateLimit-Remaining: 95
X-RateLimit-Reset: 1640995200
```

## Swagger UI

If enabled in configuration, interactive API documentation is available at:

```
http://localhost:8080/docs
```

Enable it in your config:

```yaml
api:
  enable_swagger: true
  swagger_path: "/docs"
```

## SDK and Client Libraries

### cURL Examples

```bash
# Login
TOKEN=$(curl -X POST http://localhost:8080/api/v1/auth/login \
  -H "Content-Type: application/json" \
  -d '{"username":"admin","password":"pass"}' \
  | jq -r '.token')

# List libraries
curl -H "Authorization: Bearer $TOKEN" \
  http://localhost:8080/api/v1/libraries

# Get a book
curl -H "Authorization: Bearer $TOKEN" \
  http://localhost:8080/api/v1/books/{id}
```

### JavaScript/TypeScript

```typescript
const response = await fetch('http://localhost:8080/api/v1/libraries', {
  headers: {
    'Authorization': `Bearer ${token}`,
    'Content-Type': 'application/json'
  }
});

const libraries = await response.json();
```

## Webhooks

Webhook support is planned for future releases. Subscribe to updates for notifications when:
- New books are scanned
- Libraries are updated
- Series are created

## Next Steps

- Explore the [Swagger UI](./api#swagger-ui) for interactive documentation
- Check [authentication examples](./api#authentication)
- Review [error handling](./api#error-responses)

