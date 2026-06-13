// Core entities
#[allow(unused_imports)]
pub use super::book_covers::Entity as BookCovers;
pub use super::book_duplicates::Entity as BookDuplicates;
pub use super::book_error::{BookError, BookErrorType, BookErrors};
#[allow(unused_imports)]
pub use super::book_external_ids::Entity as BookExternalIds;
#[allow(unused_imports)]
pub use super::book_external_links::Entity as BookExternalLinks;
#[allow(unused_imports)]
pub use super::book_genres::Entity as BookGenres;
pub use super::book_metadata::Entity as BookMetadata;
#[allow(unused_imports)]
pub use super::book_tags::Entity as BookTags;
pub use super::books::Entity as Books;
pub use super::libraries::Entity as Libraries;
pub use super::library_jobs::Entity as LibraryJobs;
pub use super::pages::Entity as Pages;
#[allow(unused_imports)]
pub use super::refresh_tokens::Entity as RefreshTokens;
pub use super::scheduled_firing_claims::Entity as ScheduledFiringClaims;
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
pub use super::series_duplicates::Entity as SeriesDuplicates;
#[allow(unused_imports)]
pub use super::series_external_ids::Entity as SeriesExternalIds;
pub use super::series_metadata::Entity as SeriesMetadata;

// System-scoped plugin KV store
#[allow(unused_imports)]
pub use super::plugin_data::Entity as PluginData;
// User plugin system
#[allow(unused_imports)]
pub use super::user_plugin_data::Entity as UserPluginData;
#[allow(unused_imports)]
pub use super::user_plugins::Entity as UserPlugins;

// Series exports
#[allow(unused_imports)]
pub use super::series_exports::Entity as SeriesExports;

// Filter presets
#[allow(unused_imports)]
pub use super::filter_presets::Entity as FilterPresets;

// Sharing tags for content access control
#[allow(unused_imports)]
pub use super::series_sharing_tags::Entity as SeriesSharingTags;
#[allow(unused_imports)]
pub use super::sharing_tags::Entity as SharingTags;
#[allow(unused_imports)]
pub use super::user_sharing_tags::Entity as UserSharingTags;

// Access groups
#[allow(unused_imports)]
pub use super::access_group_oidc_mappings::Entity as AccessGroupOidcMappings;
#[allow(unused_imports)]
pub use super::access_group_sharing_tags::Entity as AccessGroupSharingTags;
#[allow(unused_imports)]
pub use super::access_groups::Entity as AccessGroups;
#[allow(unused_imports)]
pub use super::user_access_groups::Entity as UserAccessGroups;

// Collections, read lists, and the per-user want-to-read queue
#[allow(unused_imports)]
pub use super::collection_series::Entity as CollectionSeries;
#[allow(unused_imports)]
pub use super::collections::Entity as Collections;
#[allow(unused_imports)]
pub use super::read_list_books::Entity as ReadListBooks;
#[allow(unused_imports)]
pub use super::read_lists::Entity as ReadLists;
#[allow(unused_imports)]
pub use super::want_to_read::Entity as WantToRead;
