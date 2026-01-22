//! Komga-compatible API handlers
//!
//! This module contains request handlers for the Komga-compatible API.
//! Handlers are implemented in separate modules by resource type.

pub mod books;
pub mod libraries;
pub mod pages;
pub mod read_progress;
pub mod series;
pub mod users;

// Re-export handlers for convenience
pub use books::{
    download_book_file, get_book, get_book_thumbnail, get_books_ondeck, get_next_book,
    get_previous_book, search_books,
};
pub use libraries::{get_library, get_library_thumbnail, list_libraries};
pub use pages::{get_page, get_page_thumbnail, list_pages};
pub use read_progress::{delete_progress, update_progress};
pub use series::{
    get_series, get_series_books, get_series_new, get_series_thumbnail, get_series_updated,
    list_series,
};
pub use users::get_current_user;

// Re-export utoipa path macros for OpenAPI documentation
#[doc(hidden)]
pub use books::{
    __path_download_book_file, __path_get_book, __path_get_book_thumbnail, __path_get_books_ondeck,
    __path_get_next_book, __path_get_previous_book, __path_search_books,
};
#[doc(hidden)]
pub use libraries::{__path_get_library, __path_get_library_thumbnail, __path_list_libraries};
#[doc(hidden)]
pub use pages::{__path_get_page, __path_get_page_thumbnail, __path_list_pages};
#[doc(hidden)]
pub use read_progress::{__path_delete_progress, __path_update_progress};
#[doc(hidden)]
pub use series::{
    __path_get_series, __path_get_series_books, __path_get_series_new, __path_get_series_thumbnail,
    __path_get_series_updated, __path_list_series,
};
#[doc(hidden)]
pub use users::__path_get_current_user;
