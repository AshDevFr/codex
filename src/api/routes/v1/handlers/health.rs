use axum::{extract::State, http::StatusCode, response::IntoResponse};
use sea_orm::{ConnectionTrait, DatabaseConnection, Statement};
use tracing::{info, warn};

/// Health check endpoint - checks database connectivity
///
/// Returns "OK" with 200 status if database is healthy,
/// or "Service Unavailable" with 503 status if database check fails.
#[utoipa::path(
    get,
    path = "/health",
    responses(
        (status = 200, description = "Service is healthy"),
        (status = 503, description = "Service is unavailable"),
    ),
    tag = "Health"
)]
pub async fn health_check(State(db): State<DatabaseConnection>) -> impl IntoResponse {
    // Check database health with a simple query
    let result = db
        .execute(Statement::from_string(
            db.get_database_backend(),
            "SELECT 1".to_string(),
        ))
        .await;

    match result {
        Ok(_) => {
            info!("Health check: database OK");
            (StatusCode::OK, "OK")
        }
        Err(e) => {
            warn!("Health check: database error: {}", e);
            (StatusCode::SERVICE_UNAVAILABLE, "Service Unavailable")
        }
    }
}
