//! News for admins, imported from the Codex release feeds.
//!
//! Once a day the server downloads `https://updates.4sh.dev/feeds/<project>.json`
//! for each project in [`NEWS_PROJECTS`] and stores the entries. The request is
//! a plain GET: no instance id, no version in the URL or headers, and a
//! `User-Agent` without a version. Everything version-dependent (which
//! releases are new to this server, whether an update exists) is decided
//! locally from the stored entries.
//!
//! Failure is quiet by design. An air-gapped server, or one behind a proxy
//! that blocks the feed, keeps whatever it already stored and shows an empty
//! drawer otherwise; the fetch logs a warning and nothing else.

use anyhow::{Context, Result, bail};
use chrono::{DateTime, Duration, Utc};
use codex_db::entities::news_entries;
use codex_db::repositories::{
    NewNewsEntry, NewsEntryRepository, NewsFeedRepository, UserPreferencesRepository,
};
use reqwest::StatusCode;
use reqwest::header::{ETAG, IF_NONE_MATCH};
use sea_orm::DatabaseConnection;
use serde::Deserialize;
use tracing::{debug, warn};
use uuid::Uuid;

/// Where the feeds live. Compiled in: every release that ships this keeps
/// requesting it for years, so it is a domain Codex controls, never a URL on a
/// third-party host.
pub const NEWS_FEED_BASE_URL: &str = "https://updates.4sh.dev/feeds";

/// Feeds this server reads. Fixed: nothing else is relevant to a Codex admin.
pub const NEWS_PROJECTS: [&str; 2] = ["codex", "codex-ios"];

/// The project whose releases are compared against the running version.
pub const CODEX_PROJECT: &str = "codex";

/// User preference holding when an admin last opened the news drawer.
pub const LAST_SEEN_PREFERENCE: &str = "news.last_seen_at";

/// How far before the first fetch a fresh install still treats news as unread,
/// so an admin who installs Codex shortly after an announcement still sees it.
pub const FRESH_INSTALL_GRACE_DAYS: i64 = 30;

/// A fetch older than this is stale; startup refreshes it.
pub const STALE_AFTER_HOURS: i64 = 24;

/// Feed envelope version this server understands.
const SUPPORTED_FEED_VERSION: u64 = 1;

/// Responses larger than this are refused. A 50-entry feed is tens of KB.
const MAX_FEED_BYTES: usize = 2 * 1024 * 1024;

const REQUEST_TIMEOUT_SECS: u64 = 10;

/// Sent instead of reqwest's default so the feed host sees a product name
/// and nothing else: no version, which would fingerprint the instance.
const USER_AGENT: &str = "Codex";

#[derive(Debug, Deserialize)]
struct FeedEnvelope {
    version: u64,
    #[serde(default)]
    entries: Vec<serde_json::Value>,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum FeedKind {
    Release,
    Post,
}

impl FeedKind {
    fn as_str(&self) -> &'static str {
        match self {
            FeedKind::Release => "release",
            FeedKind::Post => "post",
        }
    }
}

#[derive(Debug, Deserialize)]
struct FeedEntry {
    id: String,
    project: String,
    kind: FeedKind,
    title: String,
    published_at: DateTime<Utc>,
    #[serde(default)]
    version: Option<String>,
    summary: String,
    body: String,
    #[serde(default)]
    url: Option<String>,
}

/// Parse one feed document.
///
/// Strict about the envelope, liberal about entries: an unsupported envelope
/// version is an error (the shape may have changed under us), while an entry
/// that does not parse, or that claims a different project, is skipped so one
/// bad entry cannot hide the rest. Unknown fields are ignored so the feed can
/// grow without breaking older servers.
pub fn parse_feed(project: &str, bytes: &[u8]) -> Result<Vec<NewNewsEntry>> {
    let envelope: FeedEnvelope =
        serde_json::from_slice(bytes).context("feed is not a valid JSON envelope")?;
    if envelope.version != SUPPORTED_FEED_VERSION {
        bail!(
            "unsupported feed version {} (this server reads version {})",
            envelope.version,
            SUPPORTED_FEED_VERSION
        );
    }

    let mut entries = Vec::with_capacity(envelope.entries.len());
    for raw in envelope.entries {
        let entry: FeedEntry = match serde_json::from_value(raw) {
            Ok(entry) => entry,
            Err(error) => {
                debug!(project, %error, "skipping a news entry that does not parse");
                continue;
            }
        };
        if entry.project != project {
            debug!(
                project,
                entry_project = %entry.project,
                id = %entry.id,
                "skipping a news entry from another project"
            );
            continue;
        }
        entries.push(NewNewsEntry {
            id: entry.id,
            project: entry.project,
            kind: entry.kind.as_str().to_string(),
            title: entry.title,
            version: entry.version,
            summary: entry.summary,
            body: entry.body,
            url: entry.url,
            published_at: entry.published_at,
        });
    }
    Ok(entries)
}

/// What one feed fetch did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FetchOutcome {
    /// 200: this many entries were stored (new or refreshed).
    Updated(usize),
    /// 304: the feed has not changed since the stored ETag.
    NotModified,
}

/// Downloads the feeds and stores what they contain.
pub struct NewsFetcher {
    client: reqwest::Client,
    base_url: String,
}

impl NewsFetcher {
    /// A fetcher reading from [`NEWS_FEED_BASE_URL`].
    pub fn new() -> Result<Self> {
        Self::with_base_url(NEWS_FEED_BASE_URL)
    }

    /// A fetcher reading from another base URL. Tests point this at a local
    /// stub; production always uses [`NewsFetcher::new`].
    pub fn with_base_url(base_url: &str) -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(REQUEST_TIMEOUT_SECS))
            .user_agent(USER_AGENT)
            .build()
            .context("building the news HTTP client")?;
        Ok(Self {
            client,
            base_url: base_url.trim_end_matches('/').to_string(),
        })
    }

    /// Fetch every feed in [`NEWS_PROJECTS`]. A failing feed is logged and
    /// does not stop the others.
    pub async fn fetch_all(&self, db: &DatabaseConnection) -> Vec<(String, Result<FetchOutcome>)> {
        let mut results = Vec::with_capacity(NEWS_PROJECTS.len());
        for project in NEWS_PROJECTS {
            let result = self.fetch_one(db, project).await;
            if let Err(error) = &result {
                warn!(project, error = %format!("{error:#}"), "news feed fetch failed");
            }
            results.push((project.to_string(), result));
        }
        results
    }

    /// Fetch one feed, store its entries, and record the success.
    pub async fn fetch_one(&self, db: &DatabaseConnection, project: &str) -> Result<FetchOutcome> {
        let url = format!("{}/{project}.json", self.base_url);
        let stored_etag = NewsFeedRepository::get(db, project)
            .await?
            .and_then(|feed| feed.etag);

        let mut request = self.client.get(&url);
        if let Some(etag) = &stored_etag {
            request = request.header(IF_NONE_MATCH, etag);
        }
        let mut response = request
            .send()
            .await
            .with_context(|| format!("requesting {url}"))?;

        if response.status() == StatusCode::NOT_MODIFIED {
            NewsFeedRepository::record_success(db, project, stored_etag, Utc::now()).await?;
            return Ok(FetchOutcome::NotModified);
        }
        if !response.status().is_success() {
            bail!("{url} answered {}", response.status());
        }

        let etag = response
            .headers()
            .get(ETAG)
            .and_then(|value| value.to_str().ok())
            .map(str::to_string);

        if response
            .content_length()
            .is_some_and(|len| len > MAX_FEED_BYTES as u64)
        {
            bail!("{url} is larger than {MAX_FEED_BYTES} bytes");
        }
        // Content-Length can be absent or wrong, so the limit is enforced on
        // the bytes actually read too.
        let mut body = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .with_context(|| format!("reading {url}"))?
        {
            if body.len() + chunk.len() > MAX_FEED_BYTES {
                bail!("{url} is larger than {MAX_FEED_BYTES} bytes");
            }
            body.extend_from_slice(&chunk);
        }

        let entries = parse_feed(project, &body).with_context(|| format!("parsing {url}"))?;
        let now = Utc::now();
        NewsEntryRepository::upsert_many(db, &entries, now).await?;
        NewsFeedRepository::record_success(db, project, etag, now).await?;
        Ok(FetchOutcome::Updated(entries.len()))
    }
}

/// Whether startup should fetch: some feed has never succeeded, or its last
/// success is older than [`STALE_AFTER_HOURS`].
pub async fn needs_refresh(db: &DatabaseConnection, now: DateTime<Utc>) -> Result<bool> {
    for project in NEWS_PROJECTS {
        let last = NewsFeedRepository::get(db, project)
            .await?
            .and_then(|feed| feed.last_success_at);
        match last {
            Some(at) if now - at < Duration::hours(STALE_AFTER_HOURS) => {}
            _ => return Ok(true),
        }
    }
    Ok(false)
}

/// The point after which entries count as unread for one admin.
///
/// The admin's own last-seen time when they have one. Otherwise a fallback
/// anchored on when this instance first stored news, minus a grace period:
/// without it a fresh install would open on 50 unread items, and with no grace
/// at all it would hide the announcement that prompted someone to install.
/// `None` only when nothing has ever been stored, in which case there is
/// nothing to be unread.
pub fn unread_baseline(
    last_seen: Option<DateTime<Utc>>,
    earliest_first_seen: Option<DateTime<Utc>>,
) -> Option<DateTime<Utc>> {
    last_seen.or_else(|| {
        earliest_first_seen.map(|first| first - Duration::days(FRESH_INSTALL_GRACE_DAYS))
    })
}

/// Whether `entry` is unread.
///
/// Unread means published after `baseline`, unless it is a Codex release this
/// server already runs (or has moved past): "Codex 2.6.3 is out" is not news
/// on a 2.6.3 server.
pub fn is_unread(
    entry: &news_entries::Model,
    baseline: Option<DateTime<Utc>>,
    running_version: &str,
) -> bool {
    let Some(baseline) = baseline else {
        return false;
    };
    if entry.published_at <= baseline {
        return false;
    }
    if entry.project == CODEX_PROJECT && entry.kind == "release" {
        let already_running = match (
            entry.version.as_deref().and_then(parse_version),
            parse_version(running_version),
        ) {
            (Some(release), Some(running)) => release <= running,
            _ => false,
        };
        if already_running {
            return false;
        }
    }
    true
}

/// Every release newer than the running build, highest version first: what
/// an admin would get by upgrading.
///
/// Compared by version, not by date, so a backported patch published after a
/// newer minor does not sort above it. Entries with unparseable versions are
/// skipped, and an unparseable running version yields nothing rather than a
/// guess.
pub fn newer_releases<'a>(
    releases: &'a [news_entries::Model],
    running_version: &str,
) -> Vec<&'a news_entries::Model> {
    let Some(running) = parse_version(running_version) else {
        return Vec::new();
    };
    let mut newer: Vec<(semver::Version, &news_entries::Model)> = releases
        .iter()
        .filter_map(|entry| {
            let version = entry.version.as_deref().and_then(parse_version)?;
            (version > running).then_some((version, entry))
        })
        .collect();
    newer.sort_by(|(a, _), (b, _)| b.cmp(a));
    newer.into_iter().map(|(_, entry)| entry).collect()
}

/// The newest Codex release that is newer than the running build, if any.
pub fn update_available<'a>(
    releases: &'a [news_entries::Model],
    running_version: &str,
) -> Option<&'a news_entries::Model> {
    newer_releases(releases, running_version).into_iter().next()
}

/// How many entries the admin view lists.
pub const NEWS_VIEW_LIMIT: u64 = 50;

/// What one admin sees in the news drawer.
#[derive(Debug, Clone)]
pub struct NewsView {
    /// Newest first, each with whether it is unread for this admin.
    pub entries: Vec<(news_entries::Model, bool)>,
    pub unread_count: usize,
    /// Codex releases above the running version, highest first. The first
    /// one, if any, is the update available.
    pub newer_releases: Vec<(news_entries::Model, bool)>,
    /// Most recent successful fetch of any feed.
    pub last_fetched_at: Option<DateTime<Utc>>,
}

/// Build the news view for one admin.
pub async fn load_news_view(
    db: &DatabaseConnection,
    user_id: Uuid,
    running_version: &str,
) -> Result<NewsView> {
    let entries = NewsEntryRepository::list_recent(db, &NEWS_PROJECTS, NEWS_VIEW_LIMIT).await?;
    let last_seen: Option<DateTime<Utc>> =
        UserPreferencesRepository::get_value(db, user_id, LAST_SEEN_PREFERENCE).await?;
    let baseline = unread_baseline(
        last_seen,
        NewsEntryRepository::earliest_first_seen(db).await?,
    );

    let entries: Vec<(news_entries::Model, bool)> = entries
        .into_iter()
        .map(|entry| {
            let unread = is_unread(&entry, baseline, running_version);
            (entry, unread)
        })
        .collect();
    let unread_count = entries.iter().filter(|(_, unread)| *unread).count();

    let releases = NewsEntryRepository::list_releases(db, CODEX_PROJECT).await?;
    let newer_releases = newer_releases(&releases, running_version)
        .into_iter()
        .map(|entry| (entry.clone(), is_unread(entry, baseline, running_version)))
        .collect();

    let mut last_fetched_at = None;
    for project in NEWS_PROJECTS {
        let at = NewsFeedRepository::get(db, project)
            .await?
            .and_then(|feed| feed.last_success_at);
        last_fetched_at = last_fetched_at.max(at);
    }

    Ok(NewsView {
        entries,
        unread_count,
        newer_releases,
        last_fetched_at,
    })
}

/// Mark everything published up to `now` as seen by this admin.
pub async fn mark_seen(db: &DatabaseConnection, user_id: Uuid, now: DateTime<Utc>) -> Result<()> {
    UserPreferencesRepository::set(db, user_id, LAST_SEEN_PREFERENCE, &now).await?;
    Ok(())
}

/// Parse a release version, tolerating a leading `v`.
fn parse_version(raw: &str) -> Option<semver::Version> {
    semver::Version::parse(raw.trim().trim_start_matches('v')).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn at(day: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 10, day, 12, 0, 0).unwrap()
    }

    fn model(project: &str, kind: &str, version: Option<&str>, day: u32) -> news_entries::Model {
        news_entries::Model {
            id: format!("{project}-{}", version.unwrap_or("post")),
            project: project.to_string(),
            kind: kind.to_string(),
            title: "t".to_string(),
            version: version.map(str::to_string),
            summary: "s".to_string(),
            body: "b".to_string(),
            url: None,
            published_at: at(day),
            first_seen_at: at(day),
            updated_at: at(day),
        }
    }

    // ---- parse_feed ----

    const GOOD_ENTRY: &str = r#"{
        "id": "codex-v2.6.3", "project": "codex", "kind": "release",
        "title": "Codex 2.6.3", "published_at": "2026-10-02T23:24:00Z",
        "version": "2.6.3", "summary": "Fixes", "body": "- fix",
        "url": "https://github.com/AshDevFr/codex/releases/tag/v2.6.3"
    }"#;

    fn feed(version: u64, entries: &[&str]) -> Vec<u8> {
        format!(
            r#"{{"version": {version}, "project": "codex", "generated_at": "2026-10-17T00:00:00Z", "entries": [{}]}}"#,
            entries.join(",")
        )
        .into_bytes()
    }

    #[test]
    fn a_valid_feed_parses() {
        let entries = parse_feed("codex", &feed(1, &[GOOD_ENTRY])).unwrap();
        assert_eq!(entries.len(), 1);
        let e = &entries[0];
        assert_eq!(e.id, "codex-v2.6.3");
        assert_eq!(e.kind, "release");
        assert_eq!(e.version.as_deref(), Some("2.6.3"));
        assert_eq!(
            e.published_at,
            Utc.with_ymd_and_hms(2026, 10, 2, 23, 24, 0).unwrap()
        );
    }

    #[test]
    fn an_unsupported_envelope_version_is_rejected() {
        let error = parse_feed("codex", &feed(2, &[GOOD_ENTRY])).unwrap_err();
        assert!(format!("{error:#}").contains("unsupported feed version 2"));
    }

    #[test]
    fn a_body_that_is_not_an_envelope_is_rejected() {
        assert!(parse_feed("codex", b"<html>").is_err());
        assert!(parse_feed("codex", b"[]").is_err());
    }

    #[test]
    fn one_bad_entry_does_not_hide_the_rest() {
        let missing_title = r#"{"id": "x", "project": "codex", "kind": "release",
            "published_at": "2026-10-02T23:24:00Z", "summary": "s", "body": "b"}"#;
        let unknown_kind = r#"{"id": "y", "project": "codex", "kind": "poll", "title": "t",
            "published_at": "2026-10-02T23:24:00Z", "summary": "s", "body": "b"}"#;
        let bad_date = r#"{"id": "z", "project": "codex", "kind": "post", "title": "t",
            "published_at": "yesterday", "summary": "s", "body": "b"}"#;

        let entries = parse_feed(
            "codex",
            &feed(1, &[missing_title, GOOD_ENTRY, unknown_kind, bad_date]),
        )
        .unwrap();
        let ids: Vec<_> = entries.iter().map(|e| e.id.as_str()).collect();
        assert_eq!(ids, vec!["codex-v2.6.3"]);
    }

    #[test]
    fn an_entry_from_another_project_is_skipped() {
        let entries = parse_feed("codex-ios", &feed(1, &[GOOD_ENTRY])).unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn unknown_fields_are_ignored_and_optional_fields_may_be_absent() {
        let post = r#"{"id": "post-hi", "project": "codex", "kind": "post", "title": "Hi",
            "published_at": "2026-10-02T23:24:00Z", "summary": "s", "body": "b",
            "image": "https://example.com/x.png", "draft": false}"#;
        let entries = parse_feed("codex", &feed(1, &[post])).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].kind, "post");
        assert!(entries[0].version.is_none());
        assert!(entries[0].url.is_none());
    }

    // ---- unread_baseline ----

    #[test]
    fn the_baseline_is_the_admins_last_seen_when_set() {
        assert_eq!(unread_baseline(Some(at(10)), Some(at(1))), Some(at(10)));
    }

    #[test]
    fn a_fresh_install_falls_back_to_first_fetch_minus_the_grace() {
        assert_eq!(
            unread_baseline(None, Some(at(31))),
            Some(at(31) - Duration::days(FRESH_INSTALL_GRACE_DAYS))
        );
    }

    #[test]
    fn nothing_stored_means_no_baseline() {
        assert_eq!(unread_baseline(None, None), None);
    }

    // ---- is_unread ----

    #[test]
    fn entries_after_the_baseline_are_unread() {
        let ios = model("codex-ios", "release", Some("1.1.0"), 16);
        assert!(is_unread(&ios, Some(at(15)), "2.6.3"));
        assert!(
            !is_unread(&ios, Some(at(16)), "2.6.3"),
            "equal is not after"
        );
        assert!(!is_unread(&ios, Some(at(17)), "2.6.3"));
        assert!(!is_unread(&ios, None, "2.6.3"));
    }

    #[test]
    fn a_codex_release_this_server_already_runs_is_not_news() {
        let same = model("codex", "release", Some("2.6.3"), 16);
        let older = model("codex", "release", Some("2.6.2"), 16);
        let newer = model("codex", "release", Some("2.7.0"), 16);
        assert!(!is_unread(&same, Some(at(1)), "2.6.3"));
        assert!(!is_unread(&older, Some(at(1)), "2.6.3"));
        assert!(is_unread(&newer, Some(at(1)), "2.6.3"));
    }

    #[test]
    fn version_suppression_applies_only_to_codex_releases() {
        // An iOS version number has nothing to do with the server's.
        let ios = model("codex-ios", "release", Some("1.0"), 16);
        assert!(is_unread(&ios, Some(at(1)), "2.6.3"));
        let post = model("codex", "post", None, 16);
        assert!(is_unread(&post, Some(at(1)), "2.6.3"));
    }

    #[test]
    fn an_unparseable_version_is_never_suppressed() {
        let odd = model("codex", "release", Some("nightly"), 16);
        assert!(is_unread(&odd, Some(at(1)), "2.6.3"));
        let normal = model("codex", "release", Some("2.6.3"), 16);
        assert!(is_unread(&normal, Some(at(1)), "dev"));
    }

    // ---- newer_releases ----

    fn versions(releases: &[&news_entries::Model]) -> Vec<String> {
        releases
            .iter()
            .map(|e| e.version.clone().unwrap_or_default())
            .collect()
    }

    #[test]
    fn newer_releases_are_every_release_above_the_running_one_highest_first() {
        let releases = vec![
            model("codex", "release", Some("2.6.4"), 20), // backport, newest by date
            model("codex", "release", Some("2.7.0"), 18),
            model("codex", "release", Some("2.6.3"), 2),
            model("codex", "release", Some("2.6.2"), 1),
            model("codex", "release", Some("2.10.0"), 25),
        ];
        assert_eq!(
            versions(&newer_releases(&releases, "2.6.3")),
            vec!["2.10.0", "2.7.0", "2.6.4"],
            "sorted by version, not by date or string order"
        );
    }

    #[test]
    fn newer_releases_skip_unparseable_versions_and_need_a_parseable_running_version() {
        let releases = vec![
            model("codex", "release", Some("nightly"), 2),
            model("codex", "release", Some("2.7.0"), 2),
        ];
        assert_eq!(versions(&newer_releases(&releases, "2.6.3")), vec!["2.7.0"]);
        assert!(newer_releases(&releases, "dev").is_empty());
        assert!(newer_releases(&releases, "2.7.0").is_empty());
    }

    // ---- update_available ----

    #[test]
    fn the_highest_newer_version_wins_regardless_of_date() {
        let releases = vec![
            model("codex", "release", Some("2.6.4"), 20), // backport, newest by date
            model("codex", "release", Some("2.7.0"), 18),
            model("codex", "release", Some("2.6.3"), 2),
        ];
        let latest = update_available(&releases, "2.6.3").unwrap();
        assert_eq!(latest.version.as_deref(), Some("2.7.0"));
    }

    #[test]
    fn no_update_when_running_the_latest_or_newer() {
        let releases = vec![model("codex", "release", Some("2.6.3"), 2)];
        assert!(update_available(&releases, "2.6.3").is_none());
        assert!(update_available(&releases, "2.7.0").is_none());
    }

    #[test]
    fn a_prerelease_build_sees_its_final_release_as_an_update() {
        let releases = vec![model("codex", "release", Some("2.7.0"), 2)];
        assert!(update_available(&releases, "2.7.0-rc.1").is_some());
    }

    #[test]
    fn unparseable_versions_yield_no_update() {
        let releases = vec![
            model("codex", "release", Some("latest"), 2),
            model("codex", "release", Some("2.7.0"), 2),
        ];
        assert!(update_available(&releases, "dev").is_none());
        assert_eq!(
            update_available(&releases, "v2.6.3")
                .unwrap()
                .version
                .as_deref(),
            Some("2.7.0"),
            "a leading v on either side is tolerated"
        );
    }
}
