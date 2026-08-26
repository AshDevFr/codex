//! Normalize `libraries.default_reading_direction` to the lowercase vocabulary.
//!
//! Two vocabularies were live in this column at once. `LibraryRepository::create`
//! defaulted it to the Komga-style `LEFT_TO_RIGHT`, while the web library form
//! wrote `ltr`. `series_metadata.reading_direction`, the reader store, and the
//! Komga codec all speak the lowercase form, so a library holding the Komga-style
//! value could not be parsed by the reader: the direction silently did not apply,
//! and the library edit form rendered an empty select because no option matched.
//!
//! `WEBTOON` has no Komga-vocabulary origin in this column but is mapped anyway so
//! a hand-edited row converges too. Anything unrecognized is left untouched rather
//! than guessed at; resolution treats an unparseable value as absent and falls
//! through to the next layer.

use sea_orm::Statement;
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

/// Komga-style value to the lowercase form Codex stores.
const FORWARD: &[(&str, &str)] = &[
    ("LEFT_TO_RIGHT", "ltr"),
    ("RIGHT_TO_LEFT", "rtl"),
    ("VERTICAL", "ttb"),
    ("TOP_TO_BOTTOM", "ttb"),
    ("WEBTOON", "webtoon"),
];

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        let backend = manager.get_database_backend();

        for (komga, codex) in FORWARD {
            db.execute(Statement::from_sql_and_values(
                backend,
                "UPDATE libraries SET default_reading_direction = $1 \
                 WHERE default_reading_direction = $2",
                [(*codex).into(), (*komga).into()],
            ))
            .await?;
        }

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        let backend = manager.get_database_backend();

        // `ttb` had two possible origins; it reverts to the one the repository
        // default and the Komga codec agree on.
        let reverse: &[(&str, &str)] = &[
            ("ltr", "LEFT_TO_RIGHT"),
            ("rtl", "RIGHT_TO_LEFT"),
            ("ttb", "VERTICAL"),
            ("webtoon", "WEBTOON"),
        ];

        for (codex, komga) in reverse {
            db.execute(Statement::from_sql_and_values(
                backend,
                "UPDATE libraries SET default_reading_direction = $1 \
                 WHERE default_reading_direction = $2",
                [(*komga).into(), (*codex).into()],
            ))
            .await?;
        }

        Ok(())
    }
}
