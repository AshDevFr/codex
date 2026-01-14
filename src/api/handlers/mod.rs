pub mod api_keys;
pub mod auth;
pub mod books;
pub mod duplicates;
pub mod events;
pub mod filesystem;
pub mod health;
pub mod libraries;
pub mod metrics;
pub mod opds;
pub mod opds2;
pub mod pages;
pub mod read_progress;
pub mod scan;
pub mod series;
pub mod settings;
pub mod setup;
pub mod system_integrations;
pub mod task_metrics;
pub mod task_queue;
pub mod user_integrations;
pub mod user_preferences;
pub mod users;

pub use auth::*;
pub use books::*;
pub use duplicates::*;
pub use events::*;
pub use filesystem::*;
pub use health::*;
pub use libraries::*;
pub use metrics::*;
// opds2 not glob re-exported to avoid name conflicts with opds (catalog, routes, search)
// Use handlers::opds2::* directly when needed
pub use pages::*;
pub use read_progress::*;
pub use scan::*;
pub use series::*;
pub use users::*;

// Re-export AppState for convenience
pub use crate::api::extractors::AppState;
