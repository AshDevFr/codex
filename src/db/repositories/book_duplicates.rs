use anyhow::{Context, Result};
use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseBackend, DatabaseConnection,
    EntityTrait, PaginatorTrait, QueryFilter, Set, Statement,
};
use tracing::{debug, info};
use uuid::Uuid;

use crate::db::entities::{book_duplicates, prelude::*};

/// Repository for BookDuplicates operations
pub struct BookDuplicatesRepository;

impl BookDuplicatesRepository {
    /// Find all duplicate groups
    pub async fn find_all(db: &DatabaseConnection) -> Result<Vec<book_duplicates::Model>> {
        BookDuplicates::find()
            .all(db)
            .await
            .context("Failed to find all duplicates")
    }

    /// Find a duplicate group by file hash
    pub async fn find_by_hash(
        db: &DatabaseConnection,
        hash: &str,
    ) -> Result<Option<book_duplicates::Model>> {
        BookDuplicates::find()
            .filter(book_duplicates::Column::FileHash.eq(hash))
            .one(db)
            .await
            .context("Failed to find duplicate by hash")
    }

    /// Count total number of duplicate groups
    pub async fn count(db: &DatabaseConnection) -> Result<u64> {
        BookDuplicates::find()
            .count(db)
            .await
            .context("Failed to count duplicates")
    }

    /// Delete a specific duplicate group by ID
    pub async fn delete_group(db: &DatabaseConnection, id: Uuid) -> Result<()> {
        BookDuplicates::delete_by_id(id)
            .exec(db)
            .await
            .context("Failed to delete duplicate group")?;

        Ok(())
    }

    /// Rebuild the entire duplicates table from current books
    ///
    /// This completely recreates the duplicate detection data by:
    /// 1. Deleting all existing duplicate records
    /// 2. Querying books for duplicate file_hashes
    /// 3. Inserting new duplicate group records
    ///
    /// Returns the number of duplicate groups found
    pub async fn rebuild_from_books(db: &DatabaseConnection) -> Result<usize> {
        info!("Starting duplicate rebuild from books table");

        // Step 1: Delete all existing duplicates
        debug!("Clearing existing duplicate records");
        let delete_stmt = Statement::from_string(
            db.get_database_backend(),
            "DELETE FROM book_duplicates".to_owned(),
        );
        db.execute(delete_stmt)
            .await
            .context("Failed to clear duplicate records")?;

        // Step 2: Query for duplicate file_hashes
        // This query finds all file_hashes that appear more than once in non-deleted books
        let duplicates_query = match db.get_database_backend() {
            DatabaseBackend::Postgres => {
                r#"
                SELECT
                    file_hash,
                    json_agg(id ORDER BY created_at) as book_ids,
                    COUNT(*) as duplicate_count
                FROM books
                WHERE deleted = false
                GROUP BY file_hash
                HAVING COUNT(*) > 1
                ORDER BY COUNT(*) DESC
                "#
            }
            DatabaseBackend::Sqlite => {
                r#"
                SELECT
                    file_hash,
                    GROUP_CONCAT(LOWER(HEX(id)), ',') as book_ids_str,
                    COUNT(*) as duplicate_count
                FROM books
                WHERE deleted = 0
                GROUP BY file_hash
                HAVING COUNT(*) > 1
                ORDER BY COUNT(*) DESC
                "#
            }
            _ => {
                return Err(anyhow::anyhow!("Unsupported database backend"));
            }
        };

        let duplicates_stmt =
            Statement::from_string(db.get_database_backend(), duplicates_query.to_owned());

        let duplicates_result = db
            .query_all(duplicates_stmt)
            .await
            .context("Failed to query for duplicates")?;

        debug!("Found {} duplicate groups", duplicates_result.len());

        // Step 3: Insert new duplicate records
        let mut count = 0;
        for row in duplicates_result {
            let file_hash: String = row.try_get("", "file_hash")?;
            let duplicate_count: i32 = row.try_get("", "duplicate_count")?;

            // Parse book IDs based on database backend
            let book_ids: Vec<Uuid> = match db.get_database_backend() {
                DatabaseBackend::Postgres => {
                    // PostgreSQL returns JSON array
                    let book_ids_json: String = row.try_get("", "book_ids")?;
                    serde_json::from_str(&book_ids_json).context("Failed to parse book_ids JSON")?
                }
                DatabaseBackend::Sqlite => {
                    // SQLite returns comma-separated hex strings
                    let book_ids_str: String = row.try_get("", "book_ids_str")?;
                    book_ids_str
                        .split(',')
                        .map(|hex_str| {
                            // Parse hex string (without dashes) into UUID
                            Uuid::parse_str(&format!(
                                "{}-{}-{}-{}-{}",
                                &hex_str[0..8],
                                &hex_str[8..12],
                                &hex_str[12..16],
                                &hex_str[16..20],
                                &hex_str[20..32]
                            ))
                        })
                        .collect::<Result<Vec<Uuid>, _>>()
                        .context("Failed to parse UUID from hex string")?
                }
                _ => {
                    return Err(anyhow::anyhow!("Unsupported database backend"));
                }
            };

            if book_ids.len() < 2 {
                // Skip if somehow we got less than 2 books (shouldn't happen with HAVING COUNT(*) > 1)
                continue;
            }

            let duplicate = book_duplicates::ActiveModel {
                id: Set(Uuid::new_v4()),
                file_hash: Set(file_hash.clone()),
                book_ids: Set(serde_json::to_string(&book_ids)?),
                duplicate_count: Set(duplicate_count),
                created_at: Set(Utc::now()),
                updated_at: Set(Utc::now()),
            };

            duplicate
                .insert(db)
                .await
                .context("Failed to insert duplicate record")?;

            count += 1;
            debug!(
                "Added duplicate group for hash {} ({} books)",
                file_hash, duplicate_count
            );
        }

        info!("Duplicate rebuild complete: {} groups found", count);
        Ok(count)
    }

    /// Clean up duplicates after a book is deleted
    ///
    /// This removes the book from any duplicate groups and deletes groups
    /// that no longer have duplicates (only 1 book remaining)
    pub async fn cleanup_for_book(db: &DatabaseConnection, book_id: Uuid) -> Result<()> {
        debug!("Cleaning up duplicates for book {}", book_id);

        // Find all duplicate groups that might contain this book
        // We need to do this differently based on database backend
        let all_duplicates = BookDuplicates::find()
            .all(db)
            .await
            .context("Failed to fetch duplicates for cleanup")?;

        for group in all_duplicates {
            // Parse book_ids from JSON string
            let mut book_ids: Vec<Uuid> =
                serde_json::from_str(&group.book_ids).context("Failed to parse book_ids")?;

            // Check if this group contains the deleted book
            if !book_ids.contains(&book_id) {
                continue; // Skip groups that don't contain this book
            }

            // Remove the book from the list
            book_ids.retain(|id| id != &book_id);

            if book_ids.len() <= 1 {
                // No longer a duplicate, delete the group
                debug!(
                    "Deleting duplicate group {} (only {} books remaining)",
                    group.id,
                    book_ids.len()
                );
                BookDuplicates::delete_by_id(group.id)
                    .exec(db)
                    .await
                    .context("Failed to delete duplicate group")?;
            } else {
                // Update the group with the new book list
                debug!(
                    "Updating duplicate group {} (now {} books)",
                    group.id,
                    book_ids.len()
                );

                let mut active_model: book_duplicates::ActiveModel = group.into();
                active_model.book_ids = Set(serde_json::to_string(&book_ids)?);
                active_model.duplicate_count = Set(book_ids.len() as i32);
                active_model.updated_at = Set(Utc::now());

                active_model
                    .update(db)
                    .await
                    .context("Failed to update duplicate group")?;
            }
        }

        Ok(())
    }

    /// Helper: Check if a book_ids JSON array contains a specific UUID
    /// This is used for database-specific queries
    #[allow(dead_code)]
    fn json_contains_uuid(backend: DatabaseBackend, uuid: Uuid) -> String {
        match backend {
            DatabaseBackend::Postgres => {
                format!("book_ids @> '[\"{}\"]]'::jsonb", uuid)
            }
            DatabaseBackend::Sqlite => {
                // SQLite JSON containment is more complex, we'll filter in application code
                format!("book_ids LIKE '%{}%'", uuid)
            }
            _ => String::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_json_contains_uuid() {
        let uuid = Uuid::new_v4();
        let pg_query =
            BookDuplicatesRepository::json_contains_uuid(DatabaseBackend::Postgres, uuid);
        assert!(pg_query.contains(&uuid.to_string()));

        let sqlite_query =
            BookDuplicatesRepository::json_contains_uuid(DatabaseBackend::Sqlite, uuid);
        assert!(sqlite_query.contains(&uuid.to_string()));
    }
}
