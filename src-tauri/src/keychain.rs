//! Thin wrapper around the `keyring` crate for storing the bridge API
//! key. macOS Keychain, Windows Credential Manager, Linux Secret Service.
//!
//! Exposed operations:
//!   - save(key)      — store / overwrite
//!   - load()         — returns Some(key) or None
//!   - clear()        — delete the entry (uninstall path)

use crate::config::{KEYCHAIN_SERVICE, KEYCHAIN_USER_ACCOUNT};

fn entry() -> Result<keyring::Entry, String> {
    keyring::Entry::new(KEYCHAIN_SERVICE, KEYCHAIN_USER_ACCOUNT)
        .map_err(|e| format!("keychain entry error: {e}"))
}

pub fn save(api_key: &str) -> Result<(), String> {
    entry()?
        .set_password(api_key)
        .map_err(|e| format!("keychain save error: {e}"))
}

pub fn load() -> Result<Option<String>, String> {
    match entry()?.get_password() {
        Ok(p) => Ok(Some(p)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(format!("keychain load error: {e}")),
    }
}

pub fn clear() -> Result<(), String> {
    match entry()?.delete_credential() {
        Ok(_) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(format!("keychain clear error: {e}")),
    }
}
