pub use sea_orm_migration::prelude::*;

mod m20260103_000001_create_libraries;
mod m20260103_000002_create_users;
mod m20260103_000003_create_series;
mod m20260103_000004_create_books;
mod m20260103_000005_create_pages;
mod m20260103_000006_create_book_metadata_records;
mod m20260103_000007_create_metadata_sources;
mod m20260103_000008_create_read_progress;
mod m20260103_000009_create_api_keys;
mod m20260103_000010_add_user_permissions;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20260103_000001_create_libraries::Migration),
            Box::new(m20260103_000002_create_users::Migration),
            Box::new(m20260103_000003_create_series::Migration),
            Box::new(m20260103_000004_create_books::Migration),
            Box::new(m20260103_000005_create_pages::Migration),
            Box::new(m20260103_000006_create_book_metadata_records::Migration),
            Box::new(m20260103_000007_create_metadata_sources::Migration),
            Box::new(m20260103_000008_create_read_progress::Migration),
            Box::new(m20260103_000009_create_api_keys::Migration),
            Box::new(m20260103_000010_add_user_permissions::Migration),
        ]
    }
}
