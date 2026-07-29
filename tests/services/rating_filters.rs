//! Rating condition evaluation, exercised directly against `FilterService`.
//!
//! These live outside the API tests because the point is the SQL the filter
//! engine emits, not the HTTP surface. `communityRating` compares
//! `AVG(user_series_ratings.rating)` in a `HAVING` clause, and AVG over an
//! integer column has a different result type on each backend (NUMERIC on
//! PostgreSQL, REAL on SQLite). Every assertion below therefore runs on both.

#[path = "../common/mod.rs"]
mod common;

use codex::db::repositories::{UserRepository, UserSeriesRatingRepository};
use codex::services::FilterService;
use codex::utils::password;
use common::{
    create_test_library, create_test_series, create_test_user, setup_test_db,
    setup_test_db_postgres,
};
use sea_orm::DatabaseConnection;
use std::collections::HashSet;
use uuid::Uuid;

use codex::api::routes::v1::dto::filter::{NumberOperator, SeriesCondition};

struct Fixture {
    /// `[high, mid, low, unrated]`
    series: Vec<Uuid>,
    alice: Uuid,
    bob: Uuid,
}

/// Two users and four series, with:
///   high    alice 100, bob 80  -> average 90
///   mid     alice  90, bob 80  -> average 85 (exactly on the boundary)
///   low     alice  40          -> average 40
///   unrated no ratings         -> no average at all
async fn seed(db: &DatabaseConnection) -> Fixture {
    let library = create_test_library(db, "Library", "/lib").await;

    let mut series = Vec::new();
    for name in ["High", "Mid", "Low", "Unrated"] {
        series.push(create_test_series(db, &library, name).await.id);
    }

    let hash = password::hash_password("password123").unwrap();
    let alice = UserRepository::create(
        db,
        &create_test_user("alice", "alice@example.com", &hash, true),
    )
    .await
    .unwrap()
    .id;
    let bob = UserRepository::create(db, &create_test_user("bob", "bob@example.com", &hash, true))
        .await
        .unwrap()
        .id;

    for (user, target, rating) in [
        (alice, series[0], 100),
        (bob, series[0], 80),
        (alice, series[1], 90),
        (bob, series[1], 80),
        (alice, series[2], 40),
    ] {
        UserSeriesRatingRepository::create(db, user, target, rating, None)
            .await
            .unwrap();
    }

    Fixture { series, alice, bob }
}

async fn matching(
    db: &DatabaseConnection,
    condition: &SeriesCondition,
    user_id: Option<Uuid>,
) -> HashSet<Uuid> {
    FilterService::get_matching_series_for_user(db, condition, None, user_id)
        .await
        .unwrap()
}

fn community(operator: NumberOperator) -> SeriesCondition {
    SeriesCondition::CommunityRating {
        community_rating: operator,
    }
}

fn user_rating(operator: NumberOperator) -> SeriesCondition {
    SeriesCondition::UserRating {
        user_rating: operator,
    }
}

/// Every assertion that must hold identically on both backends.
async fn assert_rating_semantics(db: &DatabaseConnection) {
    let f = seed(db).await;
    let (high, mid, low, unrated) = (f.series[0], f.series[1], f.series[2], f.series[3]);

    // --- communityRating: the average, not any individual rating ------------
    // Alice rated "Mid" 90, but its average is 85, so a `> 85` filter must not
    // return it. This is the assertion that would fail if the HAVING clause
    // were comparing a raw rating instead of the aggregate.
    let above = matching(db, &community(NumberOperator::Gt { value: 85 }), None).await;
    assert_eq!(above, HashSet::from([high]), "communityRating > 85");

    let at_least = matching(db, &community(NumberOperator::Gte { value: 85 }), None).await;
    assert_eq!(
        at_least,
        HashSet::from([high, mid]),
        "communityRating >= 85 must include the boundary"
    );

    let at_most = matching(db, &community(NumberOperator::Lte { value: 85 }), None).await;
    assert_eq!(
        at_most,
        HashSet::from([mid, low]),
        "communityRating <= 85 must exclude the unrated series"
    );

    let ranged = matching(
        db,
        &community(NumberOperator::Between {
            min: Some(80),
            max: Some(89),
        }),
        None,
    )
    .await;
    assert_eq!(ranged, HashSet::from([mid]), "communityRating 80..=89");

    // An unrated series has no average, so it is excluded by every comparison
    // and returned only by isNull.
    let any_average = matching(db, &community(NumberOperator::IsNotNull), None).await;
    assert_eq!(any_average, HashSet::from([high, mid, low]));

    let no_average = matching(db, &community(NumberOperator::IsNull), None).await;
    assert_eq!(no_average, HashSet::from([unrated]));

    // The community average is a server-wide fact: identical for every caller,
    // and identical with no caller at all.
    for caller in [None, Some(f.alice), Some(f.bob)] {
        let seen = matching(db, &community(NumberOperator::Gte { value: 85 }), caller).await;
        assert_eq!(
            seen,
            HashSet::from([high, mid]),
            "communityRating must not depend on the caller"
        );
    }

    // --- userRating: per-caller --------------------------------------------
    let alice_top = matching(
        db,
        &user_rating(NumberOperator::Gte { value: 85 }),
        Some(f.alice),
    )
    .await;
    assert_eq!(
        alice_top,
        HashSet::from([high, mid]),
        "alice rated 100 / 90"
    );

    let bob_top = matching(
        db,
        &user_rating(NumberOperator::Gte { value: 85 }),
        Some(f.bob),
    )
    .await;
    assert_eq!(bob_top, HashSet::new(), "bob's highest rating is 80");

    // `ne` compares an existing rating, so series the caller never rated stay
    // out rather than flooding in.
    let alice_not_100 = matching(
        db,
        &user_rating(NumberOperator::Ne { value: 100 }),
        Some(f.alice),
    )
    .await;
    assert_eq!(alice_not_100, HashSet::from([mid, low]));

    let alice_rated = matching(db, &user_rating(NumberOperator::IsNotNull), Some(f.alice)).await;
    assert_eq!(alice_rated, HashSet::from([high, mid, low]));

    let alice_unrated = matching(db, &user_rating(NumberOperator::IsNull), Some(f.alice)).await;
    assert_eq!(alice_unrated, HashSet::from([unrated]));

    // Without a caller there are no ratings at all: comparisons and isNotNull
    // return nothing, isNull returns everything.
    let anonymous = matching(db, &user_rating(NumberOperator::Gte { value: 1 }), None).await;
    assert_eq!(anonymous, HashSet::new());

    let anonymous_rated = matching(db, &user_rating(NumberOperator::IsNotNull), None).await;
    assert_eq!(anonymous_rated, HashSet::new());

    let anonymous_unrated = matching(db, &user_rating(NumberOperator::IsNull), None).await;
    assert_eq!(
        anonymous_unrated,
        HashSet::from([high, mid, low, unrated]),
        "an anonymous caller has rated nothing, so everything is unrated"
    );
}

#[tokio::test]
async fn rating_conditions_on_sqlite() {
    let (db, _temp_dir) = setup_test_db().await;
    assert_rating_semantics(&db).await;
}

/// The same assertions on PostgreSQL, where `AVG` over an integer column
/// returns NUMERIC rather than REAL. Skipped when no test database is running.
#[tokio::test]
#[ignore]
async fn rating_conditions_on_postgres() {
    let Some(db) = setup_test_db_postgres().await else {
        return;
    };
    assert_rating_semantics(&db).await;
}
