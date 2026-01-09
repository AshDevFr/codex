// API endpoint integration tests
//
// Tests the HTTP layer including authentication, authorization,
// request/response handling, and error responses.

mod api {
    mod analyze;
    mod api_keys;
    mod auth;
    mod books;
    mod duplicates;
    mod filesystem;
    mod libraries;
    mod metrics;
    mod opds;
    mod scan;
    mod series;
    mod settings;
    mod setup;
    mod sse;
}
