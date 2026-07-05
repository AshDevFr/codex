// Single integration-test binary.
//
// All integration test trees live as modules under this binary so that
// `cargo test` only links the codex crate once for the entire suite.
// Adding a new test area means creating tests/<area>/mod.rs and declaring
// it below.

mod api;
mod db;
mod event_bridge;
mod migrate;
mod parsers;
mod scanner;
mod scheduler;
mod services;
mod task_priority_ordering;
mod task_queue;
mod task_queue_api;
mod task_queue_e2e;
mod task_recovery_integration;
mod thumbnail_generation_events;
