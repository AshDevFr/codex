//! Repository instrumentation helpers.
//!
//! Codex's repositories sit on top of SeaORM, which does not ship a built-in
//! tracing layer. Phase 2 of the OTLP plan instruments repository methods at
//! the method boundary instead of wrapping raw SQL, so a single SeaORM call
//! shows up as one span tagged with the operation (`select`, `insert`,
//! `update`, `delete`) and a stable entity name (`book`, `series`, ...).
//!
//! Span names follow `db.<entity>.<operation>`. Each span carries the
//! [OpenTelemetry semantic-convention] attributes the `tracing-opentelemetry`
//! bridge recognises:
//!
//! - `db.system`: `"sqlite"` or `"postgresql"`
//! - `db.operation`: `"select" | "insert" | "update" | "delete" | ...`
//! - `otel.kind`: `"client"` (DB calls are client RPCs from our point of view)
//!
//! Entity-identifying values (`book.id`, `series.id`, ...) go in attributes,
//! never in the span name. This keeps span cardinality bounded by the number
//! of repository methods, which is small.
//!
//! [OpenTelemetry semantic-convention]: https://opentelemetry.io/docs/specs/semconv/database/

use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend};

/// Map a SeaORM backend to the OpenTelemetry `db.system` attribute value.
///
/// The result is one of the standard `db.system` constants and is `'static`
/// so it can be embedded directly in span fields without allocation.
pub fn db_system_str(db: &DatabaseConnection) -> &'static str {
    match db.get_database_backend() {
        DbBackend::Sqlite => "sqlite",
        DbBackend::Postgres => "postgresql",
        DbBackend::MySql => "mysql",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::{Database, DatabaseConnection};
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};
    use tracing::field::{Field, Visit};
    use tracing_subscriber::Layer;
    use tracing_subscriber::layer::{Context, SubscriberExt};

    async fn in_memory_sqlite() -> DatabaseConnection {
        Database::connect("sqlite::memory:")
            .await
            .expect("connect to in-memory sqlite")
    }

    #[tokio::test]
    async fn sqlite_backend_maps_to_db_system_sqlite() {
        let db = in_memory_sqlite().await;
        assert_eq!(db_system_str(&db), "sqlite");
    }

    /// Span metadata captured by [`CapturingLayer`] for assertions in tests.
    #[derive(Debug, Default)]
    struct CapturedSpan {
        name: &'static str,
        fields: HashMap<String, String>,
    }

    /// Tracing layer that records every span it sees so tests can assert on
    /// span names and field values without a full OTel SDK.
    struct CapturingLayer {
        captured: Arc<Mutex<Vec<CapturedSpan>>>,
    }

    impl CapturingLayer {
        fn new() -> (Self, Arc<Mutex<Vec<CapturedSpan>>>) {
            let captured = Arc::new(Mutex::new(Vec::new()));
            (
                Self {
                    captured: captured.clone(),
                },
                captured,
            )
        }
    }

    struct FieldVisitor<'a>(&'a mut HashMap<String, String>);

    impl Visit for FieldVisitor<'_> {
        fn record_str(&mut self, field: &Field, value: &str) {
            self.0.insert(field.name().to_string(), value.to_string());
        }
        fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
            self.0
                .insert(field.name().to_string(), format!("{value:?}"));
        }
        fn record_i64(&mut self, field: &Field, value: i64) {
            self.0.insert(field.name().to_string(), value.to_string());
        }
        fn record_u64(&mut self, field: &Field, value: u64) {
            self.0.insert(field.name().to_string(), value.to_string());
        }
        fn record_bool(&mut self, field: &Field, value: bool) {
            self.0.insert(field.name().to_string(), value.to_string());
        }
    }

    impl<S: tracing::Subscriber + for<'a> tracing_subscriber::registry::LookupSpan<'a>> Layer<S>
        for CapturingLayer
    {
        fn on_new_span(
            &self,
            attrs: &tracing::span::Attributes<'_>,
            _id: &tracing::span::Id,
            _ctx: Context<'_, S>,
        ) {
            let mut fields = HashMap::new();
            attrs.record(&mut FieldVisitor(&mut fields));
            self.captured.lock().unwrap().push(CapturedSpan {
                name: attrs.metadata().name(),
                fields,
            });
        }
    }

    /// Demonstrates that a `#[tracing::instrument]`-decorated repository
    /// method emits a span with the expected name and OTel semantic-convention
    /// attributes. This is the shape Phase 2 contracts: callers can rely on
    /// the `db.<entity>.<operation>` naming and the `db.system`,
    /// `db.operation`, `otel.kind` fields being populated.
    #[tokio::test]
    async fn instrumented_repo_method_emits_named_span_with_semantic_conv_fields() {
        use crate::db::repositories::UserRepository;
        use uuid::Uuid;

        let db = in_memory_sqlite().await;
        let (layer, captured) = CapturingLayer::new();
        let subscriber = tracing_subscriber::registry().with(layer);

        let _guard = tracing::subscriber::set_default(subscriber);

        // The lookup will fail (no users table), which is fine: we only care
        // that the instrumented function created the expected span.
        let _ = UserRepository::get_by_id(&db, Uuid::nil()).await;

        let spans = captured.lock().unwrap();
        let span = spans
            .iter()
            .find(|s| s.name == "db.user.get_by_id")
            .expect("db.user.get_by_id span should be emitted");
        assert_eq!(
            span.fields.get("db.system").map(String::as_str),
            Some("sqlite")
        );
        assert_eq!(
            span.fields.get("db.operation").map(String::as_str),
            Some("select")
        );
        assert_eq!(
            span.fields.get("otel.kind").map(String::as_str),
            Some("client")
        );
    }
}
