// Database integration tests
//
// Tests repositories, authentication utilities, and database operations.

mod db {
    mod auth;
    mod book_duplicates;
    mod migrations;
    mod postgres;
    mod repositories;
}
