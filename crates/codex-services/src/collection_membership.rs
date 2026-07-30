//! Resolving what a collection contains.
//!
//! A collection's members come from one of two places, and every read path goes
//! through here so it does not have to know which:
//!
//! * **Manual** (`condition IS NULL`): the `collection_series` junction, with
//!   the order the user arranged.
//! * **Automatic** (`condition IS NOT NULL`): a stored `SeriesCondition`,
//!   evaluated against the library on every read.
//!
//! Automatic membership is never materialized. The collection *is* the query, so
//! the query is what gets stored: a series that starts matching appears on the
//! next read with no refresh step, no staleness window and no cache to
//! invalidate.
//!
//! This lives in `codex-services` rather than `codex-db` because the crate graph
//! only points one way: `codex-services` depends on `codex-db`, so
//! `CollectionRepository` cannot reach [`FilterService`]. The repository gained
//! the ability to sort and paginate a caller-supplied id set
//! ([`CollectionRepository::get_series_by_ids`]); deciding *which* ids happens
//! here.

use anyhow::{Context, Result};
use codex_db::entities::{collections, series};
use codex_db::repositories::CollectionRepository;
use codex_db::repositories::visibility::SeriesVisibility;
use codex_models::filter::SeriesCondition;
use codex_models::sort::{CollectionSeriesSort, SortDirection};
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use crate::filter::FilterService;

/// Resolves collection membership for both manual and rule-backed collections.
pub struct CollectionMembershipService;

impl CollectionMembershipService {
    /// `true` when this collection's membership comes from a rule.
    pub fn is_automatic(collection: &collections::Model) -> bool {
        collection.condition.is_some()
    }

    /// Parse the stored rule, if there is one.
    ///
    /// Fails loudly rather than treating an unparseable rule as "no members":
    /// silently returning an empty collection would look exactly like a rule
    /// that matches nothing, and the administrator would have no way to tell
    /// their collection is broken rather than just too narrow.
    pub fn rule(collection: &collections::Model) -> Result<Option<SeriesCondition>> {
        let Some(raw) = &collection.condition else {
            return Ok(None);
        };
        let condition: SeriesCondition =
            serde_json::from_value(raw.clone()).with_context(|| {
                format!(
                    "collection {} has a stored rule that is not a valid SeriesCondition",
                    collection.id
                )
            })?;
        Ok(Some(condition))
    }

    /// The series a collection contains, in the requested order, filtered by the
    /// caller's visibility.
    ///
    /// `user_id` is the caller, and it matters: a rule may reference the
    /// viewer's own data (`userRating`, `readStatus`, `hasUserRating`), in which
    /// case the same collection legitimately holds different series for
    /// different people. Passing `None` resolves those sub-conditions as
    /// "nobody", which is the right answer for an unauthenticated feed.
    pub async fn members(
        db: &DatabaseConnection,
        collection: &collections::Model,
        vis: Option<&SeriesVisibility>,
        sort: Option<CollectionSeriesSort>,
        direction: SortDirection,
        user_id: Option<Uuid>,
    ) -> Result<Vec<series::Model>> {
        let Some(condition) = Self::rule(collection)? else {
            return CollectionRepository::get_series(db, collection, vis, sort, direction).await;
        };

        let matching = FilterService::get_matching_series_for_user(db, &condition, None, user_id)
            .await
            .with_context(|| format!("failed to resolve rule for collection {}", collection.id))?;

        if matching.is_empty() {
            return Ok(vec![]);
        }

        // `ordered` is forced off for rule-backed collections, so the default
        // sort is title. An explicit `Manual` has no arrangement to honour and
        // degrades to the same thing.
        let sort = sort.unwrap_or(CollectionSeriesSort::Title);
        let ids: Vec<Uuid> = matching.into_iter().collect();

        CollectionRepository::get_series_by_ids(db, &ids, vis, sort, direction).await
    }

    /// How many series a collection contains for this caller.
    ///
    /// Only meaningful to call when you actually need the number. For a
    /// rule-backed collection this resolves the whole rule, so list endpoints
    /// deliberately do not call it: rendering one page of collections would mean
    /// running every rule on the server.
    pub async fn count(
        db: &DatabaseConnection,
        collection: &collections::Model,
        vis: Option<&SeriesVisibility>,
        user_id: Option<Uuid>,
    ) -> Result<u64> {
        if !Self::is_automatic(collection) {
            return CollectionRepository::count_series(db, collection.id, vis).await;
        }
        let members = Self::members(
            db,
            collection,
            vis,
            Some(CollectionSeriesSort::Title),
            SortDirection::default(),
            user_id,
        )
        .await?;
        Ok(members.len() as u64)
    }
}

/// Why a proposed collection rule was rejected.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum RuleError {
    /// `inCollection` anywhere in the tree.
    #[error(
        "the `inCollection` field cannot be used in an automatic collection's rule: automatic \
         collections are views over the library rather than members of other collections, so they \
         are invisible to collection-membership filters"
    )]
    InCollection,
    /// A top-level group with no children.
    #[error(
        "an automatic collection's rule must contain at least one condition: an empty `allOf` \
         would match the entire library and an empty `anyOf` would match nothing"
    )]
    Empty,
}

impl CollectionMembershipService {
    /// Check that a condition is usable as a collection rule.
    ///
    /// Two rejections, both about rules that are technically valid conditions but
    /// nonsense as a collection definition:
    ///
    /// * `inCollection` at any depth. Rule-backed collections are deliberately
    ///   invisible to that filter (see
    ///   [`CollectionRepository::all_member_series_ids`]), so a rule using it
    ///   would silently mean "in a *manual* collection", which is not what
    ///   anyone writing it intends. Rejecting also keeps rule recursion
    ///   impossible by construction rather than by accident.
    /// * An empty top-level group. An empty `allOf` matches the whole library and
    ///   an empty `anyOf` matches nothing; neither is a collection anybody meant
    ///   to create, and both are easy to produce by clearing a rule editor.
    ///
    /// User-dependent conditions (`userRating`, `readStatus`, `hasUserRating`)
    /// are deliberately allowed: they are the point of a personal "Favourites"
    /// collection. The UI labels them rather than preventing them.
    pub fn validate_rule(condition: &SeriesCondition) -> Result<(), RuleError> {
        match condition {
            SeriesCondition::AllOf { all_of } if all_of.is_empty() => return Err(RuleError::Empty),
            SeriesCondition::AnyOf { any_of } if any_of.is_empty() => return Err(RuleError::Empty),
            _ => {}
        }
        Self::reject_in_collection(condition)
    }

    /// Walk the whole tree looking for `inCollection`, however deeply nested.
    fn reject_in_collection(condition: &SeriesCondition) -> Result<(), RuleError> {
        match condition {
            SeriesCondition::InCollection { .. } => Err(RuleError::InCollection),
            SeriesCondition::AllOf { all_of } => {
                for child in all_of {
                    Self::reject_in_collection(child)?;
                }
                Ok(())
            }
            SeriesCondition::AnyOf { any_of } => {
                for child in any_of {
                    Self::reject_in_collection(child)?;
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_models::filter::{BoolOperator, FieldOperator};

    fn tag(name: &str) -> SeriesCondition {
        SeriesCondition::Tag {
            tag: FieldOperator::Is {
                value: name.to_string(),
            },
        }
    }

    fn in_collection() -> SeriesCondition {
        SeriesCondition::InCollection {
            in_collection: BoolOperator::IsTrue,
        }
    }

    #[test]
    fn accepts_an_ordinary_rule() {
        assert!(CollectionMembershipService::validate_rule(&tag("isekai")).is_ok());
        assert!(
            CollectionMembershipService::validate_rule(&SeriesCondition::AnyOf {
                any_of: vec![tag("isekai"), tag("reincarnation")],
            })
            .is_ok()
        );
    }

    #[test]
    fn rejects_a_bare_in_collection() {
        assert_eq!(
            CollectionMembershipService::validate_rule(&in_collection()),
            Err(RuleError::InCollection)
        );
    }

    /// The whole tree is walked, so burying it a few groups deep does not get it
    /// past the check.
    #[test]
    fn rejects_a_deeply_nested_in_collection() {
        let deep = SeriesCondition::AllOf {
            all_of: vec![
                tag("isekai"),
                SeriesCondition::AnyOf {
                    any_of: vec![
                        tag("mecha"),
                        SeriesCondition::AllOf {
                            all_of: vec![in_collection()],
                        },
                    ],
                },
            ],
        };
        assert_eq!(
            CollectionMembershipService::validate_rule(&deep),
            Err(RuleError::InCollection)
        );
    }

    #[test]
    fn rejects_empty_top_level_groups() {
        assert_eq!(
            CollectionMembershipService::validate_rule(&SeriesCondition::AllOf { all_of: vec![] }),
            Err(RuleError::Empty)
        );
        assert_eq!(
            CollectionMembershipService::validate_rule(&SeriesCondition::AnyOf { any_of: vec![] }),
            Err(RuleError::Empty)
        );
    }

    /// A nested empty group is allowed: it is redundant rather than dangerous,
    /// and the top-level rule still constrains the result.
    #[test]
    fn allows_a_nested_empty_group() {
        let condition = SeriesCondition::AllOf {
            all_of: vec![tag("isekai"), SeriesCondition::AnyOf { any_of: vec![] }],
        };
        assert!(CollectionMembershipService::validate_rule(&condition).is_ok());
    }

    /// User-dependent rules are the point of a personal collection, not an error.
    #[test]
    fn allows_user_dependent_rules() {
        use codex_models::filter::NumberOperator;
        let condition = SeriesCondition::UserRating {
            user_rating: NumberOperator::Gte { value: 85 },
        };
        assert!(CollectionMembershipService::validate_rule(&condition).is_ok());
    }

    #[test]
    fn error_messages_name_the_offending_field() {
        assert!(RuleError::InCollection.to_string().contains("inCollection"));
        assert!(RuleError::Empty.to_string().contains("allOf"));
        assert!(RuleError::Empty.to_string().contains("anyOf"));
    }
}
