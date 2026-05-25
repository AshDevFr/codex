---
sidebar_position: 7
---

# Access Groups

Access groups let you bundle a set of [sharing-tag](./sharing-tags) grants and assign them to multiple users at once. Instead of configuring 50 identical per-user grants, create one group, define its rules, and add users to it.

## Overview

Access groups sit between sharing tags and users:

```
Sharing Tags       Access Groups         Users
 +---------+      +---------------+     +-------+
 | manga   |----->| Manga Readers |---->| alice |
 | 18+     |----->|   allow manga |---->| bob   |
 +---------+      |   deny 18+    |     +-------+
                  +---------------+
                                        Per-user overrides
                                        still apply on top
```

- A group has a name, description, and a set of tag grants (allow/deny).
- Users can belong to multiple groups.
- Per-user grants in the existing `Sharing Tag Grants` panel continue to work as overrides on top of group rules.
- Deny always wins across all sources (groups + per-user).

## How Grants Merge

When a user belongs to one or more groups, their **effective grants** are the union of:

1. All grants from all their groups
2. Their per-user grants (from Settings > Users > Edit > Sharing Tag Grants)

The same deny-wins rule from [sharing tags](./sharing-tags) applies to the merged set:

| Scenario | Result |
|----------|--------|
| Group allows "manga", no user override | User sees manga-tagged content (whitelist mode) |
| Group allows "manga", user denies "18+" | User sees manga-tagged content, but not 18+-tagged content |
| Two groups: one allows "manga", other denies "manga" | Deny wins; manga content hidden |
| Group allows "manga", user also allows "manga" | Same as single allow (deduplicated) |
| No groups, no user grants | User sees all content (default open) |

:::warning Whitelist Mode
Any `allow` grant from any source (group or per-user) activates whitelist mode for that user. In whitelist mode, **untagged content is hidden**. This is the same behavior as per-user allow grants. See the [sharing tags docs](./sharing-tags#the-access-model) for details.
:::

## Managing Access Groups

### Via Web Interface

1. Go to **Settings** > **Access Groups**
2. Click **Create Group**
3. Enter a name and optional description
4. After creation, click the group name to open its detail page
5. Add **tag grants**, **members**, and optionally **OIDC mappings**

### Via API

**Create a group:**
```bash
curl -X POST http://localhost:8080/api/v1/access-groups \
  -H "Authorization: Bearer $ADMIN_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "Manga Readers",
    "description": "Access to all manga content"
  }'
```

**List groups:**
```bash
curl http://localhost:8080/api/v1/access-groups \
  -H "Authorization: Bearer $ADMIN_TOKEN"
```

**Get group details (members, grants, OIDC mappings):**
```bash
curl http://localhost:8080/api/v1/access-groups/{id} \
  -H "Authorization: Bearer $ADMIN_TOKEN"
```

**Update a group:**
```bash
curl -X PATCH http://localhost:8080/api/v1/access-groups/{id} \
  -H "Authorization: Bearer $ADMIN_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"description": "Updated description"}'
```

**Delete a group:**
```bash
curl -X DELETE http://localhost:8080/api/v1/access-groups/{id} \
  -H "Authorization: Bearer $ADMIN_TOKEN"
```

:::warning
Deleting a group removes all its memberships and grants. Users will lose the group's grants on their next request.
:::

## Managing Group Grants

Grants define what content the group's members can see.

**Add a grant:**
```bash
curl -X POST http://localhost:8080/api/v1/access-groups/{id}/grants \
  -H "Authorization: Bearer $ADMIN_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "sharingTagId": "tag-uuid",
    "accessMode": "allow"
  }'
```

**Remove a grant:**
```bash
curl -X DELETE http://localhost:8080/api/v1/access-groups/{id}/grants/{tag_id} \
  -H "Authorization: Bearer $ADMIN_TOKEN"
```

## Managing Group Members

**Add members:**
```bash
curl -X POST http://localhost:8080/api/v1/access-groups/{id}/members \
  -H "Authorization: Bearer $ADMIN_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "userIds": ["user-uuid-1", "user-uuid-2"]
  }'
```

**Remove a member:**
```bash
curl -X DELETE http://localhost:8080/api/v1/access-groups/{id}/members/{user_id} \
  -H "Authorization: Bearer $ADMIN_TOKEN"
```

**List a user's groups:**
```bash
curl http://localhost:8080/api/v1/users/{user_id}/access-groups \
  -H "Authorization: Bearer $ADMIN_TOKEN"
```

## OIDC Auto-Assignment

If you use [OIDC authentication](./oidc), you can map IdP group names to access groups. When a user logs in via OIDC, their access group memberships are automatically reconciled:

- If the user's IdP groups include a mapped group name, they are added to the corresponding access group (with `source=oidc`).
- If a previously mapped group is no longer in their IdP claims, the OIDC-sourced membership is removed.
- Manually assigned memberships are never touched by the OIDC sync.

**Add an OIDC mapping:**
```bash
curl -X POST http://localhost:8080/api/v1/access-groups/{id}/oidc-mappings \
  -H "Authorization: Bearer $ADMIN_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "oidcGroupName": "library-staff"
  }'
```

**Remove an OIDC mapping:**
```bash
curl -X DELETE http://localhost:8080/api/v1/access-groups/{id}/oidc-mappings/{mapping_id} \
  -H "Authorization: Bearer $ADMIN_TOKEN"
```

:::note Case Sensitivity
OIDC group names are matched exactly as stored. If your IdP sends `Library-Staff` but the mapping says `library-staff`, it will not match. Configure the mapping to match your IdP's exact casing.
:::

## Debugging Effective Grants

The **effective grants** endpoint shows exactly which grants apply to a user and where each one comes from (per-user override or group name). This is the primary tool for answering "why can't this user see series X?"

**Via Web Interface:**
1. Go to **Settings** > **Users**
2. Edit a user
3. Scroll to the **Effective Grants** panel

**Via API:**
```bash
curl http://localhost:8080/api/v1/users/{user_id}/effective-grants \
  -H "Authorization: Bearer $ADMIN_TOKEN"
```

Response:
```json
{
  "userId": "user-uuid",
  "grants": [
    {
      "sharingTagId": "tag-uuid",
      "sharingTagName": "manga",
      "accessMode": "allow",
      "sources": [
        {
          "kind": "group",
          "groupId": "group-uuid",
          "groupName": "Manga Readers"
        }
      ]
    },
    {
      "sharingTagId": "tag-uuid-2",
      "sharingTagName": "18+",
      "accessMode": "deny",
      "sources": [
        {
          "kind": "user",
          "groupId": null,
          "groupName": null
        }
      ]
    }
  ]
}
```

Each grant shows `sources` with `kind: "user"` for per-user overrides or `kind: "group"` with the group name for group-inherited grants. A single tag+mode combination can have multiple sources if both a user override and a group grant contribute the same rule.

## Example: Family + Organization Setup

Combine access groups with per-user overrides for a multi-layered access model:

**Step 1: Create tags and groups**
```bash
# Tags
curl -X POST .../admin/sharing-tags -d '{"name": "manga"}'
curl -X POST .../admin/sharing-tags -d '{"name": "comics"}'
curl -X POST .../admin/sharing-tags -d '{"name": "18+"}'

# Groups
curl -X POST .../access-groups -d '{"name": "Manga Readers"}'
curl -X POST .../access-groups -d '{"name": "Comics Readers"}'
```

**Step 2: Configure group grants**
```bash
# Manga Readers: allow manga
curl -X POST .../access-groups/{manga-group}/grants \
  -d '{"sharingTagId": "manga-tag-id", "accessMode": "allow"}'

# Comics Readers: allow comics
curl -X POST .../access-groups/{comics-group}/grants \
  -d '{"sharingTagId": "comics-tag-id", "accessMode": "allow"}'
```

**Step 3: Assign users**
```bash
# Alice gets manga access via group
curl -X POST .../access-groups/{manga-group}/members \
  -d '{"userIds": ["alice-id"]}'

# But deny 18+ content specifically for Alice (per-user override)
curl -X PUT .../users/alice-id/sharing-tags \
  -d '{"sharingTagId": "18+-tag-id", "accessMode": "deny"}'
```

Result: Alice sees manga-tagged content, but never 18+-tagged content (deny wins).

## Best Practices

1. **Use groups for organizational roles** rather than duplicating per-user grants
2. **Keep per-user grants for exceptions** (e.g., one user in a "Manga Readers" group who should not see 18+ content)
3. **Name groups by intent** (e.g., "Kids", "Staff", "Manga Readers") rather than by technical configuration
4. **Use the effective grants endpoint** to verify access before and after changes
5. **Map OIDC groups** when your IdP already has organizational groups, so membership stays in sync automatically
