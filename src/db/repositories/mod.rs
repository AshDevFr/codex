pub mod alternate_title;
pub mod api_key;
pub mod book;
pub mod book_duplicates;
pub mod email_verification_token;
pub mod external_link;
pub mod external_rating;
pub mod genre;
pub mod library;
pub mod metadata;
pub mod metrics;
pub mod page;
pub mod plugin_failures;
pub mod plugins;
pub mod read_progress;
pub mod series;
pub mod series_covers;
pub mod series_external_id;
pub mod series_metadata;
pub mod settings;
pub mod tag;
pub mod task;
pub mod task_metrics;
pub mod user;
pub mod user_preferences;
pub mod user_series_rating;

// Sharing tags for content access control
pub mod sharing_tag;

// Re-export repositories
pub use alternate_title::AlternateTitleRepository;
pub use api_key::ApiKeyRepository;
pub use book::{
    BookQueryOptions, BookQuerySort, BookRepository, BookSortField, ReadStatusFilter,
    ReleaseDateFilter, ReleaseDateOperator,
};
pub use book_duplicates::BookDuplicatesRepository;
pub use email_verification_token::EmailVerificationTokenRepository;
pub use external_link::ExternalLinkRepository;
pub use external_rating::ExternalRatingRepository;
pub use genre::GenreRepository;
pub use library::{CreateLibraryParams, LibraryRepository};
pub use metadata::BookMetadataRepository;
pub use metrics::MetricsRepository;
pub use page::PageRepository;
pub use plugin_failures::{FailureContext, PluginFailuresRepository};
pub use plugins::PluginsRepository;
pub use read_progress::ReadProgressRepository;
pub use series::{SeriesQueryOptions, SeriesQuerySort, SeriesRepository, SeriesSortFieldRepo};
pub use series_covers::SeriesCoversRepository;
pub use series_external_id::SeriesExternalIdRepository;
pub use series_metadata::SeriesMetadataRepository;
pub use settings::SettingsRepository;
pub use tag::TagRepository;
pub use task::TaskRepository;
pub use user::{UserListFilter, UserRepository};
pub use user_preferences::UserPreferencesRepository;
pub use user_series_rating::UserSeriesRatingRepository;

// Sharing tags
pub use sharing_tag::SharingTagRepository;
