pub mod docs;
pub mod error;
pub mod extractors;
pub mod middleware;
pub mod permissions;
pub mod routes;

pub use docs::ApiDoc;
pub use extractors::AppState;
pub use routes::create_router;
