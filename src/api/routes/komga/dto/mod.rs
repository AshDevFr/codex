//! Komga-compatible Data Transfer Objects
//!
//! This module contains all DTOs for Komga API compatibility.
//! These structures match the exact format Komic and other Komga-compatible apps expect.

pub mod book;
pub mod library;
pub mod page;
pub mod pagination;
pub mod series;
pub mod user;

// Re-export commonly used types
pub use book::{
    KomgaBookDto, KomgaBookLinkDto, KomgaBookMetadataDto, KomgaBooksSearchRequestDto,
    KomgaMediaDto, KomgaReadProgressDto, KomgaReadProgressUpdateDto,
};
pub use library::KomgaLibraryDto;
pub use page::KomgaPageDto;
pub use pagination::{KomgaPage, KomgaPageable, KomgaSort};
pub use series::{
    KomgaAlternateTitleDto, KomgaAuthorDto, KomgaBooksMetadataAggregationDto, KomgaSeriesDto,
    KomgaSeriesMetadataDto, KomgaWebLinkDto,
};
pub use user::{KomgaAgeRestrictionDto, KomgaContentRestrictionsDto, KomgaUserDto};

// Re-export utility functions
pub use book::format_file_size;
pub use series::{codex_to_komga_reading_direction, codex_to_komga_status};
