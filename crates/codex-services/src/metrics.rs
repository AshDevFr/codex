//! OpenTelemetry meter instruments and dual-write helpers.
//!
//! Two-consumer model: existing in-process counters keep powering the in-app
//! metrics dashboards; the helpers here emit OTel counters / histograms /
//! gauges at the same call sites so an OTLP backend (SigNoz, Tempo, etc.) can
//! see the data with proper percentile aggregation. Stable instrument names
//! live as `const`s so they're searchable and easy to keep in sync with the
//! operator docs.
//!
//! All entry points are safe to call when observability is disabled: the
//! global meter provider is a no-op until `codex::observability::init`
//! (still in the root binary crate) installs one.

use std::sync::OnceLock;
use std::sync::atomic::{AtomicI64, Ordering};

use opentelemetry::{
    KeyValue, global,
    metrics::{Counter, Histogram, Meter},
};
use opentelemetry_semantic_conventions::{attribute, metric as metric_semconv};

const METER_NAME: &str = "codex";

// ---- Plugin metric names ----
pub const PLUGIN_REQUESTS: &str = "codex.plugin.requests";
pub const PLUGIN_DURATION: &str = "codex.plugin.duration_ms";
pub const PLUGIN_RATE_LIMIT_REJECTIONS: &str = "codex.plugin.rate_limit_rejections";

// ---- Task metric names ----
pub const TASK_COMPLETIONS: &str = "codex.task.completions";
pub const TASK_DURATION: &str = "codex.task.duration_ms";
pub const TASK_QUEUE_WAIT: &str = "codex.task.queue_wait_ms";
pub const TASK_IN_FLIGHT: &str = "codex.task.in_flight";

// ---- Inventory metric names ----
pub const INVENTORY_LIBRARIES: &str = "codex.inventory.libraries";
pub const INVENTORY_SERIES: &str = "codex.inventory.series";
pub const INVENTORY_BOOKS: &str = "codex.inventory.books";
pub const INVENTORY_USERS: &str = "codex.inventory.users";
pub const INVENTORY_PAGES: &str = "codex.inventory.pages";

fn meter() -> &'static Meter {
    static METER: OnceLock<Meter> = OnceLock::new();
    METER.get_or_init(|| global::meter(METER_NAME))
}

// =============================================================================
// Plugin instruments
// =============================================================================

pub struct PluginInstruments {
    requests: Counter<u64>,
    duration_ms: Histogram<f64>,
    rate_limit_rejections: Counter<u64>,
}

impl PluginInstruments {
    /// Build the plugin instrument set from an explicit meter. Tests use this
    /// to point the instruments at an in-memory exporter without going
    /// through the OnceLock-cached global accessor below.
    pub fn new(m: &Meter) -> Self {
        Self {
            requests: m
                .u64_counter(PLUGIN_REQUESTS)
                .with_description("Plugin RPC requests")
                .build(),
            duration_ms: m
                .f64_histogram(PLUGIN_DURATION)
                .with_unit("ms")
                .with_description("Plugin RPC duration")
                .build(),
            rate_limit_rejections: m
                .u64_counter(PLUGIN_RATE_LIMIT_REJECTIONS)
                .with_description("Plugin requests rejected by local rate limiter")
                .build(),
        }
    }

    fn record_request(&self, plugin_id: &str, method: &str, outcome: &str, duration_ms: u64) {
        let attrs = [
            KeyValue::new("plugin_id", plugin_id.to_string()),
            KeyValue::new("method", method.to_string()),
            KeyValue::new("outcome", outcome.to_string()),
        ];
        self.requests.add(1, &attrs);
        self.duration_ms.record(duration_ms as f64, &attrs);
    }

    fn record_rate_limit_rejection(&self, plugin_id: &str) {
        self.rate_limit_rejections
            .add(1, &[KeyValue::new("plugin_id", plugin_id.to_string())]);
    }
}

fn plugin_instruments() -> &'static PluginInstruments {
    static INST: OnceLock<PluginInstruments> = OnceLock::new();
    INST.get_or_init(|| PluginInstruments::new(meter()))
}

/// Record a plugin RPC outcome. `outcome` is one of `success`, `failure`.
pub fn record_plugin_request(plugin_id: &str, method: &str, outcome: &str, duration_ms: u64) {
    plugin_instruments().record_request(plugin_id, method, outcome, duration_ms);
}

/// Record a rate-limit rejection for a plugin (no method dimension; the limit
/// is applied at the plugin level).
pub fn record_plugin_rate_limit_rejection(plugin_id: &str) {
    plugin_instruments().record_rate_limit_rejection(plugin_id);
}

// =============================================================================
// Task instruments
// =============================================================================

pub struct TaskInstruments {
    completions: Counter<u64>,
    duration_ms: Histogram<f64>,
    queue_wait_ms: Histogram<f64>,
}

impl TaskInstruments {
    pub fn new(m: &Meter) -> Self {
        Self {
            completions: m
                .u64_counter(TASK_COMPLETIONS)
                .with_description("Background task completions")
                .build(),
            duration_ms: m
                .f64_histogram(TASK_DURATION)
                .with_unit("ms")
                .with_description("Background task execution duration")
                .build(),
            queue_wait_ms: m
                .f64_histogram(TASK_QUEUE_WAIT)
                .with_unit("ms")
                .with_description("Background task queue wait time")
                .build(),
        }
    }

    fn record_completion(
        &self,
        task_type: &str,
        outcome: &str,
        duration_ms: i64,
        queue_wait_ms: i64,
    ) {
        let attrs = [
            KeyValue::new("task_type", task_type.to_string()),
            KeyValue::new("outcome", outcome.to_string()),
        ];
        self.completions.add(1, &attrs);
        if duration_ms >= 0 {
            self.duration_ms.record(duration_ms as f64, &attrs);
        }
        if queue_wait_ms >= 0 {
            self.queue_wait_ms.record(queue_wait_ms as f64, &attrs);
        }
    }
}

fn task_instruments() -> &'static TaskInstruments {
    static INST: OnceLock<TaskInstruments> = OnceLock::new();
    INST.get_or_init(|| TaskInstruments::new(meter()))
}

/// Currently executing background tasks. Workers increment on claim,
/// decrement on completion/failure; the gauge callback reads this atomic.
static TASKS_IN_FLIGHT: AtomicI64 = AtomicI64::new(0);

/// Record a task completion. `outcome` is one of `success`, `failure`,
/// `rate_limited`.
pub fn record_task_completion(
    task_type: &str,
    outcome: &str,
    duration_ms: i64,
    queue_wait_ms: i64,
) {
    task_instruments().record_completion(task_type, outcome, duration_ms, queue_wait_ms);
}

/// Increment the in-flight tasks counter (call after claiming a task).
pub fn task_in_flight_inc() {
    TASKS_IN_FLIGHT.fetch_add(1, Ordering::Relaxed);
}

/// Decrement the in-flight tasks counter (call after a task completes or
/// fails). Saturates at zero to be safe against double-decrement bugs.
pub fn task_in_flight_dec() {
    let _ = TASKS_IN_FLIGHT.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |v| {
        Some(if v > 0 { v - 1 } else { 0 })
    });
}

/// Register the observable gauge that exposes the in-flight tasks counter.
/// Idempotent on the metric layer; should be called once at startup.
fn install_in_flight_gauge() {
    let _ = meter()
        .i64_observable_gauge(TASK_IN_FLIGHT)
        .with_description("Background tasks currently executing")
        .with_callback(|obs| obs.observe(TASKS_IN_FLIGHT.load(Ordering::Relaxed), &[]))
        .build();
}

// =============================================================================
// HTTP instruments
// =============================================================================

struct HttpInstruments {
    duration_seconds: Histogram<f64>,
}

fn http_instruments() -> &'static HttpInstruments {
    static INST: OnceLock<HttpInstruments> = OnceLock::new();
    INST.get_or_init(|| {
        let m = meter();
        HttpInstruments {
            // Semantic-convention default: `http.server.request.duration` in
            // seconds. Bucketing is left to the SDK's default histogram view.
            duration_seconds: m
                .f64_histogram(metric_semconv::HTTP_SERVER_REQUEST_DURATION)
                .with_unit("s")
                .with_description("Duration of HTTP server requests")
                .build(),
        }
    })
}

/// Record an HTTP server request.
///
/// `route` should be the route template (e.g., `/api/v1/series/:id`), not the
/// resolved URL — otherwise the label cardinality explodes per series ID.
pub fn record_http_request(method: &str, route: &str, status: u16, duration_secs: f64) {
    let attrs = [
        KeyValue::new(attribute::HTTP_REQUEST_METHOD, method.to_string()),
        KeyValue::new(attribute::HTTP_ROUTE, route.to_string()),
        KeyValue::new(attribute::HTTP_RESPONSE_STATUS_CODE, status as i64),
    ];
    http_instruments()
        .duration_seconds
        .record(duration_secs, &attrs);
}

// =============================================================================
// Inventory observable gauges
// =============================================================================

/// Atomic snapshot of inventory counts, kept current by a background poller in
/// `commands::serve`. The OTel observable-gauge callbacks read these atomics
/// synchronously (they run on the SDK collection thread, no async context).
#[derive(Default)]
pub struct InventorySnapshot {
    pub libraries: AtomicI64,
    pub series: AtomicI64,
    pub books: AtomicI64,
    pub users: AtomicI64,
    pub pages: AtomicI64,
}

static INVENTORY_SNAPSHOT: OnceLock<&'static InventorySnapshot> = OnceLock::new();

/// Returns the global inventory snapshot. First call initializes it.
pub fn inventory_snapshot() -> &'static InventorySnapshot {
    INVENTORY_SNAPSHOT.get_or_init(|| Box::leak(Box::new(InventorySnapshot::default())))
}

/// Install every observable instrument the binary owns (inventory gauges,
/// in-flight task gauge, process metrics). Idempotent only insofar as the
/// underlying meter accepts re-registration; intended to be called exactly
/// once at startup, after the meter provider is in place.
pub fn install_runtime_observers() {
    install_inventory_gauges();
    install_in_flight_gauge();
    install_process_metrics();
}

/// Register the inventory observable gauges with the global meter.
///
/// Must be called once after the meter provider is installed. Safe to call
/// when observability is disabled: the no-op meter provider will accept the
/// instrument registrations without doing anything with them.
fn install_inventory_gauges() {
    let snap = inventory_snapshot();
    let m = meter();

    macro_rules! gauge {
        ($name:expr, $field:ident, $desc:expr) => {
            m.i64_observable_gauge($name)
                .with_description($desc)
                .with_callback(move |obs| {
                    obs.observe(snap.$field.load(Ordering::Relaxed), &[]);
                })
                .build()
        };
    }

    let _ = gauge!(INVENTORY_LIBRARIES, libraries, "Number of libraries");
    let _ = gauge!(INVENTORY_SERIES, series, "Number of series");
    let _ = gauge!(INVENTORY_BOOKS, books, "Number of books");
    let _ = gauge!(INVENTORY_USERS, users, "Number of users");
    let _ = gauge!(INVENTORY_PAGES, pages, "Number of pages indexed");
}

/// Update the inventory snapshot with freshly counted values.
pub fn update_inventory_snapshot(libraries: i64, series: i64, books: i64, users: i64, pages: i64) {
    let snap = inventory_snapshot();
    snap.libraries.store(libraries, Ordering::Relaxed);
    snap.series.store(series, Ordering::Relaxed);
    snap.books.store(books, Ordering::Relaxed);
    snap.users.store(users, Ordering::Relaxed);
    snap.pages.store(pages, Ordering::Relaxed);
}

// =============================================================================
// Process / runtime metrics
// =============================================================================

/// Install process-level observable gauges (CPU, memory).
///
/// Uses `sysinfo` polled from a fresh `System` snapshot inside the gauge
/// callback. The callback runs on the SDK collection thread (synchronous).
fn install_process_metrics() {
    use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System};

    let m = meter();
    let pid = match sysinfo::get_current_pid() {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!("Could not resolve current PID for process metrics; skipping: {e}");
            return;
        }
    };

    let attrs: [KeyValue; 1] = [KeyValue::new("process.pid", pid.as_u32() as i64)];

    // Semantic-convention metric names are gated behind the experimental
    // feature flag in `opentelemetry-semantic-conventions` 0.32; use the
    // standard string identifiers directly. These names match
    // `process.cpu.time` and `process.memory.usage` from the OTel spec.
    {
        let attrs = attrs.clone();
        let sys = std::sync::Mutex::new(System::new());
        m.f64_observable_gauge("process.cpu.time")
            .with_unit("s")
            .with_description("Total user + system CPU time consumed by the process")
            .with_callback(move |obs| {
                let Ok(mut s) = sys.lock() else { return };
                s.refresh_processes_specifics(
                    ProcessesToUpdate::Some(&[pid]),
                    true,
                    ProcessRefreshKind::nothing().with_cpu(),
                );
                if let Some(proc) = s.process(pid) {
                    obs.observe(proc.accumulated_cpu_time() as f64 / 1000.0, &attrs);
                }
            })
            .build();
    }

    {
        let attrs = attrs.clone();
        let sys = std::sync::Mutex::new(System::new());
        m.i64_observable_gauge("process.memory.usage")
            .with_unit("By")
            .with_description("Resident memory of the process")
            .with_callback(move |obs| {
                let Ok(mut s) = sys.lock() else { return };
                s.refresh_processes_specifics(
                    ProcessesToUpdate::Some(&[pid]),
                    true,
                    ProcessRefreshKind::nothing().with_memory(),
                );
                if let Some(proc) = s.process(pid) {
                    obs.observe(proc.memory() as i64, &attrs);
                }
            })
            .build();
    }

    {
        let sys = std::sync::Mutex::new(System::new());
        m.i64_observable_gauge("process.memory.virtual")
            .with_unit("By")
            .with_description("Virtual memory size of the process")
            .with_callback(move |obs| {
                let Ok(mut s) = sys.lock() else { return };
                s.refresh_processes_specifics(
                    ProcessesToUpdate::Some(&[pid]),
                    true,
                    ProcessRefreshKind::nothing().with_memory(),
                );
                if let Some(proc) = s.process(pid) {
                    obs.observe(proc.virtual_memory() as i64, &attrs);
                }
            })
            .build();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opentelemetry::metrics::MeterProvider;
    use opentelemetry_sdk::metrics::data::AggregatedMetrics;
    use opentelemetry_sdk::metrics::{InMemoryMetricExporter, PeriodicReader, SdkMeterProvider};

    fn test_provider() -> (InMemoryMetricExporter, SdkMeterProvider) {
        let exporter = InMemoryMetricExporter::default();
        let reader = PeriodicReader::builder(exporter.clone()).build();
        let mp = SdkMeterProvider::builder().with_reader(reader).build();
        (exporter, mp)
    }

    #[test]
    fn metric_names_are_stable() {
        // Sanity-check that the public constants haven't drifted; operators
        // build dashboards against these names, so renames need to be
        // deliberate (and announced in the changelog).
        assert_eq!(PLUGIN_REQUESTS, "codex.plugin.requests");
        assert_eq!(PLUGIN_DURATION, "codex.plugin.duration_ms");
        assert_eq!(TASK_COMPLETIONS, "codex.task.completions");
        assert_eq!(TASK_IN_FLIGHT, "codex.task.in_flight");
        assert_eq!(INVENTORY_LIBRARIES, "codex.inventory.libraries");
    }

    #[test]
    fn helpers_are_safe_with_noop_meter_provider() {
        // The global meter provider is no-op in tests (no `init` call). All
        // entry points should be safe to call: they just route to the no-op
        // instruments.
        record_plugin_request("p1", "search", "success", 12);
        record_plugin_rate_limit_rejection("p1");
        record_task_completion("scan_library", "success", 100, 5);
        task_in_flight_inc();
        task_in_flight_dec();
        record_http_request("GET", "/api/v1/series", 200, 0.014);
        update_inventory_snapshot(1, 2, 3, 4, 5);

        // Snapshot atomics should hold the values we just wrote.
        let s = inventory_snapshot();
        assert_eq!(s.libraries.load(Ordering::Relaxed), 1);
        assert_eq!(s.books.load(Ordering::Relaxed), 3);
    }

    #[test]
    fn plugin_instruments_emit_counter_and_histogram_to_in_memory_exporter() {
        let (exporter, mp) = test_provider();
        let inst = PluginInstruments::new(&mp.meter("test"));

        inst.record_request("plugin-a", "search", "success", 42);
        inst.record_request("plugin-a", "search", "failure", 100);
        inst.record_rate_limit_rejection("plugin-a");

        mp.force_flush().expect("flush");
        let batches = exporter.get_finished_metrics().expect("collected metrics");
        assert!(!batches.is_empty(), "expected at least one ResourceMetrics");

        let mut found_requests = false;
        let mut found_duration = false;
        let mut found_rejections = false;
        for rm in batches {
            for scope in rm.scope_metrics() {
                for metric in scope.metrics() {
                    match metric.name() {
                        PLUGIN_REQUESTS => {
                            // Counter exports as a Sum aggregation.
                            assert!(matches!(
                                metric.data(),
                                AggregatedMetrics::U64(
                                    opentelemetry_sdk::metrics::data::MetricData::Sum(_)
                                )
                            ));
                            found_requests = true;
                        }
                        PLUGIN_DURATION => {
                            assert!(matches!(
                                metric.data(),
                                AggregatedMetrics::F64(
                                    opentelemetry_sdk::metrics::data::MetricData::Histogram(_)
                                )
                            ));
                            found_duration = true;
                        }
                        PLUGIN_RATE_LIMIT_REJECTIONS => {
                            found_rejections = true;
                        }
                        _ => {}
                    }
                }
            }
        }
        assert!(found_requests, "plugin requests counter not exported");
        assert!(found_duration, "plugin duration histogram not exported");
        assert!(found_rejections, "plugin rejections counter not exported");
    }

    #[test]
    fn task_instruments_emit_counter_and_histograms() {
        let (exporter, mp) = test_provider();
        let inst = TaskInstruments::new(&mp.meter("test"));

        inst.record_completion("scan_library", "success", 250, 10);
        inst.record_completion("scan_library", "failure", 1000, 50);
        inst.record_completion("scan_library", "rate_limited", -1, -1);

        mp.force_flush().expect("flush");
        let batches = exporter.get_finished_metrics().expect("collected metrics");
        let names: std::collections::HashSet<String> = batches
            .iter()
            .flat_map(|rm| rm.scope_metrics().flat_map(|s| s.metrics()))
            .map(|m| m.name().to_string())
            .collect();
        assert!(names.contains(TASK_COMPLETIONS), "task completions missing");
        assert!(names.contains(TASK_DURATION), "task duration missing");
        assert!(
            names.contains(TASK_QUEUE_WAIT),
            "task queue wait missing (got {names:?})"
        );
    }

    #[test]
    fn in_flight_saturates_at_zero() {
        // Reset, then test the saturating decrement behavior. We compare
        // against the post-test state so other tests running in parallel
        // don't trip the assertions.
        TASKS_IN_FLIGHT.store(0, Ordering::Relaxed);
        task_in_flight_dec();
        assert_eq!(TASKS_IN_FLIGHT.load(Ordering::Relaxed), 0);
        task_in_flight_inc();
        task_in_flight_inc();
        assert_eq!(TASKS_IN_FLIGHT.load(Ordering::Relaxed), 2);
        task_in_flight_dec();
        assert_eq!(TASKS_IN_FLIGHT.load(Ordering::Relaxed), 1);
    }
}
