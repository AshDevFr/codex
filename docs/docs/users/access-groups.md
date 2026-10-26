---
sidebar_position: 7
---

# Access Groups

Access groups decide what content their members can see. A group holds **rules**: allow or deny a library, a genre, a tag, or a [sharing tag](./sharing-tags). You assign the group to many users at once instead of configuring each user separately.

:::info Admins and maintainers are never restricted
Access groups and sharing-tag grants do not apply to users with the **admin** or **maintainer** role, even when they are members of a group. Both always see the whole library: maintainers curate metadata, covers and scans, which needs every series. Only **readers** are restricted, so check a restricted setup with a reader account.
:::

## Rule Types

| Rule on | Matches a series when | Notes |
|---------|-----------------------|-------|
| **Library** | the series is in that library | A new library is hidden from whitelisted users until a group allows it |
| **Genre** | the series has that genre | Matched by name, ignoring case |
| **Tag** | the series has that tag | Matched by name, ignoring case |
| **Sharing tag** | the series carries that sharing tag | Sharing tags are applied by hand; see [sharing tags](./sharing-tags) |

Each rule is either **allow** or **deny**. A value can appear once per group.

Genre and tag rules are matched by **name**, not by the genre or tag itself. Codex removes genres and tags that no series uses anymore, and the next series that carries the name creates it again. A rule on "Adult" keeps working through that, and you can create a rule for a genre that no series has yet.

## How Rules Combine

1. **Allow rules inside one group narrow each other.**
   - Values of the **same type** combine as "any of" by default: allowing the genres Kids and Comedy shows series with either genre. For genres, tags and sharing tags you can switch a group to "all of", so only series with **every** allowed value match. Libraries are always "any of", since a series is in exactly one library.
   - **Different types** must all match: "allow library Manga" plus "allow genre Kids" shows Kids-genre series in the Manga library, not all of Manga plus every Kids series.
2. **Groups add up.** Each group grants its own scope, and a member of several groups sees everything any of their groups allows. A user's personal sharing-tag grants count as one more group.
3. **Deny always wins, everywhere.** Every deny rule, from every group the user is in and from their personal grants, is collected into one list and hidden at the end. A deny is not limited to the group it is written in, so no other group can grant that content back.
4. **No allow rules at all** (no groups, or only groups with deny rules): the user sees everything that is not denied.

```
visible = (what group A allows) + (what group B allows) + ...  -  (every deny from every group)
```

:::warning Whitelist mode
As soon as any allow rule applies to a user, they only see what an allow rule matches. Series that match no allow rule are hidden, including series with no genres, tags or sharing tags at all.
:::

### Examples

**Two groups, one deny.** A user is in:

- *Manga*: allow library Manga
- *Comics, no adult*: allow library Comics, deny genre Adult

They see all of Manga and all of Comics, **minus anything in the Adult genre in either library**. The Adult deny from the second group also applies to the manga the first group grants.

**One group, two types.** *Kids manga*: allow library Manga, allow genre Kids. Members see Kids-genre series in Manga only.

**Deny-only group.** *No adult*: deny genre Adult. Members see the whole library except Adult-genre series. Untagged content stays visible, because a deny-only group never starts a whitelist.

### Allow is safer than deny for metadata rules

Genres and tags come from your files' metadata and from metadata plugins. A **deny** on a genre hides only series that actually carry it: a series whose metadata is missing the genre stays visible. An **allow** on a genre does the opposite: a series missing the genre stays hidden. For a kids account, "allow library Kids" or "allow genre Kids" is the safer setup, with deny rules as a second layer.

## The Default Group

You can mark **one** group as the default. Every account created afterwards joins it automatically: accounts created by an admin, by self-registration, and on a user's first OIDC login.

- It is an ordinary membership (shown with the source `default`). You can remove a user from the default group, and they stay out.
- Making a group the default does **not** add existing users. Use **Add all existing users** on the group page to bring them in. Existing memberships are left as they are.
- Marking another group as default moves the default to it.

A default group with an allow rule (for example "allow library Kids") gives you closed-by-default accounts: new users see only that until you add them to other groups.

## Membership Sources

Each membership records how the user got into the group:

| Source | Added by | Removed by |
|--------|----------|------------|
| `manual` | an admin | an admin |
| `default` | account creation, or **Add all existing users** | an admin |
| `oidc` | an [OIDC](./oidc) group mapping on login | the next login after the user leaves the IdP group, or an admin |

Whoever adds a user to a group first owns that membership. Adding the same user again from another source changes nothing: an admin adding someone the IdP already placed does not stop the IdP from removing them later, and an IdP group mapping to a group an admin assigned does not let the IdP remove that assignment.

## Managing Access Groups

### Via Web Interface

1. Go to **Settings** > **Access Groups**
2. Click **Create Group** and enter a name and optional description
3. Open the group to configure it:
   - **Default group for new users** switch and **Add all existing users**
   - **Rules**: add libraries, genres, tags and sharing tags as allow or deny, and choose "any" or "all" per type
   - **Members** and **OIDC mappings**

The users list in **Settings** > **Users** shows each user's groups.

### Via API

**Create a group:**
```bash
curl -X POST http://localhost:8080/api/v1/access-groups \
  -H "Authorization: Bearer $ADMIN_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"name": "Kids", "description": "Kids accounts"}'
```

**Replace a group's rules** (the whole set is replaced; omitted lists are emptied and omitted match fields are `any`):
```bash
curl -X PUT http://localhost:8080/api/v1/access-groups/{id}/rules \
  -H "Authorization: Bearer $ADMIN_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "genreMatch": "any",
    "libraries": [{"libraryId": "kids-library-uuid", "accessMode": "allow"}],
    "genres": [{"name": "Adult", "accessMode": "deny"}],
    "tags": [{"name": "Gore", "accessMode": "deny"}],
    "sharingTags": []
  }'
```

**Read a group's rules:**
```bash
curl http://localhost:8080/api/v1/access-groups/{id}/rules \
  -H "Authorization: Bearer $ADMIN_TOKEN"
```

**Make a group the default, or stop it being the default:**
```bash
curl -X PATCH http://localhost:8080/api/v1/access-groups/{id} \
  -H "Authorization: Bearer $ADMIN_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"isDefault": true}'
```

**Add every user who is not a member yet:**
```bash
curl -X POST http://localhost:8080/api/v1/access-groups/{id}/add-all-users \
  -H "Authorization: Bearer $ADMIN_TOKEN"
```

**Add or remove members:**
```bash
curl -X POST http://localhost:8080/api/v1/access-groups/{id}/members \
  -H "Authorization: Bearer $ADMIN_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"userIds": ["user-uuid-1", "user-uuid-2"]}'

curl -X DELETE http://localhost:8080/api/v1/access-groups/{id}/members/{user_id} \
  -H "Authorization: Bearer $ADMIN_TOKEN"
```

**List groups, get one, update, delete:**
```bash
curl http://localhost:8080/api/v1/access-groups -H "Authorization: Bearer $ADMIN_TOKEN"
curl http://localhost:8080/api/v1/access-groups/{id} -H "Authorization: Bearer $ADMIN_TOKEN"
curl -X PATCH http://localhost:8080/api/v1/access-groups/{id} \
  -H "Authorization: Bearer $ADMIN_TOKEN" -H "Content-Type: application/json" \
  -d '{"description": "Updated description"}'
curl -X DELETE http://localhost:8080/api/v1/access-groups/{id} -H "Authorization: Bearer $ADMIN_TOKEN"
```

The older `POST /access-groups/{id}/grants` and `DELETE /access-groups/{id}/grants/{tag_id}` endpoints still add and remove a single sharing-tag rule.

:::warning
Deleting a group removes its memberships and rules. Members lose what the group granted on their next request.
:::

## OIDC Auto-Assignment

If you use [OIDC authentication](./oidc), you can map IdP group names to access groups. On every OIDC login, the user's OIDC-sourced memberships are reconciled with their IdP groups:

- A mapped IdP group the user is in adds them to the access group (source `oidc`), unless they are already a member.
- An `oidc` membership whose IdP group is no longer in the user's claims is removed.
- `manual` and `default` memberships are never touched by the sync.
- If the sync fails, the login fails. Codex does not let a user in with a partial or missing set of groups.

**Add or remove an OIDC mapping:**
```bash
curl -X POST http://localhost:8080/api/v1/access-groups/{id}/oidc-mappings \
  -H "Authorization: Bearer $ADMIN_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"oidcGroupName": "library-staff"}'

curl -X DELETE http://localhost:8080/api/v1/access-groups/{id}/oidc-mappings/{mapping_id} \
  -H "Authorization: Bearer $ADMIN_TOKEN"
```

:::note Case Sensitivity
OIDC group names are matched exactly as stored. If your IdP sends `Library-Staff` but the mapping says `library-staff`, it will not match.
:::

## Checking What a User Sees

Open **Settings** > **Users**, edit a user, and look at **What this user sees**:

- how many series they can and cannot see, and which libraries are visible to them;
- **Check a series**: pick any series to see whether the user can see it and why: the deny rule that hides it and the group it comes from, the group whose allow rules let it through, or that none of their allow rules matches it.

It uses the same rules Codex enforces, so it cannot disagree with what the user actually gets. Admins and maintainers are reported as never restricted.

**Via API:**
```bash
curl "http://localhost:8080/api/v1/users/{user_id}/visibility-preview?seriesId={series_id}" \
  -H "Authorization: Bearer $ADMIN_TOKEN"
```

`GET /api/v1/users/{user_id}/effective-grants` still lists a user's sharing-tag rules and where each comes from.

## Where Restrictions Apply

A restricted reader gets the same view everywhere: the web app, the native API, OPDS 1.2 and 2.0 feeds, the [Komga-compatible API](../third-party-apps), KOReader sync, and live updates (new or changed series and books are not announced for content they cannot see). Hidden content answers exactly like content that does not exist: it is missing from lists, search, genre and tag lists and library counts, and opening it by id returns "not found".

Server-wide statistics (**Settings** > **Metrics**) are admin-only, since they describe the whole server.

Restriction changes take effect on the next request. When Codex runs as several server processes, the others pick them up within 30 seconds. New series from a scan are covered by deny rules within 30 seconds of the scan, and by allow rules immediately (a new series is hidden until a rule allows it).

## Example: A Kids Account

1. Create a group **Kids** with these rules:
   - allow library *Kids*
   - deny genre *Adult*
2. Mark **Kids** as the default group, so every new account starts there.
3. For adult family members, remove them from **Kids** and add them to a group such as **Everyone** with no allow rules (or no group at all, which sees everything).
4. Check a kid's account under **What this user sees** before handing it over.

A kid's account sees the Kids library and nothing else. If a kid is later added to another group by mistake, the *Adult* deny still applies to everything that group grants.

## Best Practices

1. **Prefer allow rules for restricted accounts**, and use deny rules as a safety net: metadata is never perfect, and allow fails closed.
2. **Put hard limits in deny rules**: they hold whatever other groups a user joins.
3. **Use a default group** so new accounts never start unrestricted by accident.
4. **Name groups by intent** ("Kids", "Staff", "Manga Readers").
5. **Map OIDC groups** when your IdP already has organizational groups.
