//! Authentication routes
//!
//! Handles user authentication including login, registration, logout, and email verification.

use super::super::handlers;
use crate::api::extractors::AppState;
use axum::{routing::post, Router};
use std::sync::Arc;

/// Create authentication routes
///
/// All routes are public (no authentication required).
///
/// Routes:
/// - POST /login - Authenticate user and return JWT token
/// - POST /register - Register a new user account
/// - POST /logout - Invalidate current session
/// - POST /verify-email - Verify email address with token
/// - POST /resend-verification - Request a new verification email
pub fn routes(_state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        .route("/login", post(handlers::login))
        .route("/register", post(handlers::register))
        .route("/logout", post(handlers::logout))
        .route("/verify-email", post(handlers::verify_email))
        .route("/resend-verification", post(handlers::resend_verification))
}
