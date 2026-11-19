//! Vault lifecycle: initialise, unlock, lock, change password (WP-08).
//!
//! This is the platform-facing API over the migrated crypto/vault primitives.
//! It keeps the KDF salt, verifier and wrapped DEK together so callers never
//! assemble security_config by hand.

use zeroize::Zeroizing;

use crate::crypto;
use crate::recovery::{create_recovery_seal, unseal_with_recovery};
use crate::{vault, VaultState};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VaultConfig {
    pub kdf_salt: String,
    pub verifier: String,
    pub wrapped_dek: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewVault {
    pub config: VaultConfig,
    pub recovery_code: String,
    pub recovery_seal: String,
}

/// Create a fresh DEK + password-derived KEK. The vault stays locked.
pub fn create_vault(password: &str) -> Result<NewVault, String> {
    if password.chars().count() < 8 {
        return Err("主密码至少需要 8 个字符".to_string());
    }
    let dek = vault::generate_data_key();
    let kdf_salt = crypto::generate_salt_v3();
    let kek = crypto::derive_from_salt(password, &kdf_salt)?;
    let wrapped_dek = vault::wrap_data_key(&dek, &kek)?;
    let verifier = vault::make_verifier(&dek)?;
    let (recovery_code, recovery_seal) = create_recovery_seal(&dek)?;
    Ok(NewVault {
        config: VaultConfig {
            kdf_salt,
            verifier,
            wrapped_dek,
        },
        recovery_code,
        recovery_seal,
    })
}

/// Unlock with the password: verify, unwrap DEK, install session key.
pub fn unlock(password: &str, config: &VaultConfig) -> Result<(), String> {
    let kek = crypto::derive_from_salt(password, &config.kdf_salt)?;
    let dek = if config.wrapped_dek.is_empty() {
        Zeroizing::new(*kek)
    } else {
        vault::unwrap_data_key(&config.wrapped_dek, &kek)?
    };
    if !vault::verify_key(&config.verifier, &dek) {
        return Err("主密码不正确".to_string());
    }
    vault::set_key(dek);
    Ok(())
}

/// Unlock with a recovery code; returns the recovered DEK to the session.
pub fn unlock_with_recovery(code: &str, seal: &str, config: &VaultConfig) -> Result<(), String> {
    let dek = unseal_with_recovery(code, seal)?;
    if !vault::verify_key(&config.verifier, &dek) {
        return Err("恢复码与当前保险库不匹配".to_string());
    }
    vault::set_key(dek);
    Ok(())
}

pub fn lock() {
    vault::clear_key();
}

pub fn state() -> VaultState {
    if vault::is_unlocked() {
        VaultState::Unlocked
    } else {
        VaultState::Locked
    }
}

/// Re-wrap the same DEK with a new password. Fields do not need re-encryption.
pub fn change_password(
    old_password: &str,
    new_password: &str,
    config: &VaultConfig,
) -> Result<VaultConfig, String> {
    if new_password.chars().count() < 8 {
        return Err("新主密码至少需要 8 个字符".to_string());
    }
    unlock(old_password, config)?;
    let dek = vault::get_key().ok_or("保险库未解锁")?;
    let new_salt = crypto::generate_salt_v3();
    let new_kek = crypto::derive_from_salt(new_password, &new_salt)?;
    let new_wrapped = vault::wrap_data_key(&dek, &new_kek)?;
    let new_verifier = vault::make_verifier(&dek)?;
    Ok(VaultConfig {
        kdf_salt: new_salt,
        verifier: new_verifier,
        wrapped_dek: new_wrapped,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vault::test_lock;

    #[test]
    fn create_unlock_lock_roundtrip() {
        let _g = test_lock();
        lock();
        let vault = create_vault("correct horse battery").unwrap();
        assert!(!vault.recovery_code.is_empty());
        assert!(!vault.recovery_seal.is_empty());
        assert_eq!(state(), VaultState::Locked);
        unlock("wrong password", &vault.config).unwrap_err();
        unlock("correct horse battery", &vault.config).unwrap();
        assert_eq!(state(), VaultState::Unlocked);
        lock();
        assert_eq!(state(), VaultState::Locked);
    }

    #[test]
    fn recovery_code_unlocks() {
        let _g = test_lock();
        lock();
        let v = create_vault("correct horse battery").unwrap();
        unlock_with_recovery(&v.recovery_code, &v.recovery_seal, &v.config).unwrap();
        assert_eq!(state(), VaultState::Unlocked);
        lock();
    }

    #[test]
    fn change_password_keeps_dek_and_fields_readable() {
        let _g = test_lock();
        lock();
        let v = create_vault("old-password").unwrap();
        unlock("old-password", &v.config).unwrap();
        let enc = vault::encrypt_if_needed("secret").unwrap();
        let new_config = change_password("old-password", "new-password", &v.config).unwrap();
        lock();
        assert!(unlock("old-password", &new_config).is_err());
        unlock("new-password", &new_config).unwrap();
        assert_eq!(vault::decrypt_if_needed(&enc).unwrap(), "secret");
        lock();
    }

    #[test]
    fn password_minimum_is_enforced() {
        assert!(create_vault("short").is_err());
    }
}
