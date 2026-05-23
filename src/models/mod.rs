//! Cross-layer data models.
//!
//! Types in this module are shared between the api, db, services, tasks, and
//! utils layers without anyone needing to import "up the stack". Anything that
//! both a repository and an API DTO need to reference belongs here so the
//! direction of the dependency stays one-way (consumers depend on `models`,
//! `models` depends on nothing else inside the crate beyond `utils`).

pub mod filter;
pub mod permissions;
pub mod plugin;
pub mod preprocessing;
pub mod release;
pub mod sort;
pub mod strategies;
pub mod task;

pub use strategies::*;
