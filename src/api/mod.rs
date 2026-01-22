pub mod docs;
pub mod error;
pub mod extractors;
pub mod middleware;
pub mod permissions;
pub mod routes;

#[allow(unused_imports)]
pub use docs::ApiDoc;
pub use extractors::AppState;
pub use routes::create_router;
