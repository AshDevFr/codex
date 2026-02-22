//! OIDC authentication routes
//!
//! Routes for OpenID Connect (OIDC) authentication.
//! These routes enable authentication via external identity providers.

use super::super::handlers::oidc;
use crate::api::extractors::AppState;
use axum::{
    Router,
    routing::{get, post},
};
use std::sync::Arc;

/// Create OIDC authentication routes
///
/// All routes are public (no authentication required).
///
/// Routes:
/// - GET /providers - List available OIDC providers
/// - POST /{provider}/login - Initiate login flow (returns redirect URL)
/// - GET /{provider}/callback - Handle IdP callback (exchanges code, creates session)
pub fn routes(_state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        .route("/providers", get(oidc::list_providers))
        .route("/{provider}/login", post(oidc::login))
        .route("/{provider}/callback", get(oidc::callback))
}
