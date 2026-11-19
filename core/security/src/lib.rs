//! Security boundary (SEC-01..05).
//!
//! M0 intentionally contains no cryptography. M1 migrates the existing audited
//! AES-GCM/Argon2id/Shamir implementation from `legacy/app/src-tauri`; this
//! crate reserves the boundary so no domain can grow its own crypto.

#![forbid(unsafe_code)]

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VaultState {
    Locked,
    Unlocked,
    Uninitialized,
}

pub const MODULE_FAMILY: &str = "SEC";
pub const STATUS: &str = "planned";
