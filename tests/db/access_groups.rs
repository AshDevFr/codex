#[path = "../common/mod.rs"]
mod common;

// The single-default rule for access groups is a partial unique index written
// as raw SQL, so it is checked on both backends rather than trusted to SeaQuery.

use codex::db::entities::access_groups;
use codex::db::repositories::AccessGroupRepository;
use common::*;
use sea_orm::{ActiveModelTrait, DatabaseConnection, Set};
use uuid::Uuid;

async fn exercise_single_default(db: &DatabaseConnection) {
    let suffix = Uuid::new_v4();
    let a = AccessGroupRepository::create(db, &format!("Default A {suffix}"), None)
        .await
        .unwrap();
    let b = AccessGroupRepository::create(db, &format!("Default B {suffix}"), None)
        .await
        .unwrap();

    AccessGroupRepository::set_default(db, a.id, true)
        .await
        .unwrap();

    // Bypassing set_default, a second default must be refused by the index
    let mut active: access_groups::ActiveModel = b.clone().into();
    active.is_default = Set(true);
    let second = active.update(db).await;

    // Non-default rows are unconstrained
    AccessGroupRepository::set_default(db, a.id, false)
        .await
        .unwrap();
    let c = AccessGroupRepository::create(db, &format!("Default C {suffix}"), None)
        .await
        .unwrap();

    // Clean up before asserting: on the shared PostgreSQL database a default
    // left behind would pull other tests' new users into it.
    for id in [a.id, b.id, c.id] {
        AccessGroupRepository::delete(db, id).await.unwrap();
    }

    assert!(second.is_err(), "a second default group was accepted");
}

#[tokio::test]
async fn test_single_default_group_sqlite() {
    let (db, _t) = setup_test_db().await;
    exercise_single_default(&db).await;
}

#[tokio::test]
#[ignore] // Requires PostgreSQL test database
async fn test_single_default_group_postgres() {
    let Some(db) = setup_test_db_postgres().await else {
        eprintln!("PostgreSQL test database not available, skipping");
        return;
    };
    exercise_single_default(&db).await;
}
