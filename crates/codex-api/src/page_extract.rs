//! Page image extraction, addressed by archive entry name where possible.
//!
//! Page numbers are positional, and the scanner and the extractor do not filter
//! an archive identically: the scanner drops entries whose dimensions cannot be
//! read, while the entry list built for extraction keeps them. Indexing by
//! position therefore drifts by one for every dropped entry, and page N serves
//! an image the page row does not describe. A truncated JPEG, an SVG resvg
//! cannot parse, or a JXL jxl-oxide cannot read is enough to trigger it, and the
//! symptom (one wrong image, then every image after it) reads as a bad rip
//! rather than a bug.
//!
//! Whenever the page row is known, its `file_name` addresses the entry directly
//! and cannot drift. Positional extraction remains the fallback for books that
//! have not been analyzed yet, which have no page rows at all.

/// Extract the image bytes for a page from a book file.
///
/// `file_name` is the entry name recorded on the page row. When it is present
/// and the format is a comic archive, the entry is addressed by name; otherwise
/// extraction falls back to the positional form.
///
/// Only CBZ and CBR resolve by name:
///
/// - **PDF** page rows record a synthetic `page_N.jpg` name for a page that is
///   rendered on demand, not an entry that exists in the file.
/// - **EPUB** numbers pages positionally over the same unfiltered list the
///   extractor walks, so the two already agree, and page 1 deliberately
///   resolves to the cover named in the OPF rather than to the first image.
///
/// ZIP/RAR/EPUB parsing and PDF rendering are blocking file work, so they run on
/// the blocking pool.
pub async fn extract_page_image(
    path: &str,
    file_format: &str,
    page_number: i32,
    file_name: Option<&str>,
) -> anyhow::Result<Vec<u8>> {
    let path = std::path::PathBuf::from(path);
    let format = file_format.to_uppercase();
    let file_name = file_name.map(str::to_string);

    tokio::task::spawn_blocking(move || {
        if let Some(name) = file_name.as_deref() {
            let by_name = match format.as_str() {
                "CBZ" => Some(codex_parsers::cbz::extract_page_from_cbz_by_name(
                    &path, name,
                )),
                #[cfg(feature = "rar")]
                "CBR" => Some(codex_parsers::cbr::extract_page_from_cbr_by_name(
                    &path, name,
                )),
                _ => None,
            };

            match by_name {
                Some(Ok(data)) => return Ok(data),
                // The page row names an entry the archive no longer has, so the
                // file changed since the scan. Positional extraction is the best
                // remaining guess; a rescan is what actually fixes it.
                Some(Err(e)) => tracing::warn!(
                    file_name = %name,
                    page = page_number,
                    error = %e,
                    "page entry not found by name; falling back to positional extraction"
                ),
                None => {}
            }
        }

        match format.as_str() {
            "CBZ" => codex_parsers::cbz::extract_page_from_cbz(&path, page_number),
            #[cfg(feature = "rar")]
            "CBR" => codex_parsers::cbr::extract_page_from_cbr(&path, page_number),
            "EPUB" => codex_parsers::epub::extract_page_from_epub(&path, page_number),
            "PDF" => codex_parsers::pdf::extract_page_from_pdf(&path, page_number),
            _ => anyhow::bail!("Unsupported format for page extraction: {}", format),
        }
    })
    .await
    .map_err(|e| anyhow::anyhow!("image extraction join error: {e}"))?
}
