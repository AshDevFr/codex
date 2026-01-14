pub mod auth;
pub mod client_info;

// AuthMethod is part of the public API for auth context inspection
#[allow(unused_imports)]
pub use auth::{AppState, AuthContext, AuthMethod, AuthState, FlexibleAuthContext};
pub use client_info::ClientInfo;
