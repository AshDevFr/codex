#[path = "../common/mod.rs"]
mod common;

// Schema/entity tests for collections, read lists, and the per-user
// want-to-read queue (Phase 1 foundation). Exercises round-trips, ordered
// membership, uniqueness, cascade deletes, and the want_to_read CHECK
// constraint on both SQLite and (ignored) PostgreSQL.

use chrono::Utc;
use codex::db::entities::{
    collection_series, collections, read_list_books, read_lists, want_to_read,
};
use codex::db::repositories::UserRepository;
use common::*;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, Set,
};
use uuid::Uuid;

async fn persist_user(db: &DatabaseConnection, username: &str) -> users::Model {
    let model = create_test_user(username, &format!("{username}@test.test"), "hash", false);
    UserRepository::create(db, &model).await.unwrap()
}

// ============================================================================
// Collections
// ============================================================================

/// Shared assertions: collection holds ordered series membership, membership is
/// unique per (collection, series), and cascades when either side is deleted.
async fn exercise_collections(db: &DatabaseConnection) {
    let lib = create_test_library(db, &format!("Lib-{}", Uuid::new_v4()), &uniq_path()).await;
    let s1 = create_test_series(db, &lib, "Series A").await;
    let s2 = create_test_series(db, &lib, "Series B").await;

    let coll = collections::ActiveModel {
        id: Set(Uuid::new_v4()),
        name: Set(format!("Batman-{}", Uuid::new_v4())),
        normalized_name: Set(format!("batman-{}", Uuid::new_v4())),
        summary: Set(None),
        ordered: Set(true),
        created_at: Set(Utc::now()),
        updated_at: Set(Utc::now()),
    }
    .insert(db)
    .await
    .unwrap();

    // s2 first (position 0), s1 second (position 1) — deliberately reverse of
    // insertion so the order test verifies position, not insertion order.
    for (pos, s) in [(0, &s2), (1, &s1)] {
        collection_series::ActiveModel {
            id: Set(Uuid::new_v4()),
            collection_id: Set(coll.id),
            series_id: Set(s.id),
            position: Set(pos),
            created_at: Set(Utc::now()),
        }
        .insert(db)
        .await
        .unwrap();
    }

    // Members come back ordered by position.
    let members = collection_series::Entity::find()
        .filter(collection_series::Column::CollectionId.eq(coll.id))
        .order_by_asc(collection_series::Column::Position)
        .all(db)
        .await
        .unwrap();
    assert_eq!(members.len(), 2);
    assert_eq!(members[0].series_id, s2.id);
    assert_eq!(members[1].series_id, s1.id);

    // A series can't be added to the same collection twice.
    let dup = collection_series::ActiveModel {
        id: Set(Uuid::new_v4()),
        collection_id: Set(coll.id),
        series_id: Set(s1.id),
        position: Set(2),
        created_at: Set(Utc::now()),
    }
    .insert(db)
    .await;
    assert!(
        dup.is_err(),
        "duplicate (collection, series) must be rejected"
    );

    // Deleting a series cascades its membership row away.
    series::Entity::delete_by_id(s1.id).exec(db).await.unwrap();
    let after_series_delete = collection_series::Entity::find()
        .filter(collection_series::Column::SeriesId.eq(s1.id))
        .count(db)
        .await
        .unwrap();
    assert_eq!(
        after_series_delete, 0,
        "series delete should cascade membership"
    );

    // Deleting the collection cascades the remaining membership rows.
    collections::Entity::delete_by_id(coll.id)
        .exec(db)
        .await
        .unwrap();
    let after_coll_delete = collection_series::Entity::find()
        .filter(collection_series::Column::CollectionId.eq(coll.id))
        .count(db)
        .await
        .unwrap();
    assert_eq!(
        after_coll_delete, 0,
        "collection delete should cascade membership"
    );
}

#[tokio::test]
async fn test_collections_sqlite() {
    let (db, _t) = setup_test_db().await;
    exercise_collections(&db).await;
}

#[tokio::test]
#[ignore] // Requires PostgreSQL test database
async fn test_collections_postgres() {
    let Some(db) = setup_test_db_postgres().await else {
        eprintln!("PostgreSQL test database not available, skipping");
        return;
    };
    exercise_collections(&db).await;
}

// ============================================================================
// Read lists
// ============================================================================

async fn exercise_read_lists(db: &DatabaseConnection) {
    let lib = create_test_library(db, &format!("Lib-{}", Uuid::new_v4()), &uniq_path()).await;
    let series = create_test_series(db, &lib, "Series A").await;
    let b1 = create_test_book_with_hash(db, &lib, &series, "B1", &uniq_path(), &rand_hash()).await;
    let b2 = create_test_book_with_hash(db, &lib, &series, "B2", &uniq_path(), &rand_hash()).await;

    let rl = read_lists::ActiveModel {
        id: Set(Uuid::new_v4()),
        name: Set(format!("Wolverine-{}", Uuid::new_v4())),
        normalized_name: Set(format!("wolverine-{}", Uuid::new_v4())),
        summary: Set(Some("Every book where Wolverine appears".to_string())),
        ordered: Set(true),
        created_at: Set(Utc::now()),
        updated_at: Set(Utc::now()),
    }
    .insert(db)
    .await
    .unwrap();
    assert_eq!(
        rl.summary.as_deref(),
        Some("Every book where Wolverine appears")
    );

    for (pos, b) in [(0, &b2), (1, &b1)] {
        read_list_books::ActiveModel {
            id: Set(Uuid::new_v4()),
            read_list_id: Set(rl.id),
            book_id: Set(b.id),
            position: Set(pos),
            created_at: Set(Utc::now()),
        }
        .insert(db)
        .await
        .unwrap();
    }

    let members = read_list_books::Entity::find()
        .filter(read_list_books::Column::ReadListId.eq(rl.id))
        .order_by_asc(read_list_books::Column::Position)
        .all(db)
        .await
        .unwrap();
    assert_eq!(members.len(), 2);
    assert_eq!(members[0].book_id, b2.id);
    assert_eq!(members[1].book_id, b1.id);

    // A book can't be added to the same read list twice.
    let dup = read_list_books::ActiveModel {
        id: Set(Uuid::new_v4()),
        read_list_id: Set(rl.id),
        book_id: Set(b1.id),
        position: Set(2),
        created_at: Set(Utc::now()),
    }
    .insert(db)
    .await;
    assert!(dup.is_err(), "duplicate (read_list, book) must be rejected");

    // Deleting a book cascades its membership row.
    books::Entity::delete_by_id(b1.id).exec(db).await.unwrap();
    let after_book_delete = read_list_books::Entity::find()
        .filter(read_list_books::Column::BookId.eq(b1.id))
        .count(db)
        .await
        .unwrap();
    assert_eq!(
        after_book_delete, 0,
        "book delete should cascade membership"
    );

    // Deleting the read list cascades remaining membership.
    read_lists::Entity::delete_by_id(rl.id)
        .exec(db)
        .await
        .unwrap();
    let after_rl_delete = read_list_books::Entity::find()
        .filter(read_list_books::Column::ReadListId.eq(rl.id))
        .count(db)
        .await
        .unwrap();
    assert_eq!(
        after_rl_delete, 0,
        "read list delete should cascade membership"
    );
}

#[tokio::test]
async fn test_read_lists_sqlite() {
    let (db, _t) = setup_test_db().await;
    exercise_read_lists(&db).await;
}

#[tokio::test]
#[ignore] // Requires PostgreSQL test database
async fn test_read_lists_postgres() {
    let Some(db) = setup_test_db_postgres().await else {
        eprintln!("PostgreSQL test database not available, skipping");
        return;
    };
    exercise_read_lists(&db).await;
}

// ============================================================================
// Want to Read (per-user queue + CHECK constraint + partial uniqueness)
// ============================================================================

async fn exercise_want_to_read(db: &DatabaseConnection) {
    let user = persist_user(db, &format!("wtr-{}", Uuid::new_v4())).await;
    let lib = create_test_library(db, &format!("Lib-{}", Uuid::new_v4()), &uniq_path()).await;
    let series = create_test_series(db, &lib, "Series A").await;
    let book = create_test_book_with_hash(db, &lib, &series, "B", &uniq_path(), &rand_hash()).await;
    let book2 =
        create_test_book_with_hash(db, &lib, &series, "B2", &uniq_path(), &rand_hash()).await;

    // A series-only entry and a book-only entry both insert cleanly.
    want_to_read::ActiveModel {
        id: Set(Uuid::new_v4()),
        user_id: Set(user.id),
        series_id: Set(Some(series.id)),
        book_id: Set(None),
        added_at: Set(Utc::now()),
        position: Set(0),
    }
    .insert(db)
    .await
    .unwrap();

    want_to_read::ActiveModel {
        id: Set(Uuid::new_v4()),
        user_id: Set(user.id),
        series_id: Set(None),
        book_id: Set(Some(book.id)),
        added_at: Set(Utc::now()),
        position: Set(0),
    }
    .insert(db)
    .await
    .unwrap();

    let queue = want_to_read::Entity::find()
        .filter(want_to_read::Column::UserId.eq(user.id))
        .all(db)
        .await
        .unwrap();
    assert_eq!(
        queue.len(),
        2,
        "both a series and a book entry should persist"
    );

    // CHECK: neither side set is rejected.
    let both_null = want_to_read::ActiveModel {
        id: Set(Uuid::new_v4()),
        user_id: Set(user.id),
        series_id: Set(None),
        book_id: Set(None),
        added_at: Set(Utc::now()),
        position: Set(0),
    }
    .insert(db)
    .await;
    assert!(
        both_null.is_err(),
        "both-null must violate the CHECK constraint"
    );

    // CHECK: both sides set is rejected.
    let both_set = want_to_read::ActiveModel {
        id: Set(Uuid::new_v4()),
        user_id: Set(user.id),
        series_id: Set(Some(series.id)),
        book_id: Set(Some(book2.id)),
        added_at: Set(Utc::now()),
        position: Set(0),
    }
    .insert(db)
    .await;
    assert!(
        both_set.is_err(),
        "both-set must violate the CHECK constraint"
    );

    // Same series can't be flagged twice for the same user.
    let dup_series = want_to_read::ActiveModel {
        id: Set(Uuid::new_v4()),
        user_id: Set(user.id),
        series_id: Set(Some(series.id)),
        book_id: Set(None),
        added_at: Set(Utc::now()),
        position: Set(0),
    }
    .insert(db)
    .await;
    assert!(
        dup_series.is_err(),
        "duplicate (user, series) must be rejected"
    );

    // A second, distinct book-only entry coexists with the first even though
    // both have series_id NULL — NULLs are distinct in the unique index.
    want_to_read::ActiveModel {
        id: Set(Uuid::new_v4()),
        user_id: Set(user.id),
        series_id: Set(None),
        book_id: Set(Some(book2.id)),
        added_at: Set(Utc::now()),
        position: Set(0),
    }
    .insert(db)
    .await
    .expect("multiple book-only rows (series_id NULL) for one user must be allowed");

    // Deleting the user cascades the whole queue away.
    users::Entity::delete_by_id(user.id).exec(db).await.unwrap();
    let after_user_delete = want_to_read::Entity::find()
        .filter(want_to_read::Column::UserId.eq(user.id))
        .count(db)
        .await
        .unwrap();
    assert_eq!(
        after_user_delete, 0,
        "user delete should cascade the want-to-read queue"
    );
}

#[tokio::test]
async fn test_want_to_read_sqlite() {
    let (db, _t) = setup_test_db().await;
    exercise_want_to_read(&db).await;
}

#[tokio::test]
#[ignore] // Requires PostgreSQL test database
async fn test_want_to_read_postgres() {
    let Some(db) = setup_test_db_postgres().await else {
        eprintln!("PostgreSQL test database not available, skipping");
        return;
    };
    exercise_want_to_read(&db).await;
}

// Distinct, throwaway paths/hashes so parallel tests (and the shared PostgreSQL
// database) never collide on unique constraints.
fn uniq_path() -> String {
    format!("/test/{}", Uuid::new_v4())
}

fn rand_hash() -> String {
    Uuid::new_v4().simple().to_string()
}
