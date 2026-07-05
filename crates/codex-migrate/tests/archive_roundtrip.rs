//! End-to-end archive round-trip: export a source DB plus on-disk artifacts to
//! a `.tar.gz`, import into a fresh DB whose artifact dirs differ, and assert
//! the data mirrors and every stored file path is re-rooted and resolvable.

use std::fs;
use std::path::Path;

use chrono::Utc;
use codex_db::entities::{book_covers, books, series_covers};
use codex_db::test_helpers::create_test_db;
use codex_migrate::archive::{ArtifactSource, ArtifactTarget, export_archive, import_archive};
use codex_migrate::manifest::ArtifactGroup;
use codex_migrate::{registry, verify};
use sea_orm::{ActiveModelTrait, EntityTrait, Set};
use uuid::Uuid;

struct Seeded {
    book_id: Uuid,
    book_cover_id: Uuid,
    series_cover_id: Uuid,
    thumb_stored: String,
    book_cover_stored: String,
    series_cover_stored: String,
}

/// Write a small file at `abs`, creating parents. Returns the path as stored.
fn touch(abs: &Path) -> String {
    fs::create_dir_all(abs.parent().unwrap()).unwrap();
    fs::write(abs, b"BINARY").unwrap();
    abs.to_string_lossy().into_owned()
}

/// Seed a library → series → book, a book cover and a series cover, with their
/// image files placed under `thumb_dir` / `uploads_dir` and the DB storing the
/// absolute paths (mirroring an absolute-`files.*_dir` config).
async fn seed(db: &codex_db::Database, thumb_dir: &Path, uploads_dir: &Path) -> Seeded {
    let library = db
        .create_library("Comics", "/library", codex_db::ScanningStrategy::Default)
        .await
        .unwrap();
    let series = db.create_series(library.id, "Saga").await.unwrap();
    let conn = db.sea_orm_connection();

    let book_id = Uuid::new_v4();
    let thumb_abs = thumb_dir.join(format!("books/{}/{book_id}.jpg", &book_id.to_string()[..2]));
    let thumb_stored = touch(&thumb_abs);

    books::ActiveModel {
        id: Set(book_id),
        series_id: Set(series.id),
        library_id: Set(library.id),
        path: Set("/library/Saga/001.cbz".to_string()),
        file_name: Set("001.cbz".to_string()),
        file_size: Set(1234),
        file_hash: Set("hash".to_string()),
        partial_hash: Set("phash".to_string()),
        format: Set("cbz".to_string()),
        page_count: Set(20),
        deleted: Set(false),
        analyzed: Set(true),
        modified_at: Set(Utc::now()),
        created_at: Set(Utc::now()),
        updated_at: Set(Utc::now()),
        thumbnail_path: Set(Some(thumb_stored.clone())),
        ..Default::default()
    }
    .insert(conn)
    .await
    .unwrap();

    let book_cover_id = Uuid::new_v4();
    let bc_abs = uploads_dir.join(format!("covers/books/{book_id}/{book_cover_id}.jpg"));
    let book_cover_stored = touch(&bc_abs);
    book_covers::ActiveModel {
        id: Set(book_cover_id),
        book_id: Set(book_id),
        source: Set("custom".to_string()),
        path: Set(book_cover_stored.clone()),
        is_selected: Set(true),
        width: Set(None),
        height: Set(None),
        created_at: Set(Utc::now()),
        updated_at: Set(Utc::now()),
    }
    .insert(conn)
    .await
    .unwrap();

    let series_cover_id = Uuid::new_v4();
    let sc_abs = uploads_dir.join(format!("covers/series/{}/{series_cover_id}.jpg", series.id));
    let series_cover_stored = touch(&sc_abs);
    series_covers::ActiveModel {
        id: Set(series_cover_id),
        series_id: Set(series.id),
        source: Set("custom".to_string()),
        path: Set(series_cover_stored.clone()),
        is_selected: Set(true),
        width: Set(None),
        height: Set(None),
        created_at: Set(Utc::now()),
        updated_at: Set(Utc::now()),
    }
    .insert(conn)
    .await
    .unwrap();

    Seeded {
        book_id,
        book_cover_id,
        series_cover_id,
        thumb_stored,
        book_cover_stored,
        series_cover_stored,
    }
}

#[tokio::test]
async fn archive_roundtrip_mirrors_data_and_reroots_paths() {
    let scratch = tempfile::tempdir().unwrap();
    let src_thumbs = scratch.path().join("src/thumbnails");
    let src_uploads = scratch.path().join("src/uploads");
    let dst_thumbs = scratch.path().join("dst/thumbnails");
    let dst_uploads = scratch.path().join("dst/uploads");
    let archive_path = scratch.path().join("export.tar.gz");

    // --- Source: DB + artifact files on disk. ---
    let (src, _src_dir) = create_test_db().await;
    let seeded = seed(&src, &src_thumbs, &src_uploads).await;

    let manifest = export_archive(
        src.sea_orm_connection(),
        &archive_path,
        &[
            ArtifactSource {
                group: ArtifactGroup::Thumbnails,
                source_dir: src_thumbs.clone(),
            },
            ArtifactSource {
                group: ArtifactGroup::Uploads,
                source_dir: src_uploads.clone(),
            },
        ],
        codex_migrate::Progress::Silent,
    )
    .await
    .expect("export should succeed");

    assert!(archive_path.exists(), "archive file written");
    assert_eq!(manifest.source_backend, "sqlite");
    assert!(manifest.schema_version.is_some());
    assert_eq!(manifest.artifacts.len(), 2);
    assert!(manifest.total_rows >= 5);

    // --- Target: fresh DB, DIFFERENT artifact dirs. ---
    let (dst, _dst_dir) = create_test_db().await;
    let outcome = import_archive(
        dst.sea_orm_connection(),
        &archive_path,
        &[
            ArtifactTarget {
                group: ArtifactGroup::Thumbnails,
                target_dir: dst_thumbs.clone(),
            },
            ArtifactTarget {
                group: ArtifactGroup::Uploads,
                target_dir: dst_uploads.clone(),
            },
        ],
        codex_migrate::Progress::Silent,
        false,
    )
    .await
    .expect("import should succeed");

    // Row-count parity across every table.
    let src_counts = registry::count_all(src.sea_orm_connection()).await.unwrap();
    let dst_counts = registry::count_all(dst.sea_orm_connection()).await.unwrap();
    assert!(
        verify::compare(&src_counts, &dst_counts).is_empty(),
        "row counts must mirror"
    );
    assert_eq!(outcome.reroot.thumbnails, 1);
    assert_eq!(outcome.reroot.covers, 2);

    // Stored paths were re-rooted from the source base dirs to the target's.
    let dst_conn = dst.sea_orm_connection();
    let book = books::Entity::find_by_id(seeded.book_id)
        .one(dst_conn)
        .await
        .unwrap()
        .unwrap();
    let new_thumb = book.thumbnail_path.expect("thumbnail path present");
    assert!(
        new_thumb.starts_with(dst_thumbs.to_string_lossy().as_ref()),
        "thumbnail re-rooted: {new_thumb}"
    );
    assert_ne!(new_thumb, seeded.thumb_stored, "path actually changed");
    // ...and the bundled file was unpacked to the new location and resolves.
    assert!(Path::new(&new_thumb).exists(), "thumbnail file unpacked");

    let bc = book_covers::Entity::find_by_id(seeded.book_cover_id)
        .one(dst_conn)
        .await
        .unwrap()
        .unwrap();
    assert!(bc.path.starts_with(dst_uploads.to_string_lossy().as_ref()));
    assert_ne!(bc.path, seeded.book_cover_stored);
    assert!(Path::new(&bc.path).exists(), "book cover unpacked");

    let sc = series_covers::Entity::find_by_id(seeded.series_cover_id)
        .one(dst_conn)
        .await
        .unwrap()
        .unwrap();
    assert!(sc.path.starts_with(dst_uploads.to_string_lossy().as_ref()));
    assert_ne!(sc.path, seeded.series_cover_stored);
    assert!(Path::new(&sc.path).exists(), "series cover unpacked");
}
