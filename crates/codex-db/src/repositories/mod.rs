pub mod alternate_title;
pub mod api_key;
pub mod book;
pub mod book_covers;
pub mod book_duplicates;
pub mod book_external_id;
pub mod book_external_links;
pub mod collection;
pub mod email_verification_token;
pub mod external_link;
pub mod external_rating;
pub mod filter_preset;
pub mod genre;
pub mod library;
pub mod library_jobs;
pub mod metadata;
pub mod metrics;
pub mod page;
pub mod plugin_failures;
pub mod plugins;
pub mod read_list;
pub mod read_progress;
pub mod refresh_token;
pub mod release_ledger;
pub mod release_sources;
pub mod scheduled_firing;
pub mod series;
pub mod series_aliases;
pub mod series_covers;
pub mod series_duplicates;
pub mod series_export;
pub mod series_external_id;
pub mod series_metadata;
pub mod series_tracking;
pub mod settings;
pub mod tag;
pub mod task;
pub mod task_metrics;
pub mod user;
pub mod user_preferences;
pub mod user_series_rating;
pub mod want_to_read;

// Sharing tags for content access control
pub mod sharing_tag;

// Series-level visibility filter (driven by sharing tags / access groups)
pub mod visibility;

// Access groups: reusable bundles of sharing-tag grants assignable to users
pub mod access_group;

// OIDC authentication
pub mod oidc_connection;

// System-scoped plugin KV store (per-plugin, no user context)
pub mod plugin_data;

// User plugin system
pub mod user_plugin_data;
pub mod user_plugins;

// Re-export repositories
pub use alternate_title::AlternateTitleRepository;
pub use api_key::ApiKeyRepository;
pub use book::{
    BookQueryOptions, BookQuerySort, BookRepository, BookSortField, ReadStatusFilter,
    ReleaseDateFilter, ReleaseDateOperator,
};
pub use book_covers::BookCoversRepository;
pub use book_duplicates::BookDuplicatesRepository;
pub use book_external_id::BookExternalIdRepository;
pub use book_external_links::BookExternalLinkRepository;
pub use collection::CollectionRepository;
pub use email_verification_token::EmailVerificationTokenRepository;
pub use external_link::ExternalLinkRepository;
pub use external_rating::ExternalRatingRepository;
pub use filter_preset::{FilterPresetRepository, ListFilterPresetsQuery, UpdateFilterPreset};
pub use genre::GenreRepository;
pub use library::{CreateLibraryParams, LibraryRepository};
pub use library_jobs::{CreateLibraryJobParams, LibraryJobRepository, RecordRunStatus};
pub use metadata::BookMetadataRepository;
pub use metrics::MetricsRepository;
pub use page::PageRepository;
pub use plugin_failures::{FailureContext, PluginFailuresRepository};
pub use plugins::PluginsRepository;
pub use read_list::ReadListRepository;
pub use read_progress::ReadProgressRepository;
#[allow(unused_imports)]
pub use refresh_token::{NewRefreshToken, RefreshTokenRepository};
#[allow(unused_imports)]
pub use release_ledger::{
    InboxSort, LedgerInboxFilter, NewReleaseEntry, RecordOutcome, ReleaseLedgerRepository,
};
#[allow(unused_imports)]
pub use release_sources::{NewReleaseSource, ReleaseSourceRepository, ReleaseSourceUpdate};
pub use scheduled_firing::ScheduledFiringRepository;
pub use series::{SeriesQueryOptions, SeriesQuerySort, SeriesRepository, SeriesSortFieldRepo};
#[allow(unused_imports)]
pub use series_aliases::SeriesAliasRepository;
pub use series_covers::SeriesCoversRepository;
#[allow(unused_imports)]
pub use series_duplicates::SeriesDuplicatesRepository;
pub use series_export::SeriesExportRepository;
pub use series_external_id::SeriesExternalIdRepository;
pub use series_metadata::SeriesMetadataRepository;
#[allow(unused_imports)]
pub use series_tracking::{SeriesTrackingRepository, TrackingUpdate};
pub use settings::SettingsRepository;
pub use tag::TagRepository;
pub use task::TaskRepository;
pub use user::{UserListFilter, UserRepository};
pub use user_preferences::UserPreferencesRepository;
pub use user_series_rating::UserSeriesRatingRepository;
pub use want_to_read::WantToReadRepository;

// Sharing tags
pub use sharing_tag::SharingTagRepository;

// Visibility filter helpers
pub use visibility::{
    SeriesVisibility, apply_book_visibility, apply_series_visibility, visibility_predicate,
};

// Access groups
pub use access_group::AccessGroupRepository;

// OIDC authentication
pub use oidc_connection::OidcConnectionRepository;

// System-scoped plugin KV store
#[allow(unused_imports)]
pub use plugin_data::PluginDataRepository;

// User plugin system
#[allow(unused_imports)]
pub use user_plugin_data::UserPluginDataRepository;
pub use user_plugins::UserPluginsRepository;
