//! Security core (SEC-01..05).
//!
//! M1 migrates the audited AES-256-GCM / Argon2id / HKDF implementation from
//! the legacy desktop app instead of rewriting cryptography. Kept as modules
//! so every entry point uses the same primitives; no domain may grow its own.

#![forbid(unsafe_code)]

pub mod backup;
pub mod crypto;
pub mod keyring;
pub mod recovery;
pub mod session;
pub mod shares;
pub mod vault;

pub use backup::{create_backup, restore_backup, verify_backup, BackupInfo, BackupManifest};
pub use keyring::FileKeyring;
pub use recovery::{create_recovery_seal, unseal_with_recovery};
pub use session::{
    change_password, create_vault, lock, state, unlock, unlock_with_recovery, NewVault, VaultConfig,
};
pub use shares::{generate_shares, recover_from_shares, KitMeta, ShareKit};
pub use vault::{
    clear_key, decrypt_field_any, decrypt_if_needed, decrypt_or_placeholder, derive_domain_key,
    encrypt_field, encrypt_field_v2, encrypt_if_needed, generate_data_key, get_key, is_encrypted,
    is_placeholder, is_unlocked, make_verifier, resolve_external_url, resolve_tcp_endpoint,
    resolve_tcp_host, set_key, unwrap_data_key, validate_external_url, verify_key, wrap_data_key,
    ResolvedExternalUrl, ENC_PREFIX, ENC_PREFIX_V2, PLACEHOLDER_LOCKED,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VaultState {
    Locked,
    Unlocked,
    Uninitialized,
}

pub const MODULE_FAMILY: &str = "SEC";
pub const STATUS: &str = "m1-security";
