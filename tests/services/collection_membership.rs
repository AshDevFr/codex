//! Membership resolution for rule-backed ("automatic") collections.
//!
//! An automatic collection stores a `SeriesCondition` instead of junction rows,
//! and its members are resolved on every read. These tests pin the resolution
//! semantics, the deliberate blind spots (a rule-backed collection is a view,
//! not a container, so it is absent from every "what contains this series?"
//! query), and that manual collections are completely unaffected.

#[path = "../common/mod.rs"]
mod common;

use codex::api::routes::v1::dto::filter::{
    BoolOperator, FieldOperator, NumberOperator, SeriesCondition, UuidOperator,
};
use codex::db::repositories::{
    CollectionRepository, SeriesMetadataRepository, SeriesRepository, TagRepository,
    UserRepository, UserSeriesRatingRepository,
};
use codex::models::sort::{CollectionSeriesSort, SortDirection};
use codex::services::CollectionMembershipService;
use codex::utils::password;
use common::{create_test_library, create_test_series, create_test_user, setup_test_db};
use sea_orm::DatabaseConnection;
use uuid::Uuid;

fn rule(condition: SeriesCondition) -> serde_json::Value {
    serde_json::to_value(condition).unwrap()
}

fn tag_rule(tag: &str) -> serde_json::Value {
    rule(SeriesCondition::Tag {
        tag: FieldOperator::Is {
            value: tag.to_string(),
        },
    })
}

async fn members(
    db: &DatabaseConnection,
    collection: &codex::db::entities::collections::Model,
    sort: Option<CollectionSeriesSort>,
    user_id: Option<Uuid>,
) -> Vec<String> {
    CollectionMembershipService::members(
        db,
        collection,
        None,
        sort,
        SortDirection::default(),
        user_id,
    )
    .await
    .unwrap()
    .into_iter()
    .map(|s| s.name)
    .collect()
}

#[tokio::test]
async fn tag_rule_resolves_matching_series() {
    let (db, _tmp) = setup_test_db().await;
    let library = create_test_library(&db, "Library", "/lib").await;

    let isekai = create_test_series(&db, &library, "Isekai One").await;
    let also = create_test_series(&db, &library, "Isekai Two").await;
    let other = create_test_series(&db, &library, "Something Else").await;

    for series in [&isekai, &also] {
        TagRepository::set_tags_for_series(&db, series.id, vec!["isekai".to_string()])
            .await
            .unwrap();
    }
    TagRepository::set_tags_for_series(&db, other.id, vec!["mecha".to_string()])
        .await
        .unwrap();

    let collection =
        CollectionRepository::create(&db, "Isekai", None, false, Some(tag_rule("isekai")))
            .await
            .unwrap();

    let names = members(&db, &collection, None, None).await;
    assert_eq!(names, vec!["Isekai One", "Isekai Two"]);
}

/// Editing the rule changes the members on the very next read: there is no
/// refresh step to forget to run.
#[tokio::test]
async fn editing_the_rule_changes_members_immediately() {
    let (db, _tmp) = setup_test_db().await;
    let library = create_test_library(&db, "Library", "/lib").await;

    let a = create_test_series(&db, &library, "Alpha").await;
    let b = create_test_series(&db, &library, "Beta").await;
    TagRepository::set_tags_for_series(&db, a.id, vec!["isekai".to_string()])
        .await
        .unwrap();
    TagRepository::set_tags_for_series(&db, b.id, vec!["mecha".to_string()])
        .await
        .unwrap();

    let collection =
        CollectionRepository::create(&db, "Themed", None, false, Some(tag_rule("isekai")))
            .await
            .unwrap();
    assert_eq!(members(&db, &collection, None, None).await, vec!["Alpha"]);

    let updated = CollectionRepository::update(
        &db,
        collection.id,
        None,
        None,
        None,
        Some(Some(tag_rule("mecha"))),
    )
    .await
    .unwrap()
    .unwrap();

    assert_eq!(members(&db, &updated, None, None).await, vec!["Beta"]);
}

/// A series that starts matching after the collection was created appears with
/// no intervening task, which is the whole point of resolving live.
#[tokio::test]
async fn a_newly_matching_series_appears_without_a_refresh() {
    let (db, _tmp) = setup_test_db().await;
    let library = create_test_library(&db, "Library", "/lib").await;

    let existing = create_test_series(&db, &library, "Existing").await;
    TagRepository::set_tags_for_series(&db, existing.id, vec!["isekai".to_string()])
        .await
        .unwrap();

    let collection =
        CollectionRepository::create(&db, "Isekai", None, false, Some(tag_rule("isekai")))
            .await
            .unwrap();
    assert_eq!(
        members(&db, &collection, None, None).await,
        vec!["Existing"]
    );

    // Simulate a scan importing a matching series.
    let imported = create_test_series(&db, &library, "Imported").await;
    TagRepository::set_tags_for_series(&db, imported.id, vec!["isekai".to_string()])
        .await
        .unwrap();

    assert_eq!(
        members(&db, &collection, None, None).await,
        vec!["Existing", "Imported"]
    );

    // And removing the tag takes it back out.
    TagRepository::set_tags_for_series(&db, imported.id, vec![])
        .await
        .unwrap();
    assert_eq!(
        members(&db, &collection, None, None).await,
        vec!["Existing"]
    );
}

#[tokio::test]
async fn any_of_rule_returns_the_union() {
    let (db, _tmp) = setup_test_db().await;
    let library = create_test_library(&db, "Library", "/lib").await;

    let a = create_test_series(&db, &library, "Alpha").await;
    let b = create_test_series(&db, &library, "Beta").await;
    let c = create_test_series(&db, &library, "Gamma").await;
    TagRepository::set_tags_for_series(&db, a.id, vec!["isekai".to_string()])
        .await
        .unwrap();
    TagRepository::set_tags_for_series(&db, b.id, vec!["reincarnation".to_string()])
        .await
        .unwrap();
    TagRepository::set_tags_for_series(&db, c.id, vec!["mecha".to_string()])
        .await
        .unwrap();

    let condition = rule(SeriesCondition::AnyOf {
        any_of: vec![
            SeriesCondition::Tag {
                tag: FieldOperator::Is {
                    value: "isekai".to_string(),
                },
            },
            SeriesCondition::Tag {
                tag: FieldOperator::Is {
                    value: "reincarnation".to_string(),
                },
            },
        ],
    });
    let collection = CollectionRepository::create(&db, "Isekai", None, false, Some(condition))
        .await
        .unwrap();

    assert_eq!(
        members(&db, &collection, None, None).await,
        vec!["Alpha", "Beta"]
    );
}

#[tokio::test]
async fn library_scoped_rule_only_matches_that_library() {
    let (db, _tmp) = setup_test_db().await;
    let manga = create_test_library(&db, "Manga", "/manga").await;
    let comics = create_test_library(&db, "Comics", "/comics").await;

    create_test_series(&db, &manga, "Manga Series").await;
    create_test_series(&db, &comics, "Comic Series").await;

    let condition = rule(SeriesCondition::LibraryId {
        library_id: UuidOperator::Is { value: manga.id },
    });
    let collection = CollectionRepository::create(&db, "Manga only", None, false, Some(condition))
        .await
        .unwrap();

    assert_eq!(
        members(&db, &collection, None, None).await,
        vec!["Manga Series"]
    );
}

/// A rule over the viewer's own ratings resolves per caller, so two users see
/// different contents. That is the intended behaviour for a "Favourites"
/// collection, not a bug.
#[tokio::test]
async fn a_user_rating_rule_resolves_per_caller() {
    let (db, _tmp) = setup_test_db().await;
    let library = create_test_library(&db, "Library", "/lib").await;

    let alpha = create_test_series(&db, &library, "Alpha").await;
    let beta = create_test_series(&db, &library, "Beta").await;

    let hash = password::hash_password("password123").unwrap();
    let alice = UserRepository::create(
        &db,
        &create_test_user("alice", "alice@example.com", &hash, true),
    )
    .await
    .unwrap();
    let bob = UserRepository::create(
        &db,
        &create_test_user("bob", "bob@example.com", &hash, true),
    )
    .await
    .unwrap();

    UserSeriesRatingRepository::create(&db, alice.id, alpha.id, 95, None)
        .await
        .unwrap();
    UserSeriesRatingRepository::create(&db, bob.id, beta.id, 95, None)
        .await
        .unwrap();

    let condition = rule(SeriesCondition::UserRating {
        user_rating: NumberOperator::Gte { value: 85 },
    });
    let collection = CollectionRepository::create(&db, "Favourites", None, false, Some(condition))
        .await
        .unwrap();

    assert_eq!(
        members(&db, &collection, None, Some(alice.id)).await,
        vec!["Alpha"]
    );
    assert_eq!(
        members(&db, &collection, None, Some(bob.id)).await,
        vec!["Beta"]
    );
    // No caller: nobody has rated anything, so the collection reads as empty
    // rather than leaking another user's picks.
    assert!(members(&db, &collection, None, None).await.is_empty());
}

#[tokio::test]
async fn community_rating_rule_resolves() {
    let (db, _tmp) = setup_test_db().await;
    let library = create_test_library(&db, "Library", "/lib").await;

    let good = create_test_series(&db, &library, "Acclaimed").await;
    let meh = create_test_series(&db, &library, "Mediocre").await;

    let hash = password::hash_password("password123").unwrap();
    let user = UserRepository::create(
        &db,
        &create_test_user("rater", "rater@example.com", &hash, true),
    )
    .await
    .unwrap();
    UserSeriesRatingRepository::create(&db, user.id, good.id, 92, None)
        .await
        .unwrap();
    UserSeriesRatingRepository::create(&db, user.id, meh.id, 40, None)
        .await
        .unwrap();

    let condition = rule(SeriesCondition::CommunityRating {
        community_rating: NumberOperator::Gte { value: 85 },
    });
    let collection = CollectionRepository::create(&db, "Top rated", None, false, Some(condition))
        .await
        .unwrap();

    assert_eq!(
        members(&db, &collection, None, None).await,
        vec!["Acclaimed"]
    );
}

/// `Manual` has no arrangement to honour without a junction, so it degrades to
/// title order rather than returning an arbitrary sequence.
#[tokio::test]
async fn every_supported_sort_works_on_a_rule() {
    let (db, _tmp) = setup_test_db().await;
    let library = create_test_library(&db, "Library", "/lib").await;

    let b = create_test_series(&db, &library, "Bravo").await;
    let a = create_test_series(&db, &library, "Alpha").await;
    let c = create_test_series(&db, &library, "Charlie").await;

    for (series, year) in [(&a, 2020), (&b, 2010), (&c, 2015)] {
        TagRepository::set_tags_for_series(&db, series.id, vec!["pick".to_string()])
            .await
            .unwrap();
        SeriesMetadataRepository::update_year(&db, series.id, Some(year))
            .await
            .unwrap();
    }

    let collection =
        CollectionRepository::create(&db, "Picks", None, false, Some(tag_rule("pick")))
            .await
            .unwrap();

    assert_eq!(
        members(&db, &collection, Some(CollectionSeriesSort::Title), None).await,
        vec!["Alpha", "Bravo", "Charlie"]
    );
    assert_eq!(
        members(&db, &collection, Some(CollectionSeriesSort::Manual), None).await,
        vec!["Alpha", "Bravo", "Charlie"],
        "Manual has no order to honour and falls back to Title"
    );
    assert_eq!(
        members(&db, &collection, Some(CollectionSeriesSort::Year), None).await,
        vec!["Bravo", "Charlie", "Alpha"]
    );
    // Added order is library insertion order for a rule-backed collection,
    // because there is no per-member join date.
    assert_eq!(
        members(&db, &collection, Some(CollectionSeriesSort::Added), None).await,
        vec!["Bravo", "Alpha", "Charlie"]
    );
}

/// Access-group visibility must filter rule-backed members exactly as it filters
/// manual ones.
#[tokio::test]
async fn visibility_filters_rule_members() {
    use codex::db::repositories::visibility::SeriesVisibility;

    let (db, _tmp) = setup_test_db().await;
    let library = create_test_library(&db, "Library", "/lib").await;

    let visible = create_test_series(&db, &library, "Visible").await;
    let hidden = create_test_series(&db, &library, "Hidden").await;
    for series in [&visible, &hidden] {
        TagRepository::set_tags_for_series(&db, series.id, vec!["pick".to_string()])
            .await
            .unwrap();
    }

    let collection =
        CollectionRepository::create(&db, "Picks", None, false, Some(tag_rule("pick")))
            .await
            .unwrap();

    let vis = SeriesVisibility {
        allowed_series_ids: None,
        excluded_series_ids: [hidden.id].into_iter().collect(),
    };
    let names: Vec<String> = CollectionMembershipService::members(
        &db,
        &collection,
        Some(&vis),
        None,
        SortDirection::default(),
        None,
    )
    .await
    .unwrap()
    .into_iter()
    .map(|s| s.name)
    .collect();
    assert_eq!(names, vec!["Visible"]);

    // An empty whitelist hides everything.
    let nothing = SeriesVisibility {
        allowed_series_ids: Some(Default::default()),
        excluded_series_ids: Default::default(),
    };
    assert!(
        CollectionMembershipService::members(
            &db,
            &collection,
            Some(&nothing),
            None,
            SortDirection::default(),
            None,
        )
        .await
        .unwrap()
        .is_empty()
    );
}

/// A malformed stored rule must fail loudly. Reporting an empty collection would
/// be indistinguishable from a rule that simply matches nothing.
#[tokio::test]
async fn a_malformed_rule_is_an_error_not_an_empty_collection() {
    let (db, _tmp) = setup_test_db().await;
    let collection = CollectionRepository::create(
        &db,
        "Broken",
        None,
        false,
        Some(serde_json::json!({"notAField": {"operator": "is", "value": "x"}})),
    )
    .await
    .unwrap();

    let result = CollectionMembershipService::members(
        &db,
        &collection,
        None,
        None,
        SortDirection::default(),
        None,
    )
    .await;
    assert!(result.is_err(), "a malformed rule must not read as empty");
}

// ============================================================================
// A rule-backed collection is a view, not a container
// ============================================================================

#[tokio::test]
async fn auto_collections_are_absent_from_the_reverse_lookup() {
    let (db, _tmp) = setup_test_db().await;
    let library = create_test_library(&db, "Library", "/lib").await;
    let series = create_test_series(&db, &library, "Alpha").await;
    TagRepository::set_tags_for_series(&db, series.id, vec!["pick".to_string()])
        .await
        .unwrap();

    let manual = CollectionRepository::create(&db, "Manual", None, false, None)
        .await
        .unwrap();
    CollectionRepository::add_series(&db, manual.id, series.id)
        .await
        .unwrap();
    let _auto = CollectionRepository::create(&db, "Auto", None, false, Some(tag_rule("pick")))
        .await
        .unwrap();

    let names: Vec<String> = CollectionRepository::get_collections_for_series(&db, series.id)
        .await
        .unwrap()
        .into_iter()
        .map(|c| c.name)
        .collect();
    assert_eq!(names, vec!["Manual"]);

    let batched = CollectionRepository::get_collections_for_series_ids(&db, &[series.id])
        .await
        .unwrap();
    let batched_names: Vec<String> = batched
        .get(&series.id)
        .unwrap()
        .iter()
        .map(|c| c.name.clone())
        .collect();
    assert_eq!(batched_names, vec!["Manual"]);
}

/// `inCollection` search results must be identical before and after automatic
/// collections exist. This is what makes rule recursion impossible.
#[tokio::test]
async fn in_collection_search_ignores_auto_collections() {
    use codex::services::FilterService;
    use std::collections::HashSet;

    let (db, _tmp) = setup_test_db().await;
    let library = create_test_library(&db, "Library", "/lib").await;
    let in_manual = create_test_series(&db, &library, "In Manual").await;
    let rule_only = create_test_series(&db, &library, "Rule Only").await;
    for series in [&in_manual, &rule_only] {
        TagRepository::set_tags_for_series(&db, series.id, vec!["pick".to_string()])
            .await
            .unwrap();
    }

    let manual = CollectionRepository::create(&db, "Manual", None, false, None)
        .await
        .unwrap();
    CollectionRepository::add_series(&db, manual.id, in_manual.id)
        .await
        .unwrap();

    let condition = SeriesCondition::InCollection {
        in_collection: BoolOperator::IsTrue,
    };
    let before = FilterService::get_matching_series(&db, &condition, None)
        .await
        .unwrap();
    assert_eq!(before, HashSet::from([in_manual.id]));

    // The rule matches both series, but membership of an automatic collection is
    // not membership as far as `inCollection` is concerned.
    let _auto = CollectionRepository::create(&db, "Auto", None, false, Some(tag_rule("pick")))
        .await
        .unwrap();

    let after = FilterService::get_matching_series(&db, &condition, None)
        .await
        .unwrap();
    assert_eq!(
        after, before,
        "auto collections must not affect inCollection"
    );
}

// ============================================================================
// Write protection at the repository layer
// ============================================================================

#[tokio::test]
async fn manual_mutation_is_rejected_on_a_rule_backed_collection() {
    let (db, _tmp) = setup_test_db().await;
    let library = create_test_library(&db, "Library", "/lib").await;
    let series = create_test_series(&db, &library, "Alpha").await;

    let auto = CollectionRepository::create(&db, "Auto", None, false, Some(tag_rule("pick")))
        .await
        .unwrap();

    assert!(
        CollectionRepository::add_series(&db, auto.id, series.id)
            .await
            .is_err()
    );
    assert!(
        CollectionRepository::remove_series(&db, auto.id, series.id)
            .await
            .is_err()
    );
    assert!(
        CollectionRepository::reorder(&db, auto.id, &[series.id])
            .await
            .is_err()
    );
}

/// Every mutation still works on a manual collection: the guards must not have
/// caught the common case.
#[tokio::test]
async fn manual_collections_are_unaffected() {
    let (db, _tmp) = setup_test_db().await;
    let library = create_test_library(&db, "Library", "/lib").await;
    let a = create_test_series(&db, &library, "Alpha").await;
    let b = create_test_series(&db, &library, "Beta").await;

    let manual = CollectionRepository::create(&db, "Manual", None, true, None)
        .await
        .unwrap();
    assert!(manual.condition.is_none());
    assert!(manual.ordered, "manual collections keep their ordered flag");

    CollectionRepository::add_series(&db, manual.id, a.id)
        .await
        .unwrap();
    CollectionRepository::add_series(&db, manual.id, b.id)
        .await
        .unwrap();
    CollectionRepository::reorder(&db, manual.id, &[b.id, a.id])
        .await
        .unwrap();

    assert_eq!(
        members(&db, &manual, None, None).await,
        vec!["Beta", "Alpha"],
        "manual order is honoured via the ordered flag"
    );

    assert!(
        CollectionRepository::remove_series(&db, manual.id, a.id)
            .await
            .unwrap()
    );
    assert_eq!(members(&db, &manual, None, None).await, vec!["Beta"]);
}

/// `ordered` is meaningless without a manual arrangement, so it is forced off
/// for rule-backed collections on both create and update.
#[tokio::test]
async fn ordered_is_forced_off_for_rule_backed_collections() {
    let (db, _tmp) = setup_test_db().await;

    let created = CollectionRepository::create(&db, "Auto", None, true, Some(tag_rule("pick")))
        .await
        .unwrap();
    assert!(!created.ordered);

    // Turning a manual ordered collection into an automatic one clears it too.
    let manual = CollectionRepository::create(&db, "Manual", None, true, None)
        .await
        .unwrap();
    assert!(manual.ordered);

    let converted = CollectionRepository::update(
        &db,
        manual.id,
        None,
        None,
        None,
        Some(Some(tag_rule("pick"))),
    )
    .await
    .unwrap()
    .unwrap();
    assert!(!converted.ordered);
    assert!(converted.condition.is_some());
}

/// Clearing the rule converts the collection to manual, and it is empty because
/// it never had junction rows to fall back on.
#[tokio::test]
async fn clearing_the_rule_leaves_an_empty_manual_collection() {
    let (db, _tmp) = setup_test_db().await;
    let library = create_test_library(&db, "Library", "/lib").await;
    let series = create_test_series(&db, &library, "Alpha").await;
    TagRepository::set_tags_for_series(&db, series.id, vec!["pick".to_string()])
        .await
        .unwrap();

    let auto = CollectionRepository::create(&db, "Auto", None, false, Some(tag_rule("pick")))
        .await
        .unwrap();
    assert_eq!(members(&db, &auto, None, None).await, vec!["Alpha"]);

    let manual = CollectionRepository::update(&db, auto.id, None, None, None, Some(None))
        .await
        .unwrap()
        .unwrap();
    assert!(manual.condition.is_none());
    assert!(members(&db, &manual, None, None).await.is_empty());

    // And it accepts manual membership again.
    CollectionRepository::add_series(&db, manual.id, series.id)
        .await
        .unwrap();
    assert_eq!(members(&db, &manual, None, None).await, vec!["Alpha"]);
}

/// A rule matching several thousand series produces a very large `IN (...)`
/// list. SQLite caps bound parameters, so this pins that the read path survives
/// a set well past the historical 999-parameter limit.
#[tokio::test]
async fn a_rule_matching_thousands_of_series_still_reads() {
    let (db, _tmp) = setup_test_db().await;
    let library = create_test_library(&db, "Library", "/lib").await;

    const COUNT: usize = 2500;
    for i in 0..COUNT {
        let series = SeriesRepository::create(&db, library.id, &format!("Series {i:05}"), None)
            .await
            .unwrap();
        TagRepository::set_tags_for_series(&db, series.id, vec!["bulk".to_string()])
            .await
            .unwrap();
    }

    let collection = CollectionRepository::create(&db, "Bulk", None, false, Some(tag_rule("bulk")))
        .await
        .unwrap();

    let resolved = CollectionMembershipService::members(
        &db,
        &collection,
        None,
        Some(CollectionSeriesSort::Title),
        SortDirection::default(),
        None,
    )
    .await
    .unwrap();

    assert_eq!(resolved.len(), COUNT);
    assert_eq!(resolved[0].name, "Series 00000");
    assert_eq!(resolved[COUNT - 1].name, format!("Series {:05}", COUNT - 1));
}
