---
---

# Want to Read, Collections & Read Lists

Codex gives you three ways to group and queue what you read. They look similar but answer different questions:

| Feature | Holds | Scope | Ordered? | Answers |
|---------|-------|-------|----------|---------|
| **Want to Read** | series **and** books | per-user (private) | by date added | "I want to get to this later" |
| **Collections** | series | shared (library-wide) | optional | "these series belong together" |
| **Read Lists** | books (across series) | shared (library-wide) | yes (default) | "read these issues in this order" |

## Want to Read

A personal on-deck queue. Flag any series or book you intend to read, and it shows up on the **Want to Read** page in the sidebar. The queue is private to you: other users never see it.

- **Add / remove** with the bookmark button on a series or book detail page. Filled = in your queue.
- **Sort** the queue newest-first or oldest-first from the page header.

There's no manual reordering, no shared state, and no permission to configure: it's yours, and every signed-in user has one.

## Collections

A **collection** is a shared, named grouping of **series** (a "Batman" collection, a publisher line, a themed shelf). Everyone on the server sees the same collections, and a series can belong to many of them.

- **Browse** all collections from **Collections** in the sidebar; open one to see its member series.
- **Create / edit / delete** requires the `collections:write` / `collections:delete` permission (see below). Use **New Collection** on the Collections page.
- **Add a series** from its detail page via the "Add to collection" menu, which can also create a new collection inline.
- **Order**: a collection can be *ordered* (members kept in a manual order, reorder with the up/down controls on the detail page) or unordered (sorted by title).

## Read Lists

A **read list** is a shared, ordered grouping of **books** that can span multiple series. This is the comics-native concept: a crossover event like *Civil War* pulls specific issues from many series into one reading order.

- **Browse** all read lists from **Read Lists** in the sidebar; open one to see its books in order.
- **Create / edit / delete** requires the `readlists:write` / `readlists:delete` permission. A read list also has an optional **summary**.
- **Add a book** from its detail page via the "Add to read list" menu (create-new inline supported).
- **Order**: read lists default to *ordered* (manual reading order); reorder with the up/down controls. Turn ordering off to sort members by release date instead.

## Who can manage them

Browsing is available to everyone (it's part of the **reader** role). Creating, editing, and deleting collections and read lists is gated by dedicated permissions, granted to **maintainer** and **admin** by default:

| Permission | Grants |
|------------|--------|
| `collections:read` / `readlists:read` | Browse (all roles) |
| `collections:write` / `readlists:write` | Create / rename / edit / add / remove / reorder |
| `collections:delete` / `readlists:delete` | Delete the collection / read list |

You can grant the write/delete permissions to an individual reader via **Settings → Users** if you want a non-maintainer curator. Deleting a collection or read list never deletes the underlying series or books.

## Visibility

Members are filtered by your sharing-tag access: a shared collection or read list may reference series or books you can't see, and those are hidden from you (and excluded from the counts) without affecting other users.

## Third-party apps & e-readers

Collections and read lists are exposed read-only to external clients:

- **Komga-compatible apps** (e.g. Komic) see them through the [Komga API](./third-party-apps.md) once it's enabled.
- **OPDS readers** can browse both from the [OPDS catalog](./opds.md) (1.2 and 2.0).
