// API endpoint integration tests
//
// Tests the HTTP layer including authentication, authorization,
// request/response handling, and error responses.

mod api {
    mod auth;
    mod libraries;
    mod metrics;
    mod opds;
    mod scan;
    mod series;
    mod tasks;
}
