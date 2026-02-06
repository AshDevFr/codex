// Core entities
#[allow(unused_imports)]
pub use super::book_covers::Entity as BookCovers;
pub use super::book_duplicates::Entity as BookDuplicates;
pub use super::book_error::{BookError, BookErrorType, BookErrors};
#[allow(unused_imports)]
pub use super::book_external_ids::Entity as BookExternalIds;
#[allow(unused_imports)]
pub use super::book_external_links::Entity as BookExternalLinks;
pub use super::book_metadata::Entity as BookMetadata;
pub use super::books::Entity as Books;
pub use super::libraries::Entity as Libraries;
pub use super::pages::Entity as Pages;
pub use super::series::Entity as Series;
pub use super::task_metrics::Entity as TaskMetrics;
pub use super::tasks::Entity as Tasks;
pub use super::users::Entity as Users;

// OIDC authentication
#[allow(unused_imports)]
pub use super::oidc_connections::Entity as OidcConnections;

// Plugin entities (exported for external use, may not be used internally)
#[allow(unused_imports)]
pub use super::plugin_failures::Entity as PluginFailures;
#[allow(unused_imports)]
pub use super::plugins::Entity as Plugins;

// Series metadata enhancement entities
#[allow(unused_imports)]
pub use super::series_external_ids::Entity as SeriesExternalIds;
pub use super::series_metadata::Entity as SeriesMetadata;

// Sharing tags for content access control (WIP feature)
#[allow(unused_imports)]
pub use super::series_sharing_tags::Entity as SeriesSharingTags;
#[allow(unused_imports)]
pub use super::sharing_tags::Entity as SharingTags;
#[allow(unused_imports)]
pub use super::user_sharing_tags::Entity as UserSharingTags;
