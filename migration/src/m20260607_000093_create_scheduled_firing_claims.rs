use sea_orm_migration::prelude::*;

/// Distributed claim table so a cron firing runs on exactly one replica.
///
/// Every `serve` replica runs its own scheduler, so each cron fires once per
/// replica. Jobs whose firing does real work claim `(job_key, fire_slot)` here
/// before acting; the composite primary key makes exactly one INSERT win.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(ScheduledFiringClaims::Table)
                    .if_not_exists()
                    // Logical job, e.g. "plugin_sync:<plugin_uuid>".
                    .col(
                        ColumnDef::new(ScheduledFiringClaims::JobKey)
                            .string()
                            .not_null(),
                    )
                    // Firing instant truncated to the cron's granularity, so all
                    // replicas firing for the same occurrence agree on the key.
                    .col(
                        ColumnDef::new(ScheduledFiringClaims::FireSlot)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ScheduledFiringClaims::ClaimedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    // Composite PK is the uniqueness guarantee that elects the
                    // single winner per firing.
                    .primary_key(
                        Index::create()
                            .col(ScheduledFiringClaims::JobKey)
                            .col(ScheduledFiringClaims::FireSlot),
                    )
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ScheduledFiringClaims::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum ScheduledFiringClaims {
    Table,
    JobKey,
    FireSlot,
    ClaimedAt,
}
