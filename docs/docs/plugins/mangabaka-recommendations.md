---
---

# MangaBaka Recommendations Plugin

The MangaBaka Recommendations plugin suggests manga, manhwa, and manhua from [MangaBaka](https://mangabaka.org), seeded by your library. Recommendations appear on the **Recommendations** page in Codex with cover art, summary, and an explanation of why each one was picked.

Unlike the [AniList Recommendations](./anilist-recommendations.md) plugin, **it needs no account, no OAuth, and no API key**. Enable it and it works.

This is a **user plugin**: each user gets recommendations based on their own library and ratings.

## Features

- No authentication of any kind. Every endpoint it uses is public.
- Two independent recommendation signals, combined and individually tunable.
- Aggressive franchise filtering, so a seed's own spin-offs don't fill the list.
- Per-user filters for content rating, series type, genre, tag, and minimum rating.
- Recognises series you already own even when they were matched by a different provider.

## How it works

The plugin combines two signals that answer different questions.

| Signal | Question it answers | MangaBaka endpoint |
|--------|--------------------|--------------------|
| Tag similarity | "What else is like the things I read?" | `/v1/series/mix` |
| Reader overlap | "What do people who read this also read?" | `/v1/series/{id}/readers-also-like` |

Codex sends the plugin a curated set of seeds: your highest-rated series first, then your most recent reads. All of them go to the tag-similarity endpoint in a single request, which combines them into one profile and returns matches ranked against your library as a whole. Separately, your top few favourites are looked up individually for reader overlap.

Tag similarity on its own tends to return more of what you already have, so reader overlap supplies the taste signal it cannot. A series that **both** signals return is the strongest evidence available, and is ranked above anything either found alone. Each recommendation's reason says which signal produced it.

### Keeping franchises out

Tag similarity ranks a series' own spin-offs and per-arc volumes as its nearest neighbours, because they genuinely are. Left alone, seeding with Re:Zero returns five Re:Zero volumes and nothing else. The plugin therefore drops anything MangaBaka flags as related to a seed, anything whose own relationship data points back at a seed, and anything whose title matches a seed's once normalised. It then collapses each remaining franchise to its single best entry.

Series sharing an author with something you read are ranked lower but kept, since an author's *unrelated* other work is often exactly what you want next. You can hide them entirely in settings.

This is deliberately aggressive. On a library dominated by one franchise you will get fewer recommendations than requested rather than a list padded with spin-offs.

## Prerequisites

**Run a metadata match with the [MangaBaka Metadata](./mangabaka.md) plugin first.** Seeds are resolved from the `api:mangabaka` external IDs that plugin writes onto your series. Series without one cannot seed recommendations, and a library with none produces nothing at all.

There is deliberately no title-search fallback. Because all seeds are combined into a single profile, one bad title match would skew every result rather than just its own, so unmatched series are skipped instead of guessed at.

## Admin setup

1. Go to **Settings → Plugins → Browse official plugins**.
2. Pick **MangaBaka Recommendations** and click **Install**.
3. Leave the defaults and save. There are no credentials to enter.

Users then enable it individually under **Settings → Integrations**.

### Admin configuration

| Setting | Default | Purpose |
|---------|---------|---------|
| Request Timeout | 30 | HTTP timeout in seconds for calls to MangaBaka. |
| API Base URL | `https://api.mangabaka.org` | Override the API host. Rarely needed; see the stability note below. |

## Configuration

All per-user settings are optional and have working defaults. Multi-value settings are comma-separated, for example `safe,suggestive`.

| Setting | Default | What it does |
|---------|---------|--------------|
| Content Ratings | *(all)* | Limit results to `safe`, `suggestive`, `erotica`, or `pornographic`. |
| Included Types | *(all)* | Keep only these types: `manga`, `manhwa`, `manhua`, `novel`, `oel`. |
| Excluded Types | *(none)* | Drop these types. Useful for keeping light novels out. |
| Excluded Genres | *(none)* | Drop results carrying any of these genres. |
| Excluded Tags | *(none)* | Drop results carrying any of these tags, **by name**. |
| Minimum Rating | 0 | Only recommend series rated at least this highly on MangaBaka (0-100). |
| Similarity vs. Reader Overlap | 0.5 | Balance between the two signals. Higher favours tag similarity, lower favours reader overlap. |
| Reader Overlap Seeds | 5 | How many favourites to check reader overlap for. **0 turns that signal off.** |
| Exclude Same Author | off | Hide, rather than rank lower, series by an author you already read. |

### Excluded tags

Tags are given by name (`Death Game`, `Gore`) and matched against MangaBaka's tag list ignoring case. Names that don't match anything are skipped with a warning rather than failing the run. The underlying API requires numeric tag IDs; the plugin looks them up for you and caches the list.

### Where filters are applied

Filters are sent to MangaBaka as part of the request wherever the API supports it, so a filtered-out series never occupies a slot in the results. The reader-overlap endpoint accepts only the content rating and tag filters, so the remaining ones are additionally applied locally to anything it returns.

## Limitations

- **Manga, manhwa, and manhua only.** MangaBaka does not cover western comics or general ebooks. Enabling this on such a library will produce nothing useful.
- **The upstream recommendation endpoints are beta.** MangaBaka marks both endpoints this plugin depends on as beta, meaning they may change or disappear without notice. Responses are parsed defensively so an upstream change degrades recommendation quality rather than breaking the task, but breakage is a realistic possibility. The **API Base URL** admin setting exists as the escape hatch.
- **Quality follows your metadata.** The more of your library MangaBaka can identify, the better the profile it builds.

## Privacy

The plugin sends the MangaBaka IDs of your seed series to MangaBaka, along with any filters you configured. It sends no account identifier, no titles, and no reading history, because it has no account to attach them to. Requests are unauthenticated and indistinguishable from any other anonymous caller.

## Troubleshooting

### "No recommendations" after a refresh

Most often none of your library could be resolved to MangaBaka series. Run [MangaBaka Metadata](./mangabaka.md) over the library and refresh. The plugin logs how many entries it skipped for this reason.

Otherwise, your filters may be too tight. A narrow content rating combined with a high minimum rating can empty the result. Loosen one at a time.

### Fewer results than I asked for

Expected on a library dominated by one franchise: almost everything similar to your seeds is a related work, and those are removed rather than shown. Seeding from a wider range of series gives the profile more to work with.

### The results all look like the same kind of thing

Lower **Similarity vs. Reader Overlap** to favour what other readers actually read, which is less bound to your library's existing tag profile. Setting it near 0 leans almost entirely on reader overlap.

### Recommendations include things I've already read

Exclusion relies on your library entries carrying a MangaBaka ID or a cross-referenced ID from another provider. If a series was matched manually and no external ID was written back, it can be re-recommended. Run [MangaBaka Metadata](./mangabaka.md) or attach an `api:mangabaka` ID via the per-series **Edit External IDs** modal.

## Next steps

- [MangaBaka Metadata](./mangabaka.md): populate the external IDs this plugin seeds from.
- [AniList Recommendations](./anilist-recommendations.md): the alternative provider, if you have an AniList account.
- [Plugins overview](./index.md): the security and privacy model for user plugins.
