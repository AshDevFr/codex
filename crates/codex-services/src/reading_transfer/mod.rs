//! Export and import of one user's reading state across a library
//! reorganisation or an instance move.
//!
//! Every piece of state this feature touches (`read_progress`,
//! `read_completions`, `reading_sessions`, `user_series_ratings`) is keyed on
//! `books.id` / `series.id`, and both are minted fresh whenever a library is
//! rescanned under a new root. Export serialises the state against stable
//! identifiers (external ids, relative paths, file names, hashes) instead;
//! import resolves those back to real ids in the current library and writes
//! the state under an explicit conflict policy.
//!
//! Submodules:
//! - [`export`] assembles the document from the four tables.
//! - [`matching`] resolves a document's series and books against the current
//!   library, never guessing: any step with more than one candidate is
//!   reported as ambiguous rather than picking one.
//! - [`import`] applies a matched document, one transaction per series, and
//!   can run as a dry run that writes nothing but produces the same report.

pub mod export;
pub mod import;
pub mod matching;
pub mod model;

use std::path::Path;

/// Compute a book's path relative to its series folder.
///
/// `books.path` is absolute; `series.path` is relative to the library root.
/// Stripping both prefixes is what makes the result stable across a library
/// re-root: the series folder can move to a different library root entirely
/// and this value does not change, which is the whole point of exporting it
/// instead of the absolute path.
///
/// Falls back to the book's file name if the book path is not actually inside
/// the library/series folders implied by its own rows (a data inconsistency
/// that should not happen, but must not panic the export).
pub fn series_relative_book_path(library_path: &str, series_path: &str, book_path: &str) -> String {
    let Ok(library_relative) = Path::new(book_path).strip_prefix(library_path) else {
        // Should never happen on consistent data; keep the original path
        // rather than mangling it through a failed strip.
        return book_path.to_string();
    };

    let series_relative = if series_path.is_empty() {
        library_relative
    } else {
        library_relative
            .strip_prefix(series_path)
            .unwrap_or(library_relative)
    };

    // Normalise to forward slashes so the exported path is stable regardless
    // of the platform the server runs on.
    series_relative
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

/// The filename stem used for the "survives a `.cbr` repacked to `.cbz`"
/// matching step: everything before the last `.`, or the whole name when
/// there is no extension.
pub fn file_stem(file_name: &str) -> &str {
    match file_name.rsplit_once('.') {
        Some((stem, _ext)) if !stem.is_empty() => stem,
        _ => file_name,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_library_and_series_prefix() {
        assert_eq!(
            series_relative_book_path(
                "/library/root",
                "shonen/Naruto",
                "/library/root/shonen/Naruto/Vol 01/v01.cbz"
            ),
            "Vol 01/v01.cbz"
        );
    }

    #[test]
    fn handles_series_at_library_root() {
        assert_eq!(
            series_relative_book_path("/library/root", "", "/library/root/v01.cbz"),
            "v01.cbz"
        );
    }

    #[test]
    fn falls_back_gracefully_when_prefixes_do_not_match() {
        // Should never happen, but must not panic on inconsistent data.
        assert_eq!(
            series_relative_book_path("/other/root", "shonen/Naruto", "/library/root/v01.cbz"),
            "/library/root/v01.cbz"
        );
    }

    #[test]
    fn stem_strips_extension() {
        assert_eq!(file_stem("v01.cbz"), "v01");
        assert_eq!(file_stem("v01.cbr"), "v01");
    }

    #[test]
    fn stem_of_extensionless_name_is_the_name() {
        assert_eq!(file_stem("README"), "README");
    }

    #[test]
    fn stem_of_dotfile_is_the_whole_name() {
        // rsplit_once yields an empty stem for a leading dot; that is not a
        // usable stem, so the whole name is kept instead.
        assert_eq!(file_stem(".gitignore"), ".gitignore");
    }
}
