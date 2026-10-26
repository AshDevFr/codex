//! A process-wide counter of changes to who can see what.
//!
//! The content filter is cached per user (see `codex-services`). Every
//! repository write that can change a user's visibility (access group rules
//! and memberships, sharing-tag grants, series sharing tags, a user's role,
//! deleting a user) bumps this counter, and a cached filter built under an
//! older value is discarded. Bumping in the repositories rather than in the
//! HTTP handlers means every write path is covered: API calls, the OIDC
//! membership sync on login, joining the default group at account creation.
//!
//! The counter is per process. Another server process sees the change when
//! its cached entry expires.
//!
//! Genre and tag changes on series do not bump it: scans make them
//! constantly, and the cache's short expiry covers them.

use std::sync::atomic::{AtomicU64, Ordering};

static VERSION: AtomicU64 = AtomicU64::new(0);

/// The current version. A cached filter is valid only if it was built under it.
pub fn current() -> u64 {
    VERSION.load(Ordering::Acquire)
}

/// Record that visibility may have changed for some user.
pub fn bump() {
    VERSION.fetch_add(1, Ordering::AcqRel);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bump_moves_the_version_forward() {
        let before = current();
        bump();
        assert!(current() > before);
    }
}
