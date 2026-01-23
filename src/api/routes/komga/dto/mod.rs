//! Komga-compatible Data Transfer Objects
//!
//! This module contains all DTOs for Komga API compatibility.
//! These structures match the exact format Komic and other Komga-compatible apps expect.

pub mod book;
pub mod library;
pub mod page;
pub mod pagination;
pub mod series;
pub mod stubs;
pub mod user;

// Re-export commonly used types for the public Komga-compatible API.
// These may not all be used internally but are part of the API contract.
#[allow(unused_imports)]
pub use book::{
    KomgaBookDto, KomgaBookLinkDto, KomgaBookMetadataDto, KomgaBooksSearchRequestDto,
    KomgaMediaDto, KomgaReadProgressDto, KomgaReadProgressUpdateDto,
};
#[allow(unused_imports)]
pub use library::KomgaLibraryDto;
#[allow(unused_imports)]
pub use page::KomgaPageDto;
#[allow(unused_imports)]
pub use pagination::{KomgaPage, KomgaPageable, KomgaSort};
#[allow(unused_imports)]
pub use series::{
    KomgaAlternateTitleDto, KomgaAuthorDto, KomgaBooksMetadataAggregationDto, KomgaSeriesDto,
    KomgaSeriesMetadataDto, KomgaWebLinkDto,
};
#[allow(unused_imports)]
pub use user::{KomgaAgeRestrictionDto, KomgaContentRestrictionsDto, KomgaUserDto};

// Re-export utility functions
#[allow(unused_imports)]
pub use book::format_file_size;
#[allow(unused_imports)]
pub use series::{codex_to_komga_reading_direction, codex_to_komga_status};
#[allow(unused_imports)]
pub use stubs::{KomgaCollectionDto, KomgaReadListDto, StubPaginationQuery};
