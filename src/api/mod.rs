pub mod docs;
pub mod dto;
pub mod error;
pub mod extractors;
pub mod handlers;
pub mod middleware;
pub mod permissions;
pub mod routes;

pub use docs::ApiDoc;
pub use dto::*;
pub use error::ApiError;
pub use extractors::{AppState, AuthContext, AuthMethod, AuthState};
pub use permissions::Permission;
pub use routes::create_router;
