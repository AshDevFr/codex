//! Release-tracking services.
//!
//! Hosts core-side logic for the release-source plugin pipeline:
//!
//! - [`candidate`] — wire-format `ReleaseCandidate` and parsing helpers.
//! - [`matcher`] — confidence-threshold gate and dedup-on-record orchestration.
//! - [`backoff`] — per-host backoff state for rate-limit (429) and
//!   unavailability (503) signals, shared across plugins that hit the
//!   same domain.
//! - [`schedule`] — interval resolution and jitter for the polling
//!   scheduler.
//!
//! Plugins emit candidates over the reverse-RPC channel; the matcher applies
//! the threshold and hands the survivors to the ledger repository, which is
//! itself idempotent on the natural dedup keys.

pub mod backoff;
pub mod candidate;
pub mod matcher;
pub mod schedule;
