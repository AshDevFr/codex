#[path = "../common/mod.rs"]
mod common;

use chrono::{Duration, Utc};
use codex::db::repositories::{NewRefreshToken, RefreshTokenRepository, UserRepository};
use common::*;
use uuid::Uuid;

async fn make_user(db: &sea_orm::DatabaseConnection, label: &str) -> uuid::Uuid {
    let user = create_test_user(
        &format!("{}-{}", label, Uuid::new_v4()),
        &format!("{}-{}@example.com", label, Uuid::new_v4()),
        "hash",
        false,
    );
    UserRepository::create(db, &user).await.unwrap().id
}

fn input(user_id: Uuid, family_id: Uuid, hash: &str, ttl_days: i64) -> NewRefreshToken {
    let now = Utc::now();
    NewRefreshToken {
        user_id,
        family_id,
        token_hash: hash.to_string(),
        issued_at: now,
        expires_at: now + Duration::days(ttl_days),
        user_agent: Some("test-ua".to_string()),
        ip_address: Some("127.0.0.1".to_string()),
    }
}

#[tokio::test]
async fn create_and_lookup_by_hash() {
    let (db, _tmp) = setup_test_db().await;
    let user_id = make_user(&db, "create").await;
    let family_id = Uuid::new_v4();

    let inserted = RefreshTokenRepository::create(&db, input(user_id, family_id, "hash-abc", 30))
        .await
        .unwrap();

    assert_eq!(inserted.user_id, user_id);
    assert_eq!(inserted.family_id, family_id);
    assert!(inserted.revoked_at.is_none());
    assert!(inserted.replaced_by.is_none());

    let by_hash = RefreshTokenRepository::get_by_hash(&db, "hash-abc")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(by_hash.id, inserted.id);

    let missing = RefreshTokenRepository::get_by_hash(&db, "no-such-hash")
        .await
        .unwrap();
    assert!(missing.is_none());
}

#[tokio::test]
async fn revoke_is_idempotent_and_scoped() {
    let (db, _tmp) = setup_test_db().await;
    let user_id = make_user(&db, "revoke").await;
    let family_id = Uuid::new_v4();

    let a = RefreshTokenRepository::create(&db, input(user_id, family_id, "h-a", 30))
        .await
        .unwrap();
    let b = RefreshTokenRepository::create(&db, input(user_id, family_id, "h-b", 30))
        .await
        .unwrap();

    let affected = RefreshTokenRepository::revoke(&db, a.id).await.unwrap();
    assert_eq!(affected, 1);

    // Second revoke is a no-op: row was already revoked.
    let affected2 = RefreshTokenRepository::revoke(&db, a.id).await.unwrap();
    assert_eq!(affected2, 0);

    let a_row = RefreshTokenRepository::get_by_id(&db, a.id)
        .await
        .unwrap()
        .unwrap();
    let b_row = RefreshTokenRepository::get_by_id(&db, b.id)
        .await
        .unwrap()
        .unwrap();
    assert!(a_row.revoked_at.is_some());
    assert!(
        b_row.revoked_at.is_none(),
        "sibling in same family must not be touched by per-row revoke"
    );
}

#[tokio::test]
async fn revoke_family_marks_all_active_siblings() {
    let (db, _tmp) = setup_test_db().await;
    let user_id = make_user(&db, "fam").await;
    let family_id = Uuid::new_v4();
    let other_family_id = Uuid::new_v4();

    let a = RefreshTokenRepository::create(&db, input(user_id, family_id, "h-a", 30))
        .await
        .unwrap();
    let b = RefreshTokenRepository::create(&db, input(user_id, family_id, "h-b", 30))
        .await
        .unwrap();
    let other = RefreshTokenRepository::create(&db, input(user_id, other_family_id, "h-x", 30))
        .await
        .unwrap();

    let affected = RefreshTokenRepository::revoke_family(&db, family_id)
        .await
        .unwrap();
    assert_eq!(affected, 2);

    let a_row = RefreshTokenRepository::get_by_id(&db, a.id)
        .await
        .unwrap()
        .unwrap();
    let b_row = RefreshTokenRepository::get_by_id(&db, b.id)
        .await
        .unwrap()
        .unwrap();
    let other_row = RefreshTokenRepository::get_by_id(&db, other.id)
        .await
        .unwrap()
        .unwrap();
    assert!(a_row.revoked_at.is_some());
    assert!(b_row.revoked_at.is_some());
    assert!(
        other_row.revoked_at.is_none(),
        "different family must not be revoked"
    );
}

#[tokio::test]
async fn rotate_revokes_old_and_links_replaced_by() {
    let (db, _tmp) = setup_test_db().await;
    let user_id = make_user(&db, "rot").await;
    let family_id = Uuid::new_v4();

    let original = RefreshTokenRepository::create(&db, input(user_id, family_id, "h-old", 30))
        .await
        .unwrap();

    let new_row =
        RefreshTokenRepository::rotate(&db, original.id, input(user_id, family_id, "h-new", 30))
            .await
            .unwrap();

    assert_eq!(new_row.family_id, family_id);
    assert_eq!(new_row.user_id, user_id);
    assert!(new_row.revoked_at.is_none());

    let old = RefreshTokenRepository::get_by_id(&db, original.id)
        .await
        .unwrap()
        .unwrap();
    assert!(old.revoked_at.is_some());
    assert_eq!(old.replaced_by, Some(new_row.id));
}

#[tokio::test]
async fn rotate_aborts_if_predecessor_already_revoked() {
    let (db, _tmp) = setup_test_db().await;
    let user_id = make_user(&db, "race").await;
    let family_id = Uuid::new_v4();

    let original = RefreshTokenRepository::create(&db, input(user_id, family_id, "h-x", 30))
        .await
        .unwrap();
    RefreshTokenRepository::revoke(&db, original.id)
        .await
        .unwrap();

    let result =
        RefreshTokenRepository::rotate(&db, original.id, input(user_id, family_id, "h-new", 30))
            .await;
    assert!(
        result.is_err(),
        "rotation must fail on already-revoked predecessor"
    );

    // No "h-new" row was inserted: the transaction rolled back.
    let new_lookup = RefreshTokenRepository::get_by_hash(&db, "h-new")
        .await
        .unwrap();
    assert!(new_lookup.is_none());
}

#[tokio::test]
async fn cleanup_expired_drops_expired_and_old_revoked_rows() {
    let (db, _tmp) = setup_test_db().await;
    let user_id = make_user(&db, "cleanup").await;
    let family_id = Uuid::new_v4();
    let now = Utc::now();

    // Expired but not revoked: must be deleted.
    let expired = RefreshTokenRepository::create(
        &db,
        NewRefreshToken {
            user_id,
            family_id,
            token_hash: "h-expired".to_string(),
            issued_at: now - Duration::days(40),
            expires_at: now - Duration::days(1),
            user_agent: None,
            ip_address: None,
        },
    )
    .await
    .unwrap();

    // Revoked long ago: must be deleted.
    let revoked_old = RefreshTokenRepository::create(
        &db,
        NewRefreshToken {
            user_id,
            family_id,
            token_hash: "h-revoked-old".to_string(),
            issued_at: now - Duration::days(60),
            expires_at: now + Duration::days(10),
            user_agent: None,
            ip_address: None,
        },
    )
    .await
    .unwrap();
    RefreshTokenRepository::revoke(&db, revoked_old.id)
        .await
        .unwrap();
    // Push the revoked_at back beyond the grace window via a raw update.
    use sea_orm::sea_query::Expr;
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
    codex::db::entities::refresh_tokens::Entity::update_many()
        .col_expr(
            codex::db::entities::refresh_tokens::Column::RevokedAt,
            Expr::value(Some(now - Duration::days(45))),
        )
        .filter(codex::db::entities::refresh_tokens::Column::Id.eq(revoked_old.id))
        .exec(&db)
        .await
        .unwrap();

    // Active token: must survive.
    let active = RefreshTokenRepository::create(&db, input(user_id, family_id, "h-active", 30))
        .await
        .unwrap();

    // Recently-revoked token: must survive (still within grace window).
    let recent_revoked =
        RefreshTokenRepository::create(&db, input(user_id, family_id, "h-recent", 30))
            .await
            .unwrap();
    RefreshTokenRepository::revoke(&db, recent_revoked.id)
        .await
        .unwrap();

    let removed = RefreshTokenRepository::cleanup_expired(&db, 30)
        .await
        .unwrap();
    assert_eq!(removed, 2, "expired and old-revoked rows must be dropped");

    assert!(
        RefreshTokenRepository::get_by_id(&db, expired.id)
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        RefreshTokenRepository::get_by_id(&db, revoked_old.id)
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        RefreshTokenRepository::get_by_id(&db, active.id)
            .await
            .unwrap()
            .is_some()
    );
    assert!(
        RefreshTokenRepository::get_by_id(&db, recent_revoked.id)
            .await
            .unwrap()
            .is_some()
    );
}

#[tokio::test]
async fn delete_by_user_id_clears_all_user_tokens() {
    let (db, _tmp) = setup_test_db().await;
    let user_a = make_user(&db, "ua").await;
    let user_b = make_user(&db, "ub").await;
    let family_a = Uuid::new_v4();
    let family_b = Uuid::new_v4();

    RefreshTokenRepository::create(&db, input(user_a, family_a, "h-a1", 30))
        .await
        .unwrap();
    RefreshTokenRepository::create(&db, input(user_a, family_a, "h-a2", 30))
        .await
        .unwrap();
    let kept = RefreshTokenRepository::create(&db, input(user_b, family_b, "h-b1", 30))
        .await
        .unwrap();

    let removed = RefreshTokenRepository::delete_by_user_id(&db, user_a)
        .await
        .unwrap();
    assert_eq!(removed, 2);

    assert!(
        RefreshTokenRepository::get_by_hash(&db, "h-a1")
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        RefreshTokenRepository::get_by_id(&db, kept.id)
            .await
            .unwrap()
            .is_some()
    );
}

#[tokio::test]
async fn unique_token_hash_constraint_is_enforced() {
    let (db, _tmp) = setup_test_db().await;
    let user_id = make_user(&db, "uniq").await;

    RefreshTokenRepository::create(&db, input(user_id, Uuid::new_v4(), "h-same", 30))
        .await
        .unwrap();

    let dup =
        RefreshTokenRepository::create(&db, input(user_id, Uuid::new_v4(), "h-same", 30)).await;
    assert!(dup.is_err(), "duplicate token_hash must be rejected");
}
