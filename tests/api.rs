// API endpoint integration tests
//
// Tests the HTTP layer including authentication, authorization,
// request/response handling, and error responses.

mod api {
    mod alternate_titles;
    mod analyze;
    mod api_keys;
    mod auth;
    mod books;
    mod bulk_metadata;
    mod bulk_operations;
    mod covers;
    mod current_user;
    mod duplicates;
    mod external_links;
    mod external_ratings;
    mod filesystem;
    mod genres;
    mod info;
    mod komga;
    mod koreader;
    mod libraries;
    mod metadata_locks;
    mod metadata_reset;
    mod metrics;
    mod oidc;
    mod opds;
    mod opds2;
    mod pages;
    mod pdf_cache;
    mod plugin_metrics;
    mod plugins;
    mod rate_limit;
    mod read_progress;
    mod recommendations;
    mod renumber;
    mod scan;
    mod series;
    mod series_external_ids;
    mod settings;
    mod setup;
    mod sse;
    mod tags;
    mod task_metrics;
    mod thumbnails;
    mod user_plugins;
    mod user_preferences;
    mod user_ratings;
}
