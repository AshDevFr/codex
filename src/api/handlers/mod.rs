pub mod auth;
pub mod books;
pub mod health;
pub mod libraries;
pub mod metrics;
pub mod opds;
pub mod pages;
pub mod read_progress;
pub mod scan;
pub mod series;
pub mod tasks;
pub mod users;

pub use auth::*;
pub use books::*;
pub use health::*;
pub use libraries::*;
pub use metrics::*;
pub use opds::*;
pub use pages::*;
pub use read_progress::*;
pub use scan::*;
pub use series::*;
pub use tasks::*;
pub use users::*;

// Re-export AppState for convenience
pub use crate::api::extractors::AppState;
