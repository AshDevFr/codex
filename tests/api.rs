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
    mod covers;
    mod duplicates;
    mod external_links;
    mod external_ratings;
    mod filesystem;
    mod genres;
    mod libraries;
    mod metadata_locks;
    mod metrics;
    mod opds;
    mod opds2;
    mod read_progress;
    mod scan;
    mod series;
    mod settings;
    mod setup;
    mod sse;
    mod system_integrations;
    mod tags;
    mod task_metrics;
    mod thumbnails;
    mod user_integrations;
    mod user_preferences;
    mod user_ratings;
}
