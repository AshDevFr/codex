//! Pagination window for list queries.
//!
//! Lives in `models` so db repositories can take a typed window without
//! depending on the api layer, matching how [`crate::sort`] is shared.

/// A half-open row range: skip `offset` rows, then take at most `limit`.
///
/// Repository methods take this rather than a pair of bare `u64`s because the
/// pair has bitten us. Two conventions coexisted, one where the first argument
/// was a row offset and one where it was a page index the method multiplied by
/// the page size itself. Both were `u64`, so passing the wrong one compiled
/// happily and produced `(page - 1) * page_size²` as the effective offset.
///
/// Six endpoints shipped that way. The failure hides itself: the first page is
/// correct under the bug, because zero times anything is zero, so any test that
/// does not page passes. They were eventually found by a client scrolling a
/// list, not by the suite.
///
/// Constructing this through [`Window::from_page`] or [`Window::from_offset`]
/// makes the unit explicit at the call site, and the fields are private so the
/// two cannot be mixed up afterwards.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Window {
    offset: u64,
    limit: u64,
}

impl Window {
    /// Build from a 1-indexed page number, as the HTTP API expresses it.
    ///
    /// This is the one place the 1-indexed to 0-indexed conversion happens.
    /// Page 0 is treated as page 1 rather than underflowing.
    pub fn from_page(page: u64, page_size: u64) -> Self {
        Self {
            offset: page.saturating_sub(1).saturating_mul(page_size),
            limit: page_size,
        }
    }

    /// Build from an explicit row offset, for callers that genuinely have one
    /// rather than a page number.
    pub fn from_offset(offset: u64, limit: u64) -> Self {
        Self { offset, limit }
    }

    /// Every row, for callers that want no pagination at all.
    ///
    /// The limit is `i64::MAX` rather than `u64::MAX`: SQL `LIMIT` is signed on
    /// Postgres, so a value above `i64::MAX` does not survive the round trip.
    /// This is not a real ceiling either way, and having one lets every query
    /// apply its limit unconditionally instead of branching on a sentinel.
    pub fn unbounded() -> Self {
        Self {
            offset: 0,
            limit: i64::MAX as u64,
        }
    }

    /// Rows to skip.
    pub fn offset(&self) -> u64 {
        self.offset
    }

    /// Maximum rows to return.
    pub fn limit(&self) -> u64 {
        self.limit
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_page_starts_the_first_page_at_zero() {
        let window = Window::from_page(1, 20);
        assert_eq!(window.offset(), 0);
        assert_eq!(window.limit(), 20);
    }

    #[test]
    fn from_page_advances_by_one_page_at_a_time() {
        assert_eq!(Window::from_page(2, 20).offset(), 20);
        assert_eq!(Window::from_page(3, 20).offset(), 40);
        assert_eq!(Window::from_page(6, 5).offset(), 25);
    }

    /// Page 0 is not meaningful in a 1-indexed scheme, and `0 - 1` on a u64
    /// panics in debug and wraps in release. Clamp to the first page instead.
    #[test]
    fn from_page_treats_page_zero_as_the_first_page() {
        let window = Window::from_page(0, 20);
        assert_eq!(window.offset(), 0);
        assert_eq!(window.limit(), 20);
    }

    /// The defect this type exists to prevent: an offset multiplied a second
    /// time. `from_page(2, 20)` must be 20, never 400.
    #[test]
    fn from_page_does_not_multiply_an_offset_twice() {
        assert_eq!(
            Window::from_page(2, 20).offset(),
            20,
            "a page index must be converted once, not squared",
        );
    }

    #[test]
    fn from_offset_passes_the_offset_through_untouched() {
        let window = Window::from_offset(37, 10);
        assert_eq!(window.offset(), 37);
        assert_eq!(window.limit(), 10);
    }

    #[test]
    fn unbounded_starts_at_the_beginning_and_does_not_stop() {
        let window = Window::unbounded();
        assert_eq!(window.offset(), 0);
        assert_eq!(window.limit(), i64::MAX as u64);
    }

    /// SQL `LIMIT` is signed on Postgres, so an unbounded window has to stay
    /// inside `i64` or it cannot be sent at all. `u64::MAX` would not.
    #[test]
    fn unbounded_limit_fits_in_a_signed_sql_limit() {
        assert!(
            i64::try_from(Window::unbounded().limit()).is_ok(),
            "an unbounded limit must survive the trip to a signed SQL LIMIT",
        );
    }

    /// A large page must not wrap into a small offset.
    #[test]
    fn from_page_saturates_rather_than_overflowing() {
        let window = Window::from_page(u64::MAX, u64::MAX);
        assert_eq!(window.offset(), u64::MAX);
    }
}
