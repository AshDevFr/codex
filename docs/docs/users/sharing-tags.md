---
sidebar_position: 6
---

# Sharing Tags

Sharing tags provide content-level access control, allowing admins to restrict which series specific users can see. This is useful for family sharing scenarios where you want to limit access to age-appropriate content.

## Overview

Sharing tags work independently from [permissions](./permissions). While permissions control what actions users can perform (read, write, delete), sharing tags control what content users can see.

| Feature | Permissions | Sharing Tags |
|---------|-------------|--------------|
| Scope | Actions (read/write/delete) | Content visibility |
| Applied to | Users and API keys | Users only |
| Granularity | Resource type | Per-series |
| Who manages | Admins | Admins |

## How It Works

### The Access Model

Sharing tags use a hybrid **allow/deny** model with specific precedence rules:

1. **Deny always wins** - If a user has a `deny` grant for any tag on a series, the series is hidden
2. **Allow creates whitelist mode** - If a user has any `allow` grants, they enter whitelist mode
3. **No grants = full access** - Users without any grants see all content

### Access Modes

| User's Grants | Behavior |
|---------------|----------|
| No grants | Sees all content (default open) |
| Only deny grants | Sees all content EXCEPT series with denied tags |
| Any allow grants | Whitelist mode - ONLY sees series with allowed tags |
| Mixed allow + deny | Whitelist mode, but deny still overrides allow |

### Examples

**Scenario 1: Child Account (Allow grants only)**
- User has `allow` grant for "Kids" tag
- Result: User only sees series tagged "Kids"
- Untagged series are hidden (whitelist mode)

**Scenario 2: Parent Account (Deny grant)**
- User has `deny` grant for "Explicit" tag
- Result: User sees everything except series tagged "Explicit"
- Untagged series are visible

**Scenario 3: Mixed Grants**
- User has `allow` for "Teen" and `deny` for "Mature"
- Series tagged both "Teen" and "Mature" → Hidden (deny wins)
- Series tagged only "Teen" → Visible
- Untagged series → Hidden (whitelist mode active)

## Managing Sharing Tags

### Create Tags (Admin Only)

Tags are created at the system level before they can be assigned.

**Via Web Interface:**
1. Go to **Settings** > **Sharing Tags**
2. Click **Create Tag**
3. Enter name and optional description
4. Save

**Via API:**
```bash
curl -X POST http://localhost:8080/api/v1/admin/sharing-tags \
  -H "Authorization: Bearer $ADMIN_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "Kids",
    "description": "Content appropriate for children"
  }'
```

### List Tags

```bash
curl http://localhost:8080/api/v1/admin/sharing-tags \
  -H "Authorization: Bearer $ADMIN_TOKEN"
```

Response:
```json
[
  {
    "id": "uuid",
    "name": "Kids",
    "description": "Content appropriate for children",
    "created_at": "2024-01-15T10:00:00Z"
  },
  {
    "id": "uuid",
    "name": "Mature",
    "description": "Adult content",
    "created_at": "2024-01-15T10:00:00Z"
  }
]
```

### Update Tag

```bash
curl -X PATCH http://localhost:8080/api/v1/admin/sharing-tags/{id} \
  -H "Authorization: Bearer $ADMIN_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "description": "Updated description"
  }'
```

### Delete Tag

```bash
curl -X DELETE http://localhost:8080/api/v1/admin/sharing-tags/{id} \
  -H "Authorization: Bearer $ADMIN_TOKEN"
```

:::warning
Deleting a tag removes all series associations and user grants for that tag.
:::

## Assigning Tags to Series

### Via Web Interface

1. Go to a series detail page
2. Find the **Sharing Tags** section (admin only)
3. Add or remove tags using the selector

### Via API

**Get series tags:**
```bash
curl http://localhost:8080/api/v1/series/{id}/sharing-tags \
  -H "Authorization: Bearer $TOKEN"
```

**Set series tags (replaces all):**
```bash
curl -X PUT http://localhost:8080/api/v1/series/{id}/sharing-tags \
  -H "Authorization: Bearer $ADMIN_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "sharing_tag_ids": ["tag-uuid-1", "tag-uuid-2"]
  }'
```

**Add a tag:**
```bash
curl -X POST http://localhost:8080/api/v1/series/{id}/sharing-tags \
  -H "Authorization: Bearer $ADMIN_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "sharing_tag_id": "tag-uuid"
  }'
```

**Remove a tag:**
```bash
curl -X DELETE http://localhost:8080/api/v1/series/{id}/sharing-tags/{tag_id} \
  -H "Authorization: Bearer $ADMIN_TOKEN"
```

## Managing User Grants

### Via Web Interface

1. Go to **Settings** > **Users**
2. Edit a user
3. Find the **Sharing Tag Grants** section
4. Add grants with allow or deny access mode

### Via API

**Get user grants:**
```bash
curl http://localhost:8080/api/v1/users/{id}/sharing-tags \
  -H "Authorization: Bearer $ADMIN_TOKEN"
```

Response:
```json
[
  {
    "sharing_tag": {
      "id": "uuid",
      "name": "Kids"
    },
    "access_mode": "allow"
  },
  {
    "sharing_tag": {
      "id": "uuid",
      "name": "Mature"
    },
    "access_mode": "deny"
  }
]
```

**Set user grants (replaces all):**
```bash
curl -X PUT http://localhost:8080/api/v1/users/{id}/sharing-tags \
  -H "Authorization: Bearer $ADMIN_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "grants": [
      {"sharing_tag_id": "kids-uuid", "access_mode": "allow"},
      {"sharing_tag_id": "mature-uuid", "access_mode": "deny"}
    ]
  }'
```

**Remove a grant:**
```bash
curl -X DELETE http://localhost:8080/api/v1/users/{id}/sharing-tags/{tag_id} \
  -H "Authorization: Bearer $ADMIN_TOKEN"
```

### Current User's Grants

Users can view their own grants:

```bash
curl http://localhost:8080/api/v1/user/sharing-tags \
  -H "Authorization: Bearer $TOKEN"
```

## Common Patterns

### Family Sharing Setup

**Step 1: Create age-appropriate tags**
```bash
# Kids content
curl -X POST http://localhost:8080/api/v1/admin/sharing-tags \
  -d '{"name": "Kids", "description": "All ages"}'

# Teen content
curl -X POST http://localhost:8080/api/v1/admin/sharing-tags \
  -d '{"name": "Teen", "description": "Ages 13+"}'

# Adult content
curl -X POST http://localhost:8080/api/v1/admin/sharing-tags \
  -d '{"name": "Mature", "description": "Ages 18+"}'
```

**Step 2: Tag your series**
- Tag children's comics with "Kids"
- Tag shonen/YA with "Teen"
- Tag adult content with "Mature"

**Step 3: Configure user access**

*Child account (whitelist - only kids content):*
```json
{
  "grants": [
    {"sharing_tag_id": "kids-uuid", "access_mode": "allow"}
  ]
}
```

*Teen account (whitelist - kids + teen):*
```json
{
  "grants": [
    {"sharing_tag_id": "kids-uuid", "access_mode": "allow"},
    {"sharing_tag_id": "teen-uuid", "access_mode": "allow"}
  ]
}
```

*Adult account (blacklist - everything except explicit):*
```json
{
  "grants": [
    {"sharing_tag_id": "explicit-uuid", "access_mode": "deny"}
  ]
}
```

### Content Categories

Use tags to organize content by genre or collection:

```bash
# Create category tags
curl -X POST http://localhost:8080/api/v1/admin/sharing-tags \
  -d '{"name": "Manga", "description": "Japanese comics"}'

curl -X POST http://localhost:8080/api/v1/admin/sharing-tags \
  -d '{"name": "Western", "description": "American/European comics"}'
```

Then grant users access to their preferred categories.

## Content Filtering Behavior

### What Gets Filtered

Sharing tags filter content at these endpoints:

- **Series listings** - `GET /api/v1/series`, including search and filtered views
- **Series details** - `GET /api/v1/series/{id}` returns 404 for hidden series
- **Book listings** - `GET /api/v1/books` filters by parent series
- **Book details** - `GET /api/v1/books/{id}` returns 404 for books in hidden series
- **Book files** - `GET /api/v1/books/{id}/file` returns 404 for hidden content
- **Home page sections** - Recently added, keep reading, etc.

### What's Not Filtered

- **Libraries** - All users see all libraries (use permissions to restrict)
- **Pages** - If you can access a book, you can read all pages
- **User data** - Reading progress, preferences, etc.

### Books Inherit from Series

Books don't have their own sharing tags - they inherit from their parent series. If a series is hidden, all its books are hidden.

## Best Practices

1. **Start with deny grants** - For most users, deny specific content rather than whitelist
2. **Use whitelist for children** - Only allow specific tags for maximum restriction
3. **Tag consistently** - Develop a tagging scheme and apply it uniformly
4. **Document your tags** - Use descriptions to clarify tag purposes
5. **Review regularly** - Audit user grants and series tags periodically
6. **Test access** - Verify restricted users can't see hidden content

## Troubleshooting

### User Can't See Content

1. Check if user has any `allow` grants (triggers whitelist mode)
2. Verify series has appropriate tags if whitelist mode is active
3. Check for `deny` grants that might be hiding content

### User Sees Restricted Content

1. Verify the series has the appropriate tag assigned
2. Check user has a `deny` grant for that tag
3. Ensure no conflicting `allow` grants

### Tags Not Appearing

1. Confirm you're logged in as admin
2. Check the tag was created successfully
3. Refresh the page/clear cache
