//! User Plugin Services
//!
//! This module provides services for managing user-level plugin integrations:
//! - OAuth 2.0 authentication flows (authorization, token exchange, CSRF protection)
//! - Token refresh for expiring OAuth tokens
//!
//! User plugins differ from system plugins in that each user has their own
//! credentials and configuration. The services in this module handle the
//! per-user aspects of plugin management.

pub mod oauth;
pub mod token_refresh;

pub use oauth::OAuthStateManager;
