//! In-memory fuzzy search index over series and books.
//!
//! Design notes
//! - One global index, not per-library. Permission filtering happens after
//!   ranking via the existing `ContentFilter`; sharding by library would
//!   duplicate work for the common "search all libraries" case.
//! - Scoring uses `nucleo-matcher` directly. The dataset is small enough
//!   (a few thousand series, low-tens-of-thousands of books) that scoring
//!   in one pass per query is well below interactive latency.
//! - The matcher is `!Sync` because it owns a ~135KB scratch buffer; it is
//!   wrapped in a `Mutex`. Per-query contention is microseconds.
//! - All haystacks are pre-normalized via `utils::search::normalize_for_search`
//!   (NFD, strip Latin combining marks, lowercase) so accent- and case-
//!   insensitive matching works without re-normalizing on every query.
//!
//! Phase 1 exposed build + query. Phase 2 wires in event-driven updates via
//! the `listener` module: a Tokio task subscribes to the global
//! [`crate::events::EventBroadcaster`] and translates each entity event into
//! a single-row upsert or remove against the index.

pub mod builder;
pub mod index;
pub mod listener;

pub use index::FuzzyIndex;
// Re-exported for downstream phases (event listener, handler integration).
#[allow(unused_imports)]
pub use index::{BookEntry, BookSources, SeriesEntry, SeriesSources};
pub use listener::spawn_listener;
