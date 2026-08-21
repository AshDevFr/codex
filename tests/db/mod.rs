// Database integration tests
//
// Tests repositories, authentication utilities, and database operations.

mod auth;
mod book_duplicates;
mod collections;
mod entity_event_bridge;
mod migrations;
mod oidc_pending_state;
mod postgres;
mod reading_sessions;
mod reading_stats;
mod refresh_token_repository;
mod repositories;
mod series_duplicates;
mod user_plugin_oauth_state;
mod visibility;
