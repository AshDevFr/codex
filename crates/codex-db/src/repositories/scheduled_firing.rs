//! Repository for [`scheduled_firing_claims`]: a distributed claim used to make
//! a cron firing run on exactly one replica.

use anyhow::{Context, Result};
use chrono::{DateTime, Duration, Utc};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};

use crate::entities::scheduled_firing_claims::{self, Entity as ScheduledFiringClaims};

pub struct ScheduledFiringRepository;

impl ScheduledFiringRepository {
    /// How long claim rows are kept before the winner opportunistically prunes
    /// them. Only needs to outlive the longest plausible cron interval so a slow
    /// recurrence never collides with a stale row; two days is comfortably safe.
    const RETENTION_DAYS: i64 = 2;

    /// Try to claim the firing identified by `(job_key, fire_slot)`.
    ///
    /// Returns `true` if this caller won the claim and should do the work, or
    /// `false` if another replica already claimed this exact firing. Concurrency-
    /// safe: the table's composite primary key makes exactly one INSERT succeed;
    /// the losers see a unique/primary-key violation, which is reported as
    /// `Ok(false)` rather than an error. Any other DB error is propagated so the
    /// caller can decide how to proceed (callers fan-out fail-open, since the
    /// per-task dedup still prevents duplicates).
    pub async fn try_claim(
        db: &DatabaseConnection,
        job_key: &str,
        fire_slot: DateTime<Utc>,
    ) -> Result<bool> {
        let claim = scheduled_firing_claims::ActiveModel {
            job_key: Set(job_key.to_string()),
            fire_slot: Set(fire_slot),
            claimed_at: Set(Utc::now()),
        };

        match ScheduledFiringClaims::insert(claim).exec(db).await {
            Ok(_) => {
                // Winner-only cleanup: paid once per firing, not once per replica.
                Self::prune_old(db, job_key, fire_slot).await;
                Ok(true)
            }
            Err(e) => {
                let msg = e.to_string().to_lowercase();
                if msg.contains("unique")
                    || msg.contains("duplicate")
                    || msg.contains("primary key")
                    || msg.contains("constraint")
                {
                    Ok(false)
                } else {
                    Err(e).context("Failed to claim scheduled firing")
                }
            }
        }
    }

    /// Best-effort deletion of this job's claims older than the retention window.
    /// Failures are swallowed: pruning is housekeeping, not correctness.
    async fn prune_old(db: &DatabaseConnection, job_key: &str, now: DateTime<Utc>) {
        let cutoff = now - Duration::days(Self::RETENTION_DAYS);
        let _ = ScheduledFiringClaims::delete_many()
            .filter(scheduled_firing_claims::Column::JobKey.eq(job_key))
            .filter(scheduled_firing_claims::Column::FireSlot.lt(cutoff))
            .exec(db)
            .await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::setup_test_db;

    fn slot(secs: i64) -> DateTime<Utc> {
        DateTime::from_timestamp(secs, 0).unwrap()
    }

    #[tokio::test]
    async fn first_claim_wins_second_loses() {
        let db = setup_test_db().await;
        let fire = slot(1_700_000_000);

        assert!(
            ScheduledFiringRepository::try_claim(&db, "plugin_sync:a", fire)
                .await
                .unwrap(),
            "first claim should win"
        );
        assert!(
            !ScheduledFiringRepository::try_claim(&db, "plugin_sync:a", fire)
                .await
                .unwrap(),
            "second claim for the same firing should lose"
        );
    }

    #[tokio::test]
    async fn distinct_slots_and_jobs_each_win() {
        let db = setup_test_db().await;
        let fire = slot(1_700_000_000);

        // Same job, different firing → independent claim.
        assert!(
            ScheduledFiringRepository::try_claim(&db, "plugin_sync:a", fire)
                .await
                .unwrap()
        );
        assert!(
            ScheduledFiringRepository::try_claim(&db, "plugin_sync:a", slot(1_700_000_060))
                .await
                .unwrap()
        );
        // Different job, same firing → independent claim.
        assert!(
            ScheduledFiringRepository::try_claim(&db, "plugin_sync:b", fire)
                .await
                .unwrap()
        );
    }

    #[tokio::test]
    async fn concurrent_claims_elect_exactly_one_winner() {
        let db = setup_test_db().await;
        let fire = slot(1_700_000_000);

        // Fan out N concurrent claims for the same firing on a shared pool,
        // mimicking N replicas firing at once. Exactly one must win.
        let mut handles = Vec::new();
        for _ in 0..8 {
            let db = db.clone();
            handles.push(tokio::spawn(async move {
                ScheduledFiringRepository::try_claim(&db, "plugin_sync:race", fire)
                    .await
                    .unwrap()
            }));
        }

        let mut wins = 0;
        for h in handles {
            if h.await.unwrap() {
                wins += 1;
            }
        }
        assert_eq!(wins, 1, "exactly one concurrent claim should win");
    }
}
