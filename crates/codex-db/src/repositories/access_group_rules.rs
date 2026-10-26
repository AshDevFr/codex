//! Access group rules: allow/deny on sharing tags, regular tags, genres and
//! libraries, plus the per-type "any"/"all" switches.
//!
//! This module only stores rules and resolves rule targets to series IDs. How
//! rules from several groups combine into what a user can see lives in
//! `ContentFilter` (`codex-services`).
//!
//! Tag and genre rules are keyed by normalized name (see the migration that
//! creates their tables for why), so resolving them joins on
//! `tags.normalized_name` / `genres.normalized_name` at query time.

use anyhow::{Result, bail};
use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, QueryFilter,
    QueryOrder, QuerySelect, Set, TransactionTrait,
};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

use crate::entities::{
    access_group_genre_rules, access_group_library_rules, access_group_sharing_tags,
    access_group_tag_rules, access_groups, access_groups::RuleMatch, genres, series, series_genres,
    series_sharing_tags, series_tags, tags, user_sharing_tags::AccessMode,
};

/// Normalize a tag or genre name the way the `tags` and `genres` tables do.
pub fn normalize_taxonomy_name(name: &str) -> String {
    name.trim().to_lowercase()
}

/// A rule on a sharing tag.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SharingTagRule {
    pub sharing_tag_id: Uuid,
    pub access_mode: AccessMode,
}

/// A rule on a regular tag or a genre, by name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NameRule {
    /// Display name; matching uses [`normalize_taxonomy_name`] of it
    pub name: String,
    pub access_mode: AccessMode,
}

impl NameRule {
    pub fn new(name: impl Into<String>, access_mode: AccessMode) -> Self {
        Self {
            name: name.into(),
            access_mode,
        }
    }

    pub fn normalized_name(&self) -> String {
        normalize_taxonomy_name(&self.name)
    }
}

/// A rule on a library.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryRule {
    pub library_id: Uuid,
    pub access_mode: AccessMode,
}

/// Every rule an access group holds.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AccessGroupRules {
    pub sharing_tag_match: RuleMatch,
    pub tag_match: RuleMatch,
    pub genre_match: RuleMatch,
    pub sharing_tags: Vec<SharingTagRule>,
    pub tags: Vec<NameRule>,
    pub genres: Vec<NameRule>,
    pub libraries: Vec<LibraryRule>,
}

impl AccessGroupRules {
    /// Whether the group has any allow rule, of any type.
    pub fn has_allows(&self) -> bool {
        self.sharing_tags
            .iter()
            .any(|r| r.access_mode == AccessMode::Allow)
            || self.tags.iter().any(|r| r.access_mode == AccessMode::Allow)
            || self
                .genres
                .iter()
                .any(|r| r.access_mode == AccessMode::Allow)
            || self
                .libraries
                .iter()
                .any(|r| r.access_mode == AccessMode::Allow)
    }

    /// Check the rule set before storing it.
    ///
    /// Each target may appear once per group (one row per target in the
    /// tables), and names must not be blank.
    pub fn validate(&self) -> std::result::Result<(), String> {
        let mut seen = HashSet::new();
        for rule in &self.sharing_tags {
            if !seen.insert(rule.sharing_tag_id) {
                return Err(format!(
                    "Sharing tag {} appears more than once",
                    rule.sharing_tag_id
                ));
            }
        }
        for (kind, rules) in [("tag", &self.tags), ("genre", &self.genres)] {
            let mut seen = HashSet::new();
            for rule in rules {
                let normalized = rule.normalized_name();
                if normalized.is_empty() {
                    return Err(format!("A {kind} rule has an empty name"));
                }
                if !seen.insert(normalized) {
                    return Err(format!(
                        "The {kind} '{}' appears more than once",
                        rule.name.trim()
                    ));
                }
            }
        }
        let mut seen = HashSet::new();
        for rule in &self.libraries {
            if !seen.insert(rule.library_id) {
                return Err(format!(
                    "Library {} appears more than once",
                    rule.library_id
                ));
            }
        }
        Ok(())
    }
}

pub struct AccessGroupRulesRepository;

impl AccessGroupRulesRepository {
    /// All rules of one group. Returns `None` if the group does not exist.
    pub async fn get(db: &DatabaseConnection, group_id: Uuid) -> Result<Option<AccessGroupRules>> {
        Ok(Self::load_for_groups(db, &[group_id])
            .await?
            .remove(&group_id))
    }

    /// Rules for many groups in a fixed number of queries, keyed by group ID.
    /// Groups that do not exist are absent from the map.
    pub async fn load_for_groups(
        db: &impl ConnectionTrait,
        group_ids: &[Uuid],
    ) -> Result<HashMap<Uuid, AccessGroupRules>> {
        if group_ids.is_empty() {
            return Ok(HashMap::new());
        }
        let ids = group_ids.to_vec();

        let mut by_group: HashMap<Uuid, AccessGroupRules> = access_groups::Entity::find()
            .filter(access_groups::Column::Id.is_in(ids.clone()))
            .all(db)
            .await?
            .into_iter()
            .map(|g| {
                (
                    g.id,
                    AccessGroupRules {
                        sharing_tag_match: g.get_sharing_tag_match(),
                        tag_match: g.get_tag_match(),
                        genre_match: g.get_genre_match(),
                        ..Default::default()
                    },
                )
            })
            .collect();

        for row in access_group_sharing_tags::Entity::find()
            .filter(access_group_sharing_tags::Column::AccessGroupId.is_in(ids.clone()))
            .order_by_asc(access_group_sharing_tags::Column::CreatedAt)
            .all(db)
            .await?
        {
            if let Some(rules) = by_group.get_mut(&row.access_group_id) {
                rules.sharing_tags.push(SharingTagRule {
                    sharing_tag_id: row.sharing_tag_id,
                    access_mode: row.get_access_mode(),
                });
            }
        }

        for row in access_group_tag_rules::Entity::find()
            .filter(access_group_tag_rules::Column::AccessGroupId.is_in(ids.clone()))
            .order_by_asc(access_group_tag_rules::Column::Name)
            .all(db)
            .await?
        {
            if let Some(rules) = by_group.get_mut(&row.access_group_id) {
                let access_mode = row.get_access_mode();
                rules.tags.push(NameRule::new(row.name, access_mode));
            }
        }

        for row in access_group_genre_rules::Entity::find()
            .filter(access_group_genre_rules::Column::AccessGroupId.is_in(ids.clone()))
            .order_by_asc(access_group_genre_rules::Column::Name)
            .all(db)
            .await?
        {
            if let Some(rules) = by_group.get_mut(&row.access_group_id) {
                let access_mode = row.get_access_mode();
                rules.genres.push(NameRule::new(row.name, access_mode));
            }
        }

        for row in access_group_library_rules::Entity::find()
            .filter(access_group_library_rules::Column::AccessGroupId.is_in(ids))
            .order_by_asc(access_group_library_rules::Column::CreatedAt)
            .all(db)
            .await?
        {
            if let Some(rules) = by_group.get_mut(&row.access_group_id) {
                rules.libraries.push(LibraryRule {
                    library_id: row.library_id,
                    access_mode: row.get_access_mode(),
                });
            }
        }

        Ok(by_group)
    }

    /// Replace every rule of a group, atomically.
    ///
    /// Callers should [`AccessGroupRules::validate`] first; a duplicate target
    /// is also refused by the tables' unique indexes.
    pub async fn replace(
        db: &DatabaseConnection,
        group_id: Uuid,
        rules: &AccessGroupRules,
    ) -> Result<()> {
        if let Err(message) = rules.validate() {
            bail!(message);
        }

        let txn = db.begin().await?;
        let group = access_groups::Entity::find_by_id(group_id)
            .one(&txn)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Access group not found"))?;

        access_group_sharing_tags::Entity::delete_many()
            .filter(access_group_sharing_tags::Column::AccessGroupId.eq(group_id))
            .exec(&txn)
            .await?;
        access_group_tag_rules::Entity::delete_many()
            .filter(access_group_tag_rules::Column::AccessGroupId.eq(group_id))
            .exec(&txn)
            .await?;
        access_group_genre_rules::Entity::delete_many()
            .filter(access_group_genre_rules::Column::AccessGroupId.eq(group_id))
            .exec(&txn)
            .await?;
        access_group_library_rules::Entity::delete_many()
            .filter(access_group_library_rules::Column::AccessGroupId.eq(group_id))
            .exec(&txn)
            .await?;

        let now = Utc::now();
        for rule in &rules.sharing_tags {
            access_group_sharing_tags::ActiveModel {
                id: Set(Uuid::new_v4()),
                access_group_id: Set(group_id),
                sharing_tag_id: Set(rule.sharing_tag_id),
                access_mode: Set(rule.access_mode.as_str().to_string()),
                created_at: Set(now),
            }
            .insert(&txn)
            .await?;
        }
        for rule in &rules.tags {
            access_group_tag_rules::ActiveModel {
                id: Set(Uuid::new_v4()),
                access_group_id: Set(group_id),
                name: Set(rule.name.trim().to_string()),
                normalized_name: Set(rule.normalized_name()),
                access_mode: Set(rule.access_mode.as_str().to_string()),
                created_at: Set(now),
            }
            .insert(&txn)
            .await?;
        }
        for rule in &rules.genres {
            access_group_genre_rules::ActiveModel {
                id: Set(Uuid::new_v4()),
                access_group_id: Set(group_id),
                name: Set(rule.name.trim().to_string()),
                normalized_name: Set(rule.normalized_name()),
                access_mode: Set(rule.access_mode.as_str().to_string()),
                created_at: Set(now),
            }
            .insert(&txn)
            .await?;
        }
        for rule in &rules.libraries {
            access_group_library_rules::ActiveModel {
                id: Set(Uuid::new_v4()),
                access_group_id: Set(group_id),
                library_id: Set(rule.library_id),
                access_mode: Set(rule.access_mode.as_str().to_string()),
                created_at: Set(now),
            }
            .insert(&txn)
            .await?;
        }

        let mut active: access_groups::ActiveModel = group.into();
        active.sharing_tag_match = Set(rules.sharing_tag_match.as_str().to_string());
        active.tag_match = Set(rules.tag_match.as_str().to_string());
        active.genre_match = Set(rules.genre_match.as_str().to_string());
        active.updated_at = Set(now);
        active.update(&txn).await?;

        txn.commit().await?;
        crate::access_control_version::bump();
        Ok(())
    }

    // ==================== Resolving targets to series ====================

    /// Series carrying each of the given sharing tags, keyed by tag ID.
    pub async fn series_by_sharing_tag(
        db: &impl ConnectionTrait,
        sharing_tag_ids: &[Uuid],
    ) -> Result<HashMap<Uuid, HashSet<Uuid>>> {
        let mut out: HashMap<Uuid, HashSet<Uuid>> = HashMap::new();
        if sharing_tag_ids.is_empty() {
            return Ok(out);
        }
        for row in series_sharing_tags::Entity::find()
            .filter(series_sharing_tags::Column::SharingTagId.is_in(sharing_tag_ids.to_vec()))
            .all(db)
            .await?
        {
            out.entry(row.sharing_tag_id)
                .or_default()
                .insert(row.series_id);
        }
        Ok(out)
    }

    /// Series carrying each of the given tags, keyed by normalized tag name.
    pub async fn series_by_tag_name(
        db: &impl ConnectionTrait,
        normalized_names: &[String],
    ) -> Result<HashMap<String, HashSet<Uuid>>> {
        let mut out: HashMap<String, HashSet<Uuid>> = HashMap::new();
        if normalized_names.is_empty() {
            return Ok(out);
        }
        let rows: Vec<(Uuid, String)> = series_tags::Entity::find()
            .select_only()
            .column(series_tags::Column::SeriesId)
            .column(tags::Column::NormalizedName)
            .inner_join(tags::Entity)
            .filter(tags::Column::NormalizedName.is_in(normalized_names.to_vec()))
            .into_tuple()
            .all(db)
            .await?;
        for (series_id, name) in rows {
            out.entry(name).or_default().insert(series_id);
        }
        Ok(out)
    }

    /// Series carrying each of the given genres, keyed by normalized genre name.
    pub async fn series_by_genre_name(
        db: &impl ConnectionTrait,
        normalized_names: &[String],
    ) -> Result<HashMap<String, HashSet<Uuid>>> {
        let mut out: HashMap<String, HashSet<Uuid>> = HashMap::new();
        if normalized_names.is_empty() {
            return Ok(out);
        }
        let rows: Vec<(Uuid, String)> = series_genres::Entity::find()
            .select_only()
            .column(series_genres::Column::SeriesId)
            .column(genres::Column::NormalizedName)
            .inner_join(genres::Entity)
            .filter(genres::Column::NormalizedName.is_in(normalized_names.to_vec()))
            .into_tuple()
            .all(db)
            .await?;
        for (series_id, name) in rows {
            out.entry(name).or_default().insert(series_id);
        }
        Ok(out)
    }

    /// Series in each of the given libraries, keyed by library ID.
    pub async fn series_by_library(
        db: &impl ConnectionTrait,
        library_ids: &[Uuid],
    ) -> Result<HashMap<Uuid, HashSet<Uuid>>> {
        let mut out: HashMap<Uuid, HashSet<Uuid>> = HashMap::new();
        if library_ids.is_empty() {
            return Ok(out);
        }
        let rows: Vec<(Uuid, Uuid)> = series::Entity::find()
            .select_only()
            .column(series::Column::Id)
            .column(series::Column::LibraryId)
            .filter(series::Column::LibraryId.is_in(library_ids.to_vec()))
            .into_tuple()
            .all(db)
            .await?;
        for (series_id, library_id) in rows {
            out.entry(library_id).or_default().insert(series_id);
        }
        Ok(out)
    }
}

impl AccessGroupRulesRepository {
    /// The libraries that hold the given series. Queried in chunks so a large
    /// whitelist stays under the database's bound-parameter limit.
    pub async fn library_ids_of_series(
        db: &impl ConnectionTrait,
        series_ids: &[Uuid],
    ) -> Result<HashSet<Uuid>> {
        let mut out = HashSet::new();
        for chunk in series_ids.chunks(500) {
            let rows: Vec<Uuid> = series::Entity::find()
                .select_only()
                .column(series::Column::LibraryId)
                .distinct()
                .filter(series::Column::Id.is_in(chunk.to_vec()))
                .into_tuple()
                .all(db)
                .await?;
            out.extend(rows);
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ScanningStrategy;
    use crate::repositories::{AccessGroupRepository, GenreRepository, LibraryRepository};
    use crate::test_helpers::setup_test_db;

    #[test]
    fn validate_rejects_duplicate_names_after_normalizing() {
        let rules = AccessGroupRules {
            genres: vec![
                NameRule::new("Adult", AccessMode::Deny),
                NameRule::new(" adult ", AccessMode::Allow),
            ],
            ..Default::default()
        };
        assert!(rules.validate().is_err());
    }

    #[test]
    fn validate_rejects_blank_names() {
        let rules = AccessGroupRules {
            tags: vec![NameRule::new("  ", AccessMode::Deny)],
            ..Default::default()
        };
        assert!(rules.validate().is_err());
    }

    #[test]
    fn has_allows_ignores_denies() {
        let deny_only = AccessGroupRules {
            genres: vec![NameRule::new("Adult", AccessMode::Deny)],
            ..Default::default()
        };
        assert!(!deny_only.has_allows());

        let with_allow = AccessGroupRules {
            libraries: vec![LibraryRule {
                library_id: Uuid::new_v4(),
                access_mode: AccessMode::Allow,
            }],
            ..deny_only
        };
        assert!(with_allow.has_allows());
    }

    #[tokio::test]
    async fn replace_and_get_round_trip() {
        let db = setup_test_db().await;
        let group = AccessGroupRepository::create(&db, "Kids", None)
            .await
            .unwrap();
        let library =
            LibraryRepository::create(&db, "Kids", "/tmp/kids", ScanningStrategy::Default)
                .await
                .unwrap();

        let rules = AccessGroupRules {
            genre_match: RuleMatch::All,
            tags: vec![NameRule::new("Gore", AccessMode::Deny)],
            genres: vec![
                NameRule::new("Adult", AccessMode::Deny),
                NameRule::new("Kids", AccessMode::Allow),
            ],
            libraries: vec![LibraryRule {
                library_id: library.id,
                access_mode: AccessMode::Allow,
            }],
            ..Default::default()
        };
        AccessGroupRulesRepository::replace(&db, group.id, &rules)
            .await
            .unwrap();

        let stored = AccessGroupRulesRepository::get(&db, group.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(stored, rules);

        // Replacing again drops what is not in the new set
        AccessGroupRulesRepository::replace(&db, group.id, &AccessGroupRules::default())
            .await
            .unwrap();
        let stored = AccessGroupRulesRepository::get(&db, group.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(stored, AccessGroupRules::default());
    }

    #[tokio::test]
    async fn replace_refuses_an_invalid_rule_set_and_keeps_the_old_one() {
        let db = setup_test_db().await;
        let group = AccessGroupRepository::create(&db, "Kids", None)
            .await
            .unwrap();
        let original = AccessGroupRules {
            genres: vec![NameRule::new("Adult", AccessMode::Deny)],
            ..Default::default()
        };
        AccessGroupRulesRepository::replace(&db, group.id, &original)
            .await
            .unwrap();

        let invalid = AccessGroupRules {
            genres: vec![
                NameRule::new("Kids", AccessMode::Allow),
                NameRule::new("KIDS", AccessMode::Deny),
            ],
            ..Default::default()
        };
        assert!(
            AccessGroupRulesRepository::replace(&db, group.id, &invalid)
                .await
                .is_err()
        );

        let stored = AccessGroupRulesRepository::get(&db, group.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(stored, original);
    }

    /// The reason genre rules are keyed by name: cleanup deletes unused
    /// genres, and the rule must still apply when the genre comes back.
    #[tokio::test]
    async fn genre_rule_survives_genre_cleanup() {
        let db = setup_test_db().await;
        let group = AccessGroupRepository::create(&db, "Kids", None)
            .await
            .unwrap();
        GenreRepository::create(&db, "Adult").await.unwrap();
        AccessGroupRulesRepository::replace(
            &db,
            group.id,
            &AccessGroupRules {
                genres: vec![NameRule::new("Adult", AccessMode::Deny)],
                ..Default::default()
            },
        )
        .await
        .unwrap();

        GenreRepository::delete_unused(&db).await.unwrap();

        let stored = AccessGroupRulesRepository::get(&db, group.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            stored.genres,
            vec![NameRule::new("Adult", AccessMode::Deny)]
        );
    }

    #[tokio::test]
    async fn deleting_a_library_removes_its_rules() {
        let db = setup_test_db().await;
        let group = AccessGroupRepository::create(&db, "Kids", None)
            .await
            .unwrap();
        let library =
            LibraryRepository::create(&db, "Adult", "/tmp/adult", ScanningStrategy::Default)
                .await
                .unwrap();
        AccessGroupRulesRepository::replace(
            &db,
            group.id,
            &AccessGroupRules {
                libraries: vec![LibraryRule {
                    library_id: library.id,
                    access_mode: AccessMode::Deny,
                }],
                ..Default::default()
            },
        )
        .await
        .unwrap();

        LibraryRepository::delete(&db, library.id).await.unwrap();

        let stored = AccessGroupRulesRepository::get(&db, group.id)
            .await
            .unwrap()
            .unwrap();
        assert!(stored.libraries.is_empty());
    }
}
