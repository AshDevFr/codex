//! In-memory cache of open PDFium document handles.
//!
//! Re-opening a multi-hundred-page PDF on every page request is the dominant
//! cost on the streaming-reader hot path. This service holds a bounded number of
//! already-parsed `PdfDocument` handles in process memory so that page renders
//! after the first one for a given book skip the per-page PDFium open.
//!
//! The handler integration that actually calls into this cache is staged
//! separately, so the public surface (stats accessors, snapshot DTOs,
//! `get_or_open`, `evict`, `clear`) is currently exercised only by the unit
//! tests in this module. `cargo clippy` without `--tests` therefore sees it
//! as dead code; the module-level allow below silences that until the page
//! handler is wired up.
#![allow(dead_code)]
//!
//! ## Eviction model
//!
//! Two complementary policies bound memory:
//!
//! 1. **Capacity LRU** is the hard cap. When `capacity` is reached, the least-
//!    recently-used entry is dropped on the next insert.
//! 2. **Idle TTL** drives normal load-shedding. A background sweeper closes any
//!    handle whose `last_used` exceeds `idle_ttl`. Hot books stay resident;
//!    cold books get released back to the OS.
//!
//! ## Concurrency contract
//!
//! `get_or_open` returns `Arc<tokio::sync::Mutex<V>>`. Callers serialise
//! renders of the same book behind that mutex; renders of different books run
//! in parallel. `V` is generic so this module can be unit-tested without
//! needing a real PDFium runtime. Production uses `V = PdfDocument<'static>`.

use anyhow::Result;
use lru::LruCache;
use serde::Serialize;
use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
use tokio::sync::Mutex as AsyncMutex;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};
use uuid::Uuid;

/// Compile-time check: the production value type can be cached safely.
///
/// `PdfDocument<'static>` is declared `Send + Sync` by pdfium-render (with the
/// `thread_safe` feature, which is on by default). When the document borrows a
/// `&'static Pdfium`, it satisfies `'static` too. This `const` keeps that
/// invariant honest if the upstream crate ever changes its bounds.
const _: fn() = || {
    fn assert_cacheable<T: Send + Sync + 'static>() {}
    assert_cacheable::<pdfium_render::prelude::PdfDocument<'static>>();
};

/// Provides the current `Instant`. Injected so idle-TTL tests are deterministic.
pub trait Clock: Send + Sync + 'static {
    fn now(&self) -> Instant;
}

/// Production clock backed by `Instant::now`.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> Instant {
        Instant::now()
    }
}

/// Atomic counters for cache activity. Cheap to read, no allocation.
#[derive(Default, Debug)]
pub struct HandleCacheStats {
    pub hits: AtomicU64,
    pub misses: AtomicU64,
    pub opens: AtomicU64,
    pub evictions: AtomicU64,
    pub idle_evictions: AtomicU64,
}

impl HandleCacheStats {
    pub fn hits(&self) -> u64 {
        self.hits.load(Ordering::Relaxed)
    }
    pub fn misses(&self) -> u64 {
        self.misses.load(Ordering::Relaxed)
    }
    pub fn opens(&self) -> u64 {
        self.opens.load(Ordering::Relaxed)
    }
    pub fn evictions(&self) -> u64 {
        self.evictions.load(Ordering::Relaxed)
    }
    pub fn idle_evictions(&self) -> u64 {
        self.idle_evictions.load(Ordering::Relaxed)
    }
}

/// Serialisable snapshot of cache state for the admin UI.
#[derive(Debug, Clone, Default, Serialize)]
pub struct HandleCacheSnapshot {
    pub enabled: bool,
    pub capacity: usize,
    pub idle_ttl_seconds: u64,
    pub current_size: usize,
    pub hits: u64,
    pub misses: u64,
    pub opens: u64,
    pub evictions: u64,
    pub idle_evictions: u64,
    pub entries: Vec<HandleCacheEntrySnapshot>,
}

/// Per-book entry view in a snapshot.
#[derive(Debug, Clone, Serialize)]
pub struct HandleCacheEntrySnapshot {
    pub book_id: Uuid,
    pub path: String,
    pub age_seconds: u64,
    pub idle_seconds: u64,
    pub render_count: u64,
}

struct Entry<V> {
    doc: Arc<AsyncMutex<V>>,
    path: PathBuf,
    opened_at: Instant,
    last_used: Instant,
    render_count: u64,
}

/// In-memory LRU cache of open document handles keyed by book id.
///
/// `V` defaults to `PdfDocument<'static>` so callers can use the bare
/// `PdfHandleCache` type alias in `AppState`. Tests instantiate with a
/// fake value type that does not require a live PDFium runtime.
pub struct PdfHandleCache<V = pdfium_render::prelude::PdfDocument<'static>>
where
    V: Send + Sync + 'static,
{
    /// Whether the cache is active. When `false`, `get_or_open` always opens
    /// and never stores, restoring the legacy "every-request open" behaviour
    /// for operators who need to bypass the cache without redeploying.
    enabled: bool,
    capacity: NonZeroUsize,
    idle_ttl: Duration,
    inner: StdMutex<LruCache<Uuid, Entry<V>>>,
    stats: Arc<HandleCacheStats>,
    clock: Arc<dyn Clock>,
}

impl<V> PdfHandleCache<V>
where
    V: Send + Sync + 'static,
{
    /// Construct a new cache. `capacity` must be at least 1; values <= 0
    /// fall back to 1 to keep `NonZeroUsize` happy.
    pub fn new(capacity: usize, idle_ttl: Duration, enabled: bool) -> Self {
        Self::with_clock(capacity, idle_ttl, enabled, Arc::new(SystemClock))
    }

    /// Like `new`, but with an injectable clock. Used by tests.
    pub fn with_clock(
        capacity: usize,
        idle_ttl: Duration,
        enabled: bool,
        clock: Arc<dyn Clock>,
    ) -> Self {
        let capacity = NonZeroUsize::new(capacity.max(1)).expect("max(1) is always > 0");
        Self {
            enabled,
            capacity,
            idle_ttl,
            inner: StdMutex::new(LruCache::new(capacity)),
            stats: Arc::new(HandleCacheStats::default()),
            clock,
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn capacity(&self) -> usize {
        self.capacity.get()
    }

    pub fn idle_ttl(&self) -> Duration {
        self.idle_ttl
    }

    pub fn stats(&self) -> &HandleCacheStats {
        &self.stats
    }

    /// Look up a cached handle, or run `opener` to load one and cache it.
    ///
    /// `opener` is invoked while no internal lock is held, so a slow PDFium
    /// open does not block other cache operations. The whole function is sync
    /// because PDFium calls are CPU-bound; callers should invoke it inside
    /// `spawn_blocking` to keep the async runtime free during cold opens.
    ///
    /// When two callers race on the same `book_id`, both may invoke `opener`,
    /// but only one's handle is stored; both calls return the same `Arc`
    /// (the winner of the insert race). The redundant open is acceptable for
    /// the realistic load shape (rare cold misses on the same book).
    ///
    /// When the cache is disabled, every call invokes `opener` and the result
    /// is wrapped in a fresh `Arc<Mutex<_>>` without being inserted into the
    /// cache. Callers should hold the returned `Arc` for the duration of the
    /// render to keep the document alive.
    pub fn get_or_open<F>(
        &self,
        book_id: Uuid,
        path: PathBuf,
        opener: F,
    ) -> Result<Arc<AsyncMutex<V>>>
    where
        F: FnOnce() -> Result<V> + Send,
    {
        if !self.enabled {
            let doc = opener()?;
            self.stats.opens.fetch_add(1, Ordering::Relaxed);
            return Ok(Arc::new(AsyncMutex::new(doc)));
        }

        // Fast path: hit
        {
            let mut lru = self.inner.lock().expect("lru mutex poisoned");
            if let Some(entry) = lru.get_mut(&book_id) {
                self.stats.hits.fetch_add(1, Ordering::Relaxed);
                entry.last_used = self.clock.now();
                entry.render_count = entry.render_count.saturating_add(1);
                debug!(
                    %book_id,
                    file = %entry.path.display(),
                    "pdf handle cache hit"
                );
                return Ok(entry.doc.clone());
            }
        }

        // Slow path: miss. Open without holding the lock.
        self.stats.misses.fetch_add(1, Ordering::Relaxed);
        let open_start = self.clock.now();
        let doc = match opener() {
            Ok(d) => d,
            Err(e) => {
                warn!(%book_id, error = %e, "pdf handle open failed");
                return Err(e);
            }
        };
        self.stats.opens.fetch_add(1, Ordering::Relaxed);
        let opened_at = self.clock.now();
        let open_elapsed_ms = opened_at.saturating_duration_since(open_start).as_millis() as u64;
        info!(
            %book_id,
            file = %path.display(),
            elapsed_ms = open_elapsed_ms,
            "pdf handle opened"
        );

        let doc = Arc::new(AsyncMutex::new(doc));
        let entry = Entry {
            doc: doc.clone(),
            path: path.clone(),
            opened_at,
            last_used: opened_at,
            render_count: 1,
        };

        let mut lru = self.inner.lock().expect("lru mutex poisoned");
        // If a concurrent caller already inserted, prefer the existing handle
        // so both racers see the same Arc.
        if let Some(existing) = lru.get_mut(&book_id) {
            self.stats.hits.fetch_add(1, Ordering::Relaxed);
            existing.last_used = self.clock.now();
            existing.render_count = existing.render_count.saturating_add(1);
            return Ok(existing.doc.clone());
        }
        // Capacity eviction: if we're at the cap, `put` evicts the LRU.
        if lru.len() >= lru.cap().get() {
            self.stats.evictions.fetch_add(1, Ordering::Relaxed);
        }
        lru.put(book_id, entry);
        Ok(doc)
    }

    /// Drop the handle for a single book. Returns true if an entry was present.
    pub fn evict(&self, book_id: Uuid) -> bool {
        let mut lru = self.inner.lock().expect("lru mutex poisoned");
        let removed = lru.pop(&book_id).is_some();
        if removed {
            self.stats.evictions.fetch_add(1, Ordering::Relaxed);
            debug!(%book_id, "pdf handle evicted");
        }
        removed
    }

    /// Drop every cached handle. Returns the number of entries cleared.
    pub fn clear(&self) -> usize {
        let mut lru = self.inner.lock().expect("lru mutex poisoned");
        let count = lru.len();
        lru.clear();
        if count > 0 {
            self.stats
                .evictions
                .fetch_add(count as u64, Ordering::Relaxed);
            info!(count, "pdf handle cache cleared");
        }
        count
    }

    /// Snapshot for the admin UI.
    pub fn snapshot(&self) -> HandleCacheSnapshot {
        let lru = self.inner.lock().expect("lru mutex poisoned");
        let now = self.clock.now();
        let entries = lru
            .iter()
            .map(|(book_id, entry)| HandleCacheEntrySnapshot {
                book_id: *book_id,
                path: entry.path.display().to_string(),
                age_seconds: now.saturating_duration_since(entry.opened_at).as_secs(),
                idle_seconds: now.saturating_duration_since(entry.last_used).as_secs(),
                render_count: entry.render_count,
            })
            .collect();

        HandleCacheSnapshot {
            enabled: self.enabled,
            capacity: self.capacity.get(),
            idle_ttl_seconds: self.idle_ttl.as_secs(),
            current_size: lru.len(),
            hits: self.stats.hits(),
            misses: self.stats.misses(),
            opens: self.stats.opens(),
            evictions: self.stats.evictions(),
            idle_evictions: self.stats.idle_evictions(),
            entries,
        }
    }

    /// Walk the cache once and drop entries whose last-used age exceeds the
    /// idle TTL. Returns the number of entries evicted by this pass.
    pub fn sweep_idle(&self) -> usize {
        if self.idle_ttl.is_zero() {
            return 0;
        }
        let mut lru = self.inner.lock().expect("lru mutex poisoned");
        let now = self.clock.now();
        let stale: Vec<Uuid> = lru
            .iter()
            .filter_map(|(book_id, entry)| {
                let idle = now.saturating_duration_since(entry.last_used);
                if idle >= self.idle_ttl {
                    Some(*book_id)
                } else {
                    None
                }
            })
            .collect();
        let count = stale.len();
        for book_id in stale {
            if lru.pop(&book_id).is_some() {
                self.stats.idle_evictions.fetch_add(1, Ordering::Relaxed);
                debug!(%book_id, "pdf handle idle-evicted");
            }
        }
        if count > 0 {
            info!(count, "pdf handle cache idle sweep");
        }
        count
    }
}

impl PdfHandleCache<pdfium_render::prelude::PdfDocument<'static>> {
    /// Spawn a background task that sweeps idle entries every `interval`.
    ///
    /// The task exits cleanly when `cancel` is triggered. Returns the
    /// `JoinHandle` so the caller can await graceful shutdown alongside the
    /// other background services.
    pub fn spawn_sweeper(
        self: Arc<Self>,
        interval: Duration,
        cancel: CancellationToken,
    ) -> JoinHandle<()> {
        tokio::spawn(async move {
            // Disabled cache or pathological interval: nothing to do.
            if !self.enabled || interval.is_zero() {
                return;
            }
            let mut ticker = tokio::time::interval(interval);
            ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            // Skip the immediate first tick that `interval` fires.
            ticker.tick().await;
            loop {
                tokio::select! {
                    _ = cancel.cancelled() => {
                        debug!("pdf handle cache sweeper shutting down");
                        break;
                    }
                    _ = ticker.tick() => {
                        self.sweep_idle();
                    }
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    /// Test value: stand-in for `PdfDocument`. Tracks an id so we can
    /// distinguish "the same Arc" from "a different Arc with the same content".
    #[derive(Debug)]
    struct TestDoc {
        #[allow(dead_code)]
        id: u32,
    }

    #[derive(Clone, Default)]
    struct TestClock {
        offset_nanos: Arc<AtomicU64>,
        base: Arc<StdMutex<Option<Instant>>>,
    }

    impl TestClock {
        fn new() -> Self {
            Self::default()
        }

        fn advance(&self, by: Duration) {
            self.offset_nanos
                .fetch_add(by.as_nanos() as u64, Ordering::Relaxed);
        }
    }

    impl Clock for TestClock {
        fn now(&self) -> Instant {
            let mut base = self.base.lock().unwrap();
            let base = *base.get_or_insert_with(Instant::now);
            base + Duration::from_nanos(self.offset_nanos.load(Ordering::Relaxed))
        }
    }

    fn cache(capacity: usize, idle_ttl: Duration) -> (PdfHandleCache<TestDoc>, TestClock) {
        let clock = TestClock::new();
        let cache = PdfHandleCache::<TestDoc>::with_clock(
            capacity,
            idle_ttl,
            true,
            Arc::new(clock.clone()),
        );
        (cache, clock)
    }

    fn opener(id: u32, calls: Arc<AtomicUsize>) -> impl FnOnce() -> Result<TestDoc> {
        move || {
            calls.fetch_add(1, Ordering::Relaxed);
            Ok(TestDoc { id })
        }
    }

    #[test]
    fn get_or_open_hits_after_first_open() {
        let (cache, _clock) = cache(4, Duration::from_secs(60));
        let book = Uuid::new_v4();
        let calls = Arc::new(AtomicUsize::new(0));

        let a = cache
            .get_or_open(book, PathBuf::from("/tmp/a.pdf"), opener(1, calls.clone()))
            .unwrap();
        let b = cache
            .get_or_open(book, PathBuf::from("/tmp/a.pdf"), opener(99, calls.clone()))
            .unwrap();

        assert_eq!(calls.load(Ordering::Relaxed), 1, "opener called once");
        assert!(
            Arc::ptr_eq(&a, &b),
            "subsequent gets must return the same Arc"
        );
        assert_eq!(cache.stats().hits(), 1);
        assert_eq!(cache.stats().misses(), 1);
        assert_eq!(cache.stats().opens(), 1);
    }

    #[test]
    fn capacity_evicts_least_recently_used() {
        let (cache, _clock) = cache(2, Duration::from_secs(60));
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        let c = Uuid::new_v4();
        let calls = Arc::new(AtomicUsize::new(0));

        let _ = cache
            .get_or_open(a, PathBuf::from("a"), opener(1, calls.clone()))
            .unwrap();
        let _ = cache
            .get_or_open(b, PathBuf::from("b"), opener(2, calls.clone()))
            .unwrap();
        // Touch `a` so `b` becomes LRU.
        let _ = cache
            .get_or_open(a, PathBuf::from("a"), opener(99, calls.clone()))
            .unwrap();
        // Insert `c`, evicting `b`.
        let _ = cache
            .get_or_open(c, PathBuf::from("c"), opener(3, calls.clone()))
            .unwrap();

        let snap = cache.snapshot();
        assert_eq!(snap.current_size, 2);
        let present: std::collections::HashSet<_> =
            snap.entries.iter().map(|e| e.book_id).collect();
        assert!(present.contains(&a));
        assert!(present.contains(&c));
        assert!(!present.contains(&b));
        assert_eq!(cache.stats().evictions(), 1);
    }

    #[test]
    fn idle_sweep_drops_stale_entries() {
        let idle = Duration::from_secs(10);
        let (cache, clock) = cache(4, idle);
        let book = Uuid::new_v4();
        let calls = Arc::new(AtomicUsize::new(0));

        let _ = cache
            .get_or_open(book, PathBuf::from("a"), opener(1, calls.clone()))
            .unwrap();
        assert_eq!(cache.snapshot().current_size, 1);

        // Advance past the TTL.
        clock.advance(idle + Duration::from_secs(1));
        let evicted = cache.sweep_idle();
        assert_eq!(evicted, 1);
        assert_eq!(cache.snapshot().current_size, 0);
        assert_eq!(cache.stats().idle_evictions(), 1);

        // Re-opening forces a fresh open.
        let _ = cache
            .get_or_open(book, PathBuf::from("a"), opener(2, calls.clone()))
            .unwrap();
        assert_eq!(calls.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn idle_sweep_keeps_recently_used_entries() {
        let idle = Duration::from_secs(10);
        let (cache, clock) = cache(4, idle);
        let book = Uuid::new_v4();
        let calls = Arc::new(AtomicUsize::new(0));

        let _ = cache
            .get_or_open(book, PathBuf::from("a"), opener(1, calls.clone()))
            .unwrap();
        clock.advance(Duration::from_secs(5));
        // Touch the entry, last_used resets.
        let _ = cache
            .get_or_open(book, PathBuf::from("a"), opener(99, calls.clone()))
            .unwrap();
        clock.advance(Duration::from_secs(6));

        let evicted = cache.sweep_idle();
        assert_eq!(evicted, 0, "should still be within idle window after touch");
        assert_eq!(cache.snapshot().current_size, 1);
    }

    #[test]
    fn evict_removes_single_book() {
        let (cache, _clock) = cache(4, Duration::from_secs(60));
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        let calls = Arc::new(AtomicUsize::new(0));

        let _ = cache
            .get_or_open(a, PathBuf::from("a"), opener(1, calls.clone()))
            .unwrap();
        let _ = cache
            .get_or_open(b, PathBuf::from("b"), opener(2, calls.clone()))
            .unwrap();

        assert!(cache.evict(a));
        assert!(!cache.evict(a), "second evict is a no-op");
        let snap = cache.snapshot();
        assert_eq!(snap.current_size, 1);
        assert_eq!(snap.entries[0].book_id, b);
    }

    #[test]
    fn clear_empties_cache() {
        let (cache, _clock) = cache(4, Duration::from_secs(60));
        let calls = Arc::new(AtomicUsize::new(0));
        for _ in 0..3 {
            let id = Uuid::new_v4();
            let _ = cache
                .get_or_open(id, PathBuf::from("f"), opener(1, calls.clone()))
                .unwrap();
        }
        assert_eq!(cache.snapshot().current_size, 3);
        let removed = cache.clear();
        assert_eq!(removed, 3);
        assert_eq!(cache.snapshot().current_size, 0);
    }

    #[test]
    fn disabled_cache_never_stores() {
        let clock = TestClock::new();
        let cache = PdfHandleCache::<TestDoc>::with_clock(
            4,
            Duration::from_secs(60),
            false,
            Arc::new(clock),
        );
        let book = Uuid::new_v4();
        let calls = Arc::new(AtomicUsize::new(0));

        let _ = cache
            .get_or_open(book, PathBuf::from("a"), opener(1, calls.clone()))
            .unwrap();
        let _ = cache
            .get_or_open(book, PathBuf::from("a"), opener(2, calls.clone()))
            .unwrap();

        assert_eq!(
            calls.load(Ordering::Relaxed),
            2,
            "opener runs every call when disabled"
        );
        assert_eq!(cache.snapshot().current_size, 0);
        assert_eq!(cache.stats().opens(), 2);
        assert_eq!(cache.stats().hits(), 0);
        assert_eq!(cache.stats().misses(), 0);
    }

    #[tokio::test]
    async fn concurrent_get_or_open_returns_same_arc() {
        let cache = Arc::new(PdfHandleCache::<TestDoc>::new(
            4,
            Duration::from_secs(60),
            true,
        ));
        let book = Uuid::new_v4();
        let calls = Arc::new(AtomicUsize::new(0));

        // Fire several concurrent get_or_open calls for the same book on
        // blocking threads, since the cache API is sync.
        let mut handles = Vec::new();
        for _ in 0..8 {
            let cache = cache.clone();
            let calls = calls.clone();
            handles.push(tokio::task::spawn_blocking(move || {
                cache
                    .get_or_open(book, PathBuf::from("a"), opener(1, calls))
                    .unwrap()
            }));
        }
        let mut arcs = Vec::new();
        for h in handles {
            arcs.push(h.await.unwrap());
        }

        // Every returned Arc must point at the same underlying allocation.
        for window in arcs.windows(2) {
            assert!(Arc::ptr_eq(&window[0], &window[1]));
        }
        // Opens may exceed 1 under a race (acceptable), but the final cache
        // state must hold exactly one entry for the book.
        assert_eq!(cache.snapshot().current_size, 1);
    }

    #[test]
    fn opener_error_propagates_without_caching() {
        let (cache, _clock) = cache(4, Duration::from_secs(60));
        let book = Uuid::new_v4();
        let result = cache.get_or_open(book, PathBuf::from("a"), || -> Result<TestDoc> {
            Err(anyhow::anyhow!("boom"))
        });
        assert!(result.is_err());
        assert_eq!(cache.snapshot().current_size, 0);
        assert_eq!(cache.stats().misses(), 1);
        assert_eq!(cache.stats().opens(), 0);
    }
}
