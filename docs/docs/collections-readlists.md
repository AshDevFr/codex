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

### Hand-picked or automatic

Every collection is one of two kinds, chosen when you create it:

- **Hand-picked** (the default): you decide which series belong, one at a time. This is the behaviour described above.
- **Automatic**: you write a **rule**, and every series matching it belongs. "Isekai" becomes *every series tagged `isekai` or `reincarnation`* instead of a list somebody has to keep topping up.

An automatic collection is a saved search with a name and a cover. Switch the **Membership** toggle in the collection form to **Automatic** and build the rule with the same filter builder used on the [advanced search page](./filtering.md). If you have already saved a filter preset you like, open **Manage presets** and use the collection button on the preset to start from a copy of its condition.

#### Contents are always current

The rule is evaluated every time someone opens the collection. There is no refresh button, no nightly job, and no "last updated" to worry about:

- A series imported by a scan appears the moment it matches.
- Editing the rule changes the contents on the next page load.
- Removing a tag from a series takes it out immediately.

#### You cannot edit the members by hand

Add, remove and reorder are refused on an automatic collection, and the UI does not offer them. Its members *are* the rule, so there is nothing to add to.

If a series is in the collection and shouldn't be (or the other way round), the fix is either the rule or **the series' own metadata**. Fixing the metadata is usually the right answer: it also corrects search, every other rule, and what your e-reader sees, instead of hiding the correction inside one collection.

An automatic collection is also never *ordered*. Nobody arranged the members, so there is no manual order to keep; sort the page by title, date added or release year instead.

#### Rules that use your own data show different contents to different people

Most rules describe the library: tags, genres, publisher, release year. Those look the same to everyone.

Some rules describe **you**: your own rating, your reading progress, whether you have rated something. A collection built on those resolves against whoever is looking, so two people on the same server genuinely see different series in it. A rule like "my rating is at least 8.5" works exactly as you would want for a personal favourites shelf, but it will confuse anyone comparing notes with you.

Codex labels these with a **Personal** badge on the collection card and detail page, and warns you in the form while you are building the rule. Nothing prevents you from creating one: it is a useful thing to have, as long as everyone knows.

#### What automatic collections deliberately do not do

- **No member count in the collections list.** Counting one means running its rule; showing counts for a page of them would mean running every rule on the server. The count appears on the collection's own page, where the members are loaded anyway.
- **A series page will not say it is in an automatic collection.** Answering that question would mean evaluating every stored rule against that one series, and the answer could differ per viewer.
- **The "in a collection" search filter ignores them.** Otherwise a rule could ask about collections whose contents are themselves rules, and Codex would have to unpick the circle.
- **Series only.** Read lists are always hand-picked.

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

Members are filtered by your sharing-tag access: a shared collection or read list may reference series or books you can't see, and those are hidden from you (and excluded from the counts) without affecting other users. This applies to automatic collections too: a rule can only ever show you series you already have access to.

## Third-party apps & e-readers

Collections and read lists are exposed read-only to external clients:

- **Komga-compatible apps** (e.g. Komic) see them through the [Komga API](./third-party-apps.md) once it's enabled.
- **OPDS readers** can browse both from the [OPDS catalog](./opds.md) (1.2 and 2.0).

Automatic collections appear to these clients as ordinary collections. Their contents are resolved when the client asks, so an app that has no concept of rules still sees an up-to-date list of series.
