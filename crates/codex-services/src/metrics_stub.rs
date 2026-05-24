//! No-op stubs for the metrics helpers when the `observability` feature is
//! disabled. The shapes mirror `metrics.rs` so call sites stay cfg-free.
//!
//! The metric-name constants below are not referenced when the feature is
//! off, but are kept so the public surface of `observability::metrics`
//! stays identical across feature configurations.

#![allow(dead_code)]

use std::sync::OnceLock;
use std::sync::atomic::AtomicI64;

pub const PLUGIN_REQUESTS: &str = "codex.plugin.requests";
pub const PLUGIN_DURATION: &str = "codex.plugin.duration_ms";
pub const PLUGIN_RATE_LIMIT_REJECTIONS: &str = "codex.plugin.rate_limit_rejections";
pub const TASK_COMPLETIONS: &str = "codex.task.completions";
pub const TASK_DURATION: &str = "codex.task.duration_ms";
pub const TASK_QUEUE_WAIT: &str = "codex.task.queue_wait_ms";
pub const TASK_IN_FLIGHT: &str = "codex.task.in_flight";
pub const INVENTORY_LIBRARIES: &str = "codex.inventory.libraries";
pub const INVENTORY_SERIES: &str = "codex.inventory.series";
pub const INVENTORY_BOOKS: &str = "codex.inventory.books";
pub const INVENTORY_USERS: &str = "codex.inventory.users";
pub const INVENTORY_PAGES: &str = "codex.inventory.pages";

pub fn record_plugin_request(_plugin_id: &str, _method: &str, _outcome: &str, _duration_ms: u64) {}

pub fn record_plugin_rate_limit_rejection(_plugin_id: &str) {}

pub fn record_task_completion(
    _task_type: &str,
    _outcome: &str,
    _duration_ms: i64,
    _queue_wait_ms: i64,
) {
}

pub fn task_in_flight_inc() {}

pub fn task_in_flight_dec() {}

pub fn record_http_request(_method: &str, _route: &str, _status: u16, _duration_secs: f64) {}

pub fn install_runtime_observers() {}

#[derive(Default)]
pub struct InventorySnapshot {
    pub libraries: AtomicI64,
    pub series: AtomicI64,
    pub books: AtomicI64,
    pub users: AtomicI64,
    pub pages: AtomicI64,
}

static INVENTORY_SNAPSHOT: OnceLock<&'static InventorySnapshot> = OnceLock::new();

pub fn inventory_snapshot() -> &'static InventorySnapshot {
    INVENTORY_SNAPSHOT.get_or_init(|| Box::leak(Box::new(InventorySnapshot::default())))
}

pub fn update_inventory_snapshot(libraries: i64, series: i64, books: i64, users: i64, pages: i64) {
    use std::sync::atomic::Ordering;
    let snap = inventory_snapshot();
    snap.libraries.store(libraries, Ordering::Relaxed);
    snap.series.store(series, Ordering::Relaxed);
    snap.books.store(books, Ordering::Relaxed);
    snap.users.store(users, Ordering::Relaxed);
    snap.pages.store(pages, Ordering::Relaxed);
}
