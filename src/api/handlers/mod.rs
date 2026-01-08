pub mod api_keys;
pub mod auth;
pub mod books;
pub mod events;
pub mod filesystem;
pub mod health;
pub mod libraries;
pub mod metrics;
pub mod opds;
pub mod pages;
pub mod read_progress;
pub mod scan;
pub mod series;
pub mod settings;
pub mod setup;
pub mod task_queue;
pub mod users;

pub use api_keys::*;
pub use auth::*;
pub use books::*;
pub use events::*;
pub use filesystem::*;
pub use health::*;
pub use libraries::*;
pub use metrics::*;
pub use opds::*;
pub use pages::*;
pub use read_progress::*;
pub use scan::*;
pub use series::*;
pub use setup::*;
pub use users::*;

// Re-export AppState for convenience
pub use crate::api::extractors::AppState;
