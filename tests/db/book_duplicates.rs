#[path = "../common/mod.rs"]
mod common;

use codex::db::repositories::{BookDuplicatesRepository, BookRepository};
use common::*;
use uuid::Uuid;

#[tokio::test]
async fn test_rebuild_from_books_finds_duplicates() {
    let (db, _temp_dir) = setup_test_db().await;

    // Create library and series
    let library = create_test_library(&db, "Test Library", "/test/library").await;
    let series = create_test_series(&db, &library, "Test Series").await;

    // Create three books with the same file_hash
    let file_hash = "abc123";
    let book1 = create_test_book_with_hash(
        &db,
        &library,
        &series,
        "Book 1",
        "/test/book1.cbz",
        file_hash,
    )
    .await;
    let book2 = create_test_book_with_hash(
        &db,
        &library,
        &series,
        "Book 2",
        "/test/book2.cbz",
        file_hash,
    )
    .await;
    let book3 = create_test_book_with_hash(
        &db,
        &library,
        &series,
        "Book 3",
        "/test/book3.cbz",
        file_hash,
    )
    .await;

    // Create two books with a different file_hash
    let file_hash2 = "def456";
    let book4 = create_test_book_with_hash(
        &db,
        &library,
        &series,
        "Book 4",
        "/test/book4.cbz",
        file_hash2,
    )
    .await;
    let book5 = create_test_book_with_hash(
        &db,
        &library,
        &series,
        "Book 5",
        "/test/book5.cbz",
        file_hash2,
    )
    .await;

    // Create a book with a unique hash (should not appear in duplicates)
    create_test_book_with_hash(
        &db,
        &library,
        &series,
        "Book 6",
        "/test/book6.cbz",
        "unique789",
    )
    .await;

    // Rebuild duplicates
    let count = BookDuplicatesRepository::rebuild_from_books(&db)
        .await
        .unwrap();

    // Should have found 2 duplicate groups
    assert_eq!(count, 2);

    // Verify the duplicate groups
    let duplicates = BookDuplicatesRepository::find_all(&db).await.unwrap();
    assert_eq!(duplicates.len(), 2);

    // Find the group with file_hash "abc123"
    let group1 = duplicates
        .iter()
        .find(|d| d.file_hash == file_hash)
        .unwrap();
    assert_eq!(group1.duplicate_count, 3);
    let book_ids: Vec<Uuid> = serde_json::from_str(&group1.book_ids).unwrap();
    assert!(book_ids.contains(&book1.id));
    assert!(book_ids.contains(&book2.id));
    assert!(book_ids.contains(&book3.id));

    // Find the group with file_hash "def456"
    let group2 = duplicates
        .iter()
        .find(|d| d.file_hash == file_hash2)
        .unwrap();
    assert_eq!(group2.duplicate_count, 2);
    let book_ids: Vec<Uuid> = serde_json::from_str(&group2.book_ids).unwrap();
    assert!(book_ids.contains(&book4.id));
    assert!(book_ids.contains(&book5.id));
}

#[tokio::test]
async fn test_rebuild_excludes_deleted_books() {
    let (db, _temp_dir) = setup_test_db().await;

    let library = create_test_library(&db, "Test Library", "/test/library").await;
    let series = create_test_series(&db, &library, "Test Series").await;

    // Create two books with the same hash
    let file_hash = "abc123";
    let book1 = create_test_book_with_hash(
        &db,
        &library,
        &series,
        "Book 1",
        "/test/book1.cbz",
        file_hash,
    )
    .await;
    let book2 = create_test_book_with_hash(
        &db,
        &library,
        &series,
        "Book 2",
        "/test/book2.cbz",
        file_hash,
    )
    .await;

    // Soft delete book2
    BookRepository::mark_deleted(&db, book2.id, true)
        .await
        .unwrap();

    // Rebuild duplicates - should NOT find duplicates since one book is deleted
    let count = BookDuplicatesRepository::rebuild_from_books(&db)
        .await
        .unwrap();

    assert_eq!(count, 0);

    let duplicates = BookDuplicatesRepository::find_all(&db).await.unwrap();
    assert_eq!(duplicates.len(), 0);

    // Restore book2
    BookRepository::mark_deleted(&db, book2.id, false)
        .await
        .unwrap();

    // Rebuild - should now find the duplicate
    let count = BookDuplicatesRepository::rebuild_from_books(&db)
        .await
        .unwrap();

    assert_eq!(count, 1);
}

#[tokio::test]
async fn test_cleanup_removes_book_from_group() {
    let (db, _temp_dir) = setup_test_db().await;

    let library = create_test_library(&db, "Test Library", "/test/library").await;
    let series = create_test_series(&db, &library, "Test Series").await;

    // Create three books with the same hash
    let file_hash = "abc123";
    let book1 = create_test_book_with_hash(
        &db,
        &library,
        &series,
        "Book 1",
        "/test/book1.cbz",
        file_hash,
    )
    .await;
    let book2 = create_test_book_with_hash(
        &db,
        &library,
        &series,
        "Book 2",
        "/test/book2.cbz",
        file_hash,
    )
    .await;
    let book3 = create_test_book_with_hash(
        &db,
        &library,
        &series,
        "Book 3",
        "/test/book3.cbz",
        file_hash,
    )
    .await;

    // Rebuild duplicates
    BookDuplicatesRepository::rebuild_from_books(&db)
        .await
        .unwrap();

    // Verify we have 1 group with 3 books
    let duplicates = BookDuplicatesRepository::find_all(&db).await.unwrap();
    assert_eq!(duplicates.len(), 1);
    assert_eq!(duplicates[0].duplicate_count, 3);

    // Cleanup book2
    BookDuplicatesRepository::cleanup_for_book(&db, book2.id)
        .await
        .unwrap();

    // Verify the group now has 2 books
    let duplicates = BookDuplicatesRepository::find_all(&db).await.unwrap();
    assert_eq!(duplicates.len(), 1);
    assert_eq!(duplicates[0].duplicate_count, 2);

    let book_ids: Vec<Uuid> = serde_json::from_str(&duplicates[0].book_ids).unwrap();
    assert!(book_ids.contains(&book1.id));
    assert!(!book_ids.contains(&book2.id)); // Should not contain book2
    assert!(book_ids.contains(&book3.id));
}

#[tokio::test]
async fn test_cleanup_deletes_group_when_only_one_remains() {
    let (db, _temp_dir) = setup_test_db().await;

    let library = create_test_library(&db, "Test Library", "/test/library").await;
    let series = create_test_series(&db, &library, "Test Series").await;

    // Create two books with the same hash
    let file_hash = "abc123";
    let book1 = create_test_book_with_hash(
        &db,
        &library,
        &series,
        "Book 1",
        "/test/book1.cbz",
        file_hash,
    )
    .await;
    let book2 = create_test_book_with_hash(
        &db,
        &library,
        &series,
        "Book 2",
        "/test/book2.cbz",
        file_hash,
    )
    .await;

    // Rebuild duplicates
    BookDuplicatesRepository::rebuild_from_books(&db)
        .await
        .unwrap();

    // Verify we have 1 group with 2 books
    let duplicates = BookDuplicatesRepository::find_all(&db).await.unwrap();
    assert_eq!(duplicates.len(), 1);

    // Cleanup book2 - should delete the entire group
    BookDuplicatesRepository::cleanup_for_book(&db, book2.id)
        .await
        .unwrap();

    // Verify the group was deleted
    let duplicates = BookDuplicatesRepository::find_all(&db).await.unwrap();
    assert_eq!(duplicates.len(), 0);
}

#[tokio::test]
async fn test_find_by_hash() {
    let (db, _temp_dir) = setup_test_db().await;

    let library = create_test_library(&db, "Test Library", "/test/library").await;
    let series = create_test_series(&db, &library, "Test Series").await;

    let file_hash = "abc123";
    create_test_book_with_hash(
        &db,
        &library,
        &series,
        "Book 1",
        "/test/book1.cbz",
        file_hash,
    )
    .await;
    create_test_book_with_hash(
        &db,
        &library,
        &series,
        "Book 2",
        "/test/book2.cbz",
        file_hash,
    )
    .await;

    BookDuplicatesRepository::rebuild_from_books(&db)
        .await
        .unwrap();

    // Find by hash
    let group = BookDuplicatesRepository::find_by_hash(&db, file_hash)
        .await
        .unwrap();

    assert!(group.is_some());
    let group = group.unwrap();
    assert_eq!(group.file_hash, file_hash);
    assert_eq!(group.duplicate_count, 2);

    // Find by non-existent hash
    let group = BookDuplicatesRepository::find_by_hash(&db, "nonexistent")
        .await
        .unwrap();

    assert!(group.is_none());
}

#[tokio::test]
async fn test_delete_group() {
    let (db, _temp_dir) = setup_test_db().await;

    let library = create_test_library(&db, "Test Library", "/test/library").await;
    let series = create_test_series(&db, &library, "Test Series").await;

    let file_hash = "abc123";
    create_test_book_with_hash(
        &db,
        &library,
        &series,
        "Book 1",
        "/test/book1.cbz",
        file_hash,
    )
    .await;
    create_test_book_with_hash(
        &db,
        &library,
        &series,
        "Book 2",
        "/test/book2.cbz",
        file_hash,
    )
    .await;

    BookDuplicatesRepository::rebuild_from_books(&db)
        .await
        .unwrap();

    let duplicates = BookDuplicatesRepository::find_all(&db).await.unwrap();
    assert_eq!(duplicates.len(), 1);

    let group_id = duplicates[0].id;

    // Delete the group
    BookDuplicatesRepository::delete_group(&db, group_id)
        .await
        .unwrap();

    // Verify it's deleted
    let duplicates = BookDuplicatesRepository::find_all(&db).await.unwrap();
    assert_eq!(duplicates.len(), 0);
}

#[tokio::test]
async fn test_count() {
    let (db, _temp_dir) = setup_test_db().await;

    let library = create_test_library(&db, "Test Library", "/test/library").await;
    let series = create_test_series(&db, &library, "Test Series").await;

    // Initially 0
    let count = BookDuplicatesRepository::count(&db).await.unwrap();
    assert_eq!(count, 0);

    // Create duplicates
    let file_hash1 = "abc123";
    create_test_book_with_hash(
        &db,
        &library,
        &series,
        "Book 1",
        "/test/book1.cbz",
        file_hash1,
    )
    .await;
    create_test_book_with_hash(
        &db,
        &library,
        &series,
        "Book 2",
        "/test/book2.cbz",
        file_hash1,
    )
    .await;

    let file_hash2 = "def456";
    create_test_book_with_hash(
        &db,
        &library,
        &series,
        "Book 3",
        "/test/book3.cbz",
        file_hash2,
    )
    .await;
    create_test_book_with_hash(
        &db,
        &library,
        &series,
        "Book 4",
        "/test/book4.cbz",
        file_hash2,
    )
    .await;

    BookDuplicatesRepository::rebuild_from_books(&db)
        .await
        .unwrap();

    let count = BookDuplicatesRepository::count(&db).await.unwrap();
    assert_eq!(count, 2);
}

#[tokio::test]
async fn test_rebuild_is_idempotent() {
    let (db, _temp_dir) = setup_test_db().await;

    let library = create_test_library(&db, "Test Library", "/test/library").await;
    let series = create_test_series(&db, &library, "Test Series").await;

    let file_hash = "abc123";
    create_test_book_with_hash(
        &db,
        &library,
        &series,
        "Book 1",
        "/test/book1.cbz",
        file_hash,
    )
    .await;
    create_test_book_with_hash(
        &db,
        &library,
        &series,
        "Book 2",
        "/test/book2.cbz",
        file_hash,
    )
    .await;

    // Rebuild multiple times
    let count1 = BookDuplicatesRepository::rebuild_from_books(&db)
        .await
        .unwrap();
    let count2 = BookDuplicatesRepository::rebuild_from_books(&db)
        .await
        .unwrap();
    let count3 = BookDuplicatesRepository::rebuild_from_books(&db)
        .await
        .unwrap();

    assert_eq!(count1, count2);
    assert_eq!(count2, count3);

    // Should still have exactly 1 group
    let duplicates = BookDuplicatesRepository::find_all(&db).await.unwrap();
    assert_eq!(duplicates.len(), 1);
}
