use anyhow::{Context, Result};
use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
};
use uuid::Uuid;

use crate::db::entities::{libraries, prelude::*};
use crate::db::ScanningStrategy;

/// Repository for Library operations
pub struct LibraryRepository;

impl LibraryRepository {
    /// Create a new library
    pub async fn create(
        db: &DatabaseConnection,
        name: &str,
        path: &str,
        strategy: ScanningStrategy,
    ) -> Result<libraries::Model> {
        let now = Utc::now();

        let library = libraries::ActiveModel {
            id: Set(Uuid::new_v4()),
            name: Set(name.to_string()),
            path: Set(path.to_string()),
            scanning_strategy: Set(strategy.as_str().to_string()),
            scanning_config: Set(None),
            created_at: Set(now),
            updated_at: Set(now),
            last_scanned_at: Set(None),
        };

        library.insert(db).await.context("Failed to create library")
    }

    /// Get a library by ID
    pub async fn get_by_id(db: &DatabaseConnection, id: Uuid) -> Result<Option<libraries::Model>> {
        Libraries::find_by_id(id)
            .one(db)
            .await
            .context("Failed to get library by ID")
    }

    /// Get all libraries
    pub async fn list_all(db: &DatabaseConnection) -> Result<Vec<libraries::Model>> {
        Libraries::find()
            .order_by_asc(libraries::Column::Name)
            .all(db)
            .await
            .context("Failed to list libraries")
    }

    /// Get library by path
    pub async fn get_by_path(
        db: &DatabaseConnection,
        path: &str,
    ) -> Result<Option<libraries::Model>> {
        Libraries::find()
            .filter(libraries::Column::Path.eq(path))
            .one(db)
            .await
            .context("Failed to get library by path")
    }

    /// Update library
    pub async fn update(db: &DatabaseConnection, library: &libraries::Model) -> Result<()> {
        let active = libraries::ActiveModel {
            id: Set(library.id),
            name: Set(library.name.clone()),
            path: Set(library.path.clone()),
            scanning_strategy: Set(library.scanning_strategy.clone()),
            scanning_config: Set(library.scanning_config.clone()),
            created_at: Set(library.created_at),
            updated_at: Set(Utc::now()),
            last_scanned_at: Set(library.last_scanned_at),
        };

        active
            .update(db)
            .await
            .context("Failed to update library")?;

        Ok(())
    }

    /// Update last scanned timestamp
    pub async fn update_last_scanned(db: &DatabaseConnection, id: Uuid) -> Result<()> {
        let library = Libraries::find_by_id(id)
            .one(db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Library not found"))?;

        let mut active: libraries::ActiveModel = library.into();
        active.last_scanned_at = Set(Some(Utc::now()));
        active.updated_at = Set(Utc::now());

        active
            .update(db)
            .await
            .context("Failed to update last scanned timestamp")?;

        Ok(())
    }

    /// Delete a library
    pub async fn delete(db: &DatabaseConnection, id: Uuid) -> Result<()> {
        Libraries::delete_by_id(id)
            .exec(db)
            .await
            .context("Failed to delete library")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_helpers::create_test_db;
    use crate::db::ScanningStrategy;

    #[tokio::test]
    async fn test_create_library() {
        let (db, _temp_dir) = create_test_db().await;

        let library = LibraryRepository::create(
            db.sea_orm_connection(),
            "Test Library",
            "/test/path",
            ScanningStrategy::Default,
        )
        .await
        .unwrap();

        assert_eq!(library.name, "Test Library");
        assert_eq!(library.path, "/test/path");
        assert_eq!(library.scanning_strategy, "default");
    }

    #[tokio::test]
    async fn test_get_library_by_id() {
        let (db, _temp_dir) = create_test_db().await;

        let created = LibraryRepository::create(
            db.sea_orm_connection(),
            "Test Library",
            "/test/path",
            ScanningStrategy::Default,
        )
        .await
        .unwrap();

        let retrieved = LibraryRepository::get_by_id(db.sea_orm_connection(), created.id)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(retrieved.id, created.id);
        assert_eq!(retrieved.name, "Test Library");
    }

    #[tokio::test]
    async fn test_get_library_by_id_not_found() {
        let (db, _temp_dir) = create_test_db().await;

        let result = LibraryRepository::get_by_id(db.sea_orm_connection(), Uuid::new_v4())
            .await
            .unwrap();

        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_list_all_libraries() {
        let (db, _temp_dir) = create_test_db().await;

        LibraryRepository::create(
            db.sea_orm_connection(),
            "Library 1",
            "/path1",
            ScanningStrategy::Default,
        )
        .await
        .unwrap();

        LibraryRepository::create(
            db.sea_orm_connection(),
            "Library 2",
            "/path2",
            ScanningStrategy::Default,
        )
        .await
        .unwrap();

        let libraries = LibraryRepository::list_all(db.sea_orm_connection())
            .await
            .unwrap();

        assert_eq!(libraries.len(), 2);
        assert_eq!(libraries[0].name, "Library 1");
        assert_eq!(libraries[1].name, "Library 2");
    }

    #[tokio::test]
    async fn test_get_library_by_path() {
        let (db, _temp_dir) = create_test_db().await;

        let created = LibraryRepository::create(
            db.sea_orm_connection(),
            "Test Library",
            "/test/path",
            ScanningStrategy::Default,
        )
        .await
        .unwrap();

        let retrieved = LibraryRepository::get_by_path(db.sea_orm_connection(), "/test/path")
            .await
            .unwrap()
            .unwrap();

        assert_eq!(retrieved.id, created.id);
        assert_eq!(retrieved.path, "/test/path");
    }

    #[tokio::test]
    async fn test_update_library() {
        let (db, _temp_dir) = create_test_db().await;

        let mut library = LibraryRepository::create(
            db.sea_orm_connection(),
            "Original Name",
            "/original/path",
            ScanningStrategy::Default,
        )
        .await
        .unwrap();

        library.name = "Updated Name".to_string();
        library.path = "/updated/path".to_string();

        LibraryRepository::update(db.sea_orm_connection(), &library)
            .await
            .unwrap();

        let retrieved = LibraryRepository::get_by_id(db.sea_orm_connection(), library.id)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(retrieved.name, "Updated Name");
        assert_eq!(retrieved.path, "/updated/path");
    }

    #[tokio::test]
    async fn test_update_last_scanned() {
        let (db, _temp_dir) = create_test_db().await;

        let library = LibraryRepository::create(
            db.sea_orm_connection(),
            "Test Library",
            "/test/path",
            ScanningStrategy::Default,
        )
        .await
        .unwrap();

        assert!(library.last_scanned_at.is_none());

        LibraryRepository::update_last_scanned(db.sea_orm_connection(), library.id)
            .await
            .unwrap();

        let retrieved = LibraryRepository::get_by_id(db.sea_orm_connection(), library.id)
            .await
            .unwrap()
            .unwrap();

        assert!(retrieved.last_scanned_at.is_some());
    }

    #[tokio::test]
    async fn test_delete_library() {
        let (db, _temp_dir) = create_test_db().await;

        let library = LibraryRepository::create(
            db.sea_orm_connection(),
            "To Delete",
            "/delete/path",
            ScanningStrategy::Default,
        )
        .await
        .unwrap();

        LibraryRepository::delete(db.sea_orm_connection(), library.id)
            .await
            .unwrap();

        let result = LibraryRepository::get_by_id(db.sea_orm_connection(), library.id)
            .await
            .unwrap();

        assert!(result.is_none());
    }
}
