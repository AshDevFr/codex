//! Repository for user_series_reader_settings table operations
//!
//! Storage only. The three-state merge that turns a PATCH payload into a new
//! sparse record happens at the API boundary, where `PatchValue` already
//! distinguishes absent from null, the same way the series metadata handlers
//! do it.

use anyhow::{Context, Result};
use chrono::Utc;
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};
use std::collections::HashMap;
use uuid::Uuid;

use codex_models::reader_settings::SeriesReaderSettings;

use crate::entities::{
    user_series_reader_settings, user_series_reader_settings::Entity as UserSeriesReaderSettings,
};

/// Repository for per-user, per-series reader settings.
pub struct UserSeriesReaderSettingsRepository;

impl UserSeriesReaderSettingsRepository {
    /// One user's overrides for one series.
    ///
    /// Returns `None` when the user has no overrides. A row whose stored JSON
    /// cannot be parsed is also reported as `None`: the settings are a
    /// convenience layer, and failing a book listing over one unreadable
    /// preference row would be worse than ignoring it.
    pub async fn get_for_user_series(
        db: &DatabaseConnection,
        user_id: Uuid,
        series_id: Uuid,
    ) -> Result<Option<SeriesReaderSettings>> {
        let row = UserSeriesReaderSettings::find()
            .filter(user_series_reader_settings::Column::UserId.eq(user_id))
            .filter(user_series_reader_settings::Column::SeriesId.eq(series_id))
            .one(db)
            .await?;

        Ok(row.and_then(|row| parse_settings(&row)))
    }

    /// One user's overrides across many series, keyed by series id.
    ///
    /// The batch form exists for book listings, which resolve reading
    /// direction for every series on the page and must not issue a query per
    /// row. Series with no overrides are simply absent from the map.
    pub async fn get_for_user_series_batch(
        db: &DatabaseConnection,
        user_id: Uuid,
        series_ids: &[Uuid],
    ) -> Result<HashMap<Uuid, SeriesReaderSettings>> {
        if series_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let rows = UserSeriesReaderSettings::find()
            .filter(user_series_reader_settings::Column::UserId.eq(user_id))
            .filter(user_series_reader_settings::Column::SeriesId.is_in(series_ids.to_vec()))
            .all(db)
            .await?;

        Ok(rows
            .into_iter()
            .filter_map(|row| {
                let series_id = row.series_id;
                parse_settings(&row).map(|settings| (series_id, settings))
            })
            .collect())
    }

    /// Write a user's overrides for one series, replacing whatever was there.
    ///
    /// An empty record is stored as no record at all: the row is deleted
    /// instead of holding `{}`. A present row means "this user has overrides
    /// here", and an empty one would lie to every later query.
    pub async fn upsert(
        db: &DatabaseConnection,
        user_id: Uuid,
        series_id: Uuid,
        settings: SeriesReaderSettings,
    ) -> Result<Option<user_series_reader_settings::Model>> {
        if settings.is_empty() {
            Self::delete(db, user_id, series_id).await?;
            return Ok(None);
        }

        let encoded =
            serde_json::to_value(settings).context("Failed to serialize series reader settings")?;
        let now = Utc::now();

        let existing = UserSeriesReaderSettings::find()
            .filter(user_series_reader_settings::Column::UserId.eq(user_id))
            .filter(user_series_reader_settings::Column::SeriesId.eq(series_id))
            .one(db)
            .await?;

        let saved = match existing {
            Some(row) => {
                let mut active: user_series_reader_settings::ActiveModel = row.into();
                active.settings = Set(encoded);
                active.updated_at = Set(now);
                active.update(db).await?
            }
            None => {
                user_series_reader_settings::ActiveModel {
                    id: Set(Uuid::new_v4()),
                    user_id: Set(user_id),
                    series_id: Set(series_id),
                    settings: Set(encoded),
                    created_at: Set(now),
                    updated_at: Set(now),
                }
                .insert(db)
                .await?
            }
        };

        Ok(Some(saved))
    }

    /// Drop a user's overrides for one series, restoring full inheritance.
    ///
    /// Returns whether a row was removed.
    pub async fn delete(db: &DatabaseConnection, user_id: Uuid, series_id: Uuid) -> Result<bool> {
        let result = UserSeriesReaderSettings::delete_many()
            .filter(user_series_reader_settings::Column::UserId.eq(user_id))
            .filter(user_series_reader_settings::Column::SeriesId.eq(series_id))
            .exec(db)
            .await?;

        Ok(result.rows_affected > 0)
    }
}

/// Decode a stored row, treating unreadable JSON as no overrides.
fn parse_settings(row: &user_series_reader_settings::Model) -> Option<SeriesReaderSettings> {
    match serde_json::from_value::<SeriesReaderSettings>(row.settings.clone()) {
        Ok(settings) => Some(settings),
        Err(error) => {
            tracing::warn!(
                user_id = %row.user_id,
                series_id = %row.series_id,
                %error,
                "Ignoring unreadable series reader settings"
            );
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ScanningStrategy;
    use crate::entities::{series, users};
    use crate::repositories::{LibraryRepository, SeriesRepository, UserRepository};
    use crate::test_helpers::create_test_db;
    use codex_models::reader_settings::{BackgroundColor, FitMode, PageLayout};
    use codex_models::reading_direction::ReadingDirection;

    async fn create_user(db: &DatabaseConnection, username: &str) -> users::Model {
        let model = users::Model {
            id: Uuid::new_v4(),
            username: username.to_string(),
            email: format!("{}@example.com", username),
            password_hash: "hashedpassword".to_string(),
            role: "reader".to_string(),
            is_active: true,
            email_verified: true,
            permissions: serde_json::json!([]),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_login_at: None,
        };
        UserRepository::create(db, &model).await.unwrap()
    }

    async fn create_series(db: &DatabaseConnection, name: &str, path: &str) -> series::Model {
        let library = LibraryRepository::create(db, name, path, ScanningStrategy::Default)
            .await
            .unwrap();
        SeriesRepository::create(db, library.id, name, None)
            .await
            .unwrap()
    }

    fn rtl() -> SeriesReaderSettings {
        SeriesReaderSettings {
            reading_direction: Some(ReadingDirection::Rtl),
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn upsert_then_get_round_trips_a_sparse_record() {
        let (db, _temp) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let user = create_user(conn, "reader").await;
        let series = create_series(conn, "Berserk", "/berserk").await;

        UserSeriesReaderSettingsRepository::upsert(conn, user.id, series.id, rtl())
            .await
            .unwrap()
            .expect("a row for a non-empty record");

        let stored =
            UserSeriesReaderSettingsRepository::get_for_user_series(conn, user.id, series.id)
                .await
                .unwrap()
                .expect("the settings just written");

        assert_eq!(stored.reading_direction, Some(ReadingDirection::Rtl));
        // The six untouched settings keep inheriting.
        assert_eq!(stored.fit_mode, None);
        assert_eq!(stored.background_color, None);
    }

    #[tokio::test]
    async fn get_returns_none_when_the_user_has_no_overrides() {
        let (db, _temp) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let user = create_user(conn, "reader").await;
        let series = create_series(conn, "Berserk", "/berserk").await;

        assert!(
            UserSeriesReaderSettingsRepository::get_for_user_series(conn, user.id, series.id)
                .await
                .unwrap()
                .is_none()
        );
    }

    #[tokio::test]
    async fn upsert_replaces_the_stored_record() {
        let (db, _temp) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let user = create_user(conn, "reader").await;
        let series = create_series(conn, "Berserk", "/berserk").await;

        UserSeriesReaderSettingsRepository::upsert(conn, user.id, series.id, rtl())
            .await
            .unwrap();

        // The repository stores what it is given. Merging a PATCH onto the
        // existing record is the handler's job, so a second write replaces.
        let replacement = SeriesReaderSettings {
            fit_mode: Some(FitMode::WidthShrink),
            ..Default::default()
        };
        UserSeriesReaderSettingsRepository::upsert(conn, user.id, series.id, replacement)
            .await
            .unwrap();

        let stored =
            UserSeriesReaderSettingsRepository::get_for_user_series(conn, user.id, series.id)
                .await
                .unwrap()
                .unwrap();

        assert_eq!(stored.fit_mode, Some(FitMode::WidthShrink));
        assert_eq!(stored.reading_direction, None);
    }

    #[tokio::test]
    async fn upsert_of_an_empty_record_deletes_the_row() {
        let (db, _temp) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let user = create_user(conn, "reader").await;
        let series = create_series(conn, "Berserk", "/berserk").await;

        UserSeriesReaderSettingsRepository::upsert(conn, user.id, series.id, rtl())
            .await
            .unwrap();

        // Clearing the last override restores full inheritance. Keeping an
        // empty row would report "this user has overrides here" forever.
        let saved = UserSeriesReaderSettingsRepository::upsert(
            conn,
            user.id,
            series.id,
            SeriesReaderSettings::default(),
        )
        .await
        .unwrap();

        assert!(saved.is_none());
        assert!(
            UserSeriesReaderSettingsRepository::get_for_user_series(conn, user.id, series.id)
                .await
                .unwrap()
                .is_none()
        );
    }

    #[tokio::test]
    async fn delete_reports_whether_a_row_was_removed() {
        let (db, _temp) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let user = create_user(conn, "reader").await;
        let series = create_series(conn, "Berserk", "/berserk").await;

        assert!(
            !UserSeriesReaderSettingsRepository::delete(conn, user.id, series.id)
                .await
                .unwrap()
        );

        UserSeriesReaderSettingsRepository::upsert(conn, user.id, series.id, rtl())
            .await
            .unwrap();

        assert!(
            UserSeriesReaderSettingsRepository::delete(conn, user.id, series.id)
                .await
                .unwrap()
        );
    }

    #[tokio::test]
    async fn two_users_hold_different_settings_for_one_series() {
        let (db, _temp) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let reader = create_user(conn, "reader").await;
        let admin = create_user(conn, "admin").await;
        let series = create_series(conn, "Berserk", "/berserk").await;

        UserSeriesReaderSettingsRepository::upsert(conn, reader.id, series.id, rtl())
            .await
            .unwrap();
        UserSeriesReaderSettingsRepository::upsert(
            conn,
            admin.id,
            series.id,
            SeriesReaderSettings {
                reading_direction: Some(ReadingDirection::Ltr),
                ..Default::default()
            },
        )
        .await
        .unwrap();

        let for_reader =
            UserSeriesReaderSettingsRepository::get_for_user_series(conn, reader.id, series.id)
                .await
                .unwrap()
                .unwrap();
        let for_admin =
            UserSeriesReaderSettingsRepository::get_for_user_series(conn, admin.id, series.id)
                .await
                .unwrap()
                .unwrap();

        assert_eq!(for_reader.reading_direction, Some(ReadingDirection::Rtl));
        assert_eq!(for_admin.reading_direction, Some(ReadingDirection::Ltr));
    }

    #[tokio::test]
    async fn batch_returns_only_series_with_overrides_for_that_user() {
        let (db, _temp) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let user = create_user(conn, "reader").await;
        let other = create_user(conn, "other").await;
        let with_override = create_series(conn, "Berserk", "/berserk").await;
        let without = create_series(conn, "Bone", "/bone").await;
        let other_users = create_series(conn, "Akira", "/akira").await;

        UserSeriesReaderSettingsRepository::upsert(conn, user.id, with_override.id, rtl())
            .await
            .unwrap();
        UserSeriesReaderSettingsRepository::upsert(conn, other.id, other_users.id, rtl())
            .await
            .unwrap();

        let map = UserSeriesReaderSettingsRepository::get_for_user_series_batch(
            conn,
            user.id,
            &[with_override.id, without.id, other_users.id],
        )
        .await
        .unwrap();

        assert_eq!(map.len(), 1);
        assert_eq!(
            map.get(&with_override.id).unwrap().reading_direction,
            Some(ReadingDirection::Rtl)
        );
        // Another user's row must not leak into this user's resolution.
        assert!(!map.contains_key(&other_users.id));
    }

    #[tokio::test]
    async fn batch_with_no_series_makes_no_query() {
        let (db, _temp) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let user = create_user(conn, "reader").await;

        let map = UserSeriesReaderSettingsRepository::get_for_user_series_batch(conn, user.id, &[])
            .await
            .unwrap();

        assert!(map.is_empty());
    }

    #[tokio::test]
    async fn deleting_the_series_removes_the_settings() {
        let (db, _temp) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let user = create_user(conn, "reader").await;
        let series = create_series(conn, "Berserk", "/berserk").await;

        UserSeriesReaderSettingsRepository::upsert(conn, user.id, series.id, rtl())
            .await
            .unwrap();

        SeriesRepository::delete(conn, series.id).await.unwrap();

        let remaining = UserSeriesReaderSettings::find().all(conn).await.unwrap();
        assert!(
            remaining.is_empty(),
            "cascade should clear orphaned settings"
        );
    }

    #[tokio::test]
    async fn deleting_the_user_removes_the_settings() {
        let (db, _temp) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let user = create_user(conn, "reader").await;
        let series = create_series(conn, "Berserk", "/berserk").await;

        UserSeriesReaderSettingsRepository::upsert(conn, user.id, series.id, rtl())
            .await
            .unwrap();

        UserRepository::delete(conn, user.id).await.unwrap();

        let remaining = UserSeriesReaderSettings::find().all(conn).await.unwrap();
        assert!(
            remaining.is_empty(),
            "cascade should clear orphaned settings"
        );
    }

    #[tokio::test]
    async fn an_unreadable_row_reads_as_no_overrides() {
        let (db, _temp) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let user = create_user(conn, "reader").await;
        let series = create_series(conn, "Berserk", "/berserk").await;

        // Written outside the API boundary, as a hand-edited row or an older
        // version might have been.
        let now = Utc::now();
        user_series_reader_settings::ActiveModel {
            id: Set(Uuid::new_v4()),
            user_id: Set(user.id),
            series_id: Set(series.id),
            settings: Set(serde_json::json!({ "readingDirection": "sideways" })),
            created_at: Set(now),
            updated_at: Set(now),
        }
        .insert(conn)
        .await
        .unwrap();

        // Junk must degrade to inheritance, not fail the read that a whole
        // book listing depends on.
        assert!(
            UserSeriesReaderSettingsRepository::get_for_user_series(conn, user.id, series.id)
                .await
                .unwrap()
                .is_none()
        );

        let map = UserSeriesReaderSettingsRepository::get_for_user_series_batch(
            conn,
            user.id,
            &[series.id],
        )
        .await
        .unwrap();
        assert!(map.is_empty());
    }

    #[tokio::test]
    async fn every_setting_survives_a_write_and_read() {
        let (db, _temp) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let user = create_user(conn, "reader").await;
        let series = create_series(conn, "Berserk", "/berserk").await;

        let all = SeriesReaderSettings {
            fit_mode: Some(FitMode::Height),
            webtoon_fit_mode: Some(codex_models::reader_settings::WebtoonFitMode::Original),
            page_layout: Some(PageLayout::Double),
            reading_direction: Some(ReadingDirection::Ttb),
            background_color: Some(BackgroundColor::White),
            double_page_show_wide_alone: Some(true),
            double_page_start_on_odd: Some(false),
        };

        UserSeriesReaderSettingsRepository::upsert(conn, user.id, series.id, all)
            .await
            .unwrap();

        let stored =
            UserSeriesReaderSettingsRepository::get_for_user_series(conn, user.id, series.id)
                .await
                .unwrap()
                .unwrap();

        assert_eq!(stored, all);
    }
}
