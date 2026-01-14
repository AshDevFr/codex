//! Integration services for external service connections
//!
//! This module provides:
//! - Credential encryption for secure storage
//! - System-level integrations (admin-managed)
//! - User-level integrations (per-user OAuth tokens)
//! - Provider traits and implementations

pub mod encryption;
pub mod providers;

pub use encryption::CredentialEncryption;
