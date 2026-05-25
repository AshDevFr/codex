//! Series-level visibility filter for use with book and series queries.
//!
//! Sharing-tag access control is resolved to a flat set of "denied" and
//! optionally "allowed" series IDs (see `ContentFilter` in `codex-services`).
//! This module exposes a small data type plus SeaORM helpers so repositories
//! can apply the filter directly at the SQL layer instead of forcing every
//! caller to fetch-then-filter in memory.
//!
//! Semantics (mirroring `ContentFilter`):
//! - `excluded_series_ids` are always hidden (deny wins).
//! - `allowed_series_ids = Some(_)` means whitelist mode is active:
//!   only series in that list are visible. An empty allowed list means
//!   "nothing is visible" (whitelist with zero matches).
//! - `allowed_series_ids = None` means no whitelist constraint.

use crate::entities::{books, series};
use sea_orm::{ColumnTrait, QueryFilter, Select, sea_query::SimpleExpr};
use uuid::Uuid;

/// Visibility constraints to apply at the SQL layer.
#[derive(Debug, Clone, Default)]
pub struct SeriesVisibility {
    /// Series IDs the user is denied access to.
    pub excluded_series_ids: Vec<Uuid>,
    /// When `Some`, only series in this list are visible (whitelist mode).
    /// When `None`, no whitelist constraint is applied.
    pub allowed_series_ids: Option<Vec<Uuid>>,
}

impl SeriesVisibility {
    /// Quick check whether this filter would constrain results at all.
    pub fn is_unrestricted(&self) -> bool {
        self.excluded_series_ids.is_empty() && self.allowed_series_ids.is_none()
    }

    /// Whitelist mode with zero allowed series: result is guaranteed empty.
    pub fn is_empty_whitelist(&self) -> bool {
        matches!(&self.allowed_series_ids, Some(ids) if ids.is_empty())
    }
}

/// Build a SeaORM `SimpleExpr` enforcing the visibility filter against a
/// column that holds a series UUID (the series PK on `series`, or the
/// `series_id` FK on `books`).
///
/// Returns `None` when the filter is unrestricted, letting callers avoid
/// adding a no-op `WHERE` clause.
pub fn visibility_predicate<C>(column: C, vis: &SeriesVisibility) -> Option<SimpleExpr>
where
    C: ColumnTrait,
{
    if vis.is_unrestricted() {
        return None;
    }

    let mut expr: Option<SimpleExpr> = None;

    if !vis.excluded_series_ids.is_empty() {
        let denied = column.is_not_in(vis.excluded_series_ids.clone());
        expr = Some(match expr {
            Some(prev) => prev.and(denied),
            None => denied,
        });
    }

    if let Some(allowed) = &vis.allowed_series_ids {
        let whitelist = column.is_in(allowed.clone());
        expr = Some(match expr {
            Some(prev) => prev.and(whitelist),
            None => whitelist,
        });
    }

    expr
}

/// Apply the visibility filter to a books query (filters by `books.series_id`).
///
/// When `vis` is `None` or unrestricted, the query is returned unchanged.
pub fn apply_book_visibility(
    query: Select<books::Entity>,
    vis: Option<&SeriesVisibility>,
) -> Select<books::Entity> {
    let Some(vis) = vis else {
        return query;
    };
    match visibility_predicate(books::Column::SeriesId, vis) {
        Some(expr) => query.filter(expr),
        None => query,
    }
}

/// Apply the visibility filter to a series query (filters by `series.id`).
///
/// When `vis` is `None` or unrestricted, the query is returned unchanged.
pub fn apply_series_visibility(
    query: Select<series::Entity>,
    vis: Option<&SeriesVisibility>,
) -> Select<series::Entity> {
    let Some(vis) = vis else {
        return query;
    };
    match visibility_predicate(series::Column::Id, vis) {
        Some(expr) => query.filter(expr),
        None => query,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unrestricted_returns_no_predicate() {
        let vis = SeriesVisibility::default();
        assert!(vis.is_unrestricted());
        assert!(visibility_predicate(books::Column::SeriesId, &vis).is_none());
    }

    #[test]
    fn empty_whitelist_is_detected() {
        let vis = SeriesVisibility {
            excluded_series_ids: vec![],
            allowed_series_ids: Some(vec![]),
        };
        assert!(!vis.is_unrestricted());
        assert!(vis.is_empty_whitelist());
        // Still produces a predicate so callers can short-circuit to "no rows"
        assert!(visibility_predicate(series::Column::Id, &vis).is_some());
    }

    #[test]
    fn deny_only_produces_predicate() {
        let vis = SeriesVisibility {
            excluded_series_ids: vec![Uuid::new_v4()],
            allowed_series_ids: None,
        };
        assert!(visibility_predicate(books::Column::SeriesId, &vis).is_some());
    }

    #[test]
    fn allow_only_produces_predicate() {
        let vis = SeriesVisibility {
            excluded_series_ids: vec![],
            allowed_series_ids: Some(vec![Uuid::new_v4()]),
        };
        assert!(visibility_predicate(series::Column::Id, &vis).is_some());
    }

    #[test]
    fn allow_and_deny_combine() {
        let vis = SeriesVisibility {
            excluded_series_ids: vec![Uuid::new_v4()],
            allowed_series_ids: Some(vec![Uuid::new_v4(), Uuid::new_v4()]),
        };
        assert!(visibility_predicate(books::Column::SeriesId, &vis).is_some());
    }
}
