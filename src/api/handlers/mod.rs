pub mod auth;
pub mod books;
pub mod health;
pub mod libraries;
pub mod pages;
pub mod scan;
pub mod series;
pub mod users;

pub use auth::*;
pub use books::*;
pub use health::*;
pub use libraries::*;
pub use pages::*;
pub use scan::*;
pub use series::*;
pub use users::*;

// Re-export AppState for convenience
pub use crate::api::extractors::AppState;
