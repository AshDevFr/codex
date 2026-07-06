pub mod db_batch;
pub mod docs;
pub mod error;
pub mod extractors;
pub mod image_limit;
pub mod middleware;
pub mod observability;
pub mod permissions;
pub mod routes;
pub mod web;

#[allow(unused_imports)]
pub use docs::ApiDoc;
pub use extractors::AppState;
pub use routes::create_router;
