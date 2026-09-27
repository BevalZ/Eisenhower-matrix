//! API key and WebDAV password live in the OS credential store (Windows Credential
//! Manager, macOS Keychain, Secret Service on Linux) instead of plaintext SQLite.
//! If the store is unavailable (e.g. no Secret Service on a headless Linux), the value
//! falls back to the settings table as before.
//!
//! A non-secret `<name>_set` flag records whether a value exists, so showing the
//! settings page never touches the credential store (no macOS keychain prompt).

use crate::db::Db;

const SERVICE: &str = "com.eisenhower.app";
pub const API_KEY: &str = "api_key";
pub const WEBDAV_PASSWORD: &str = "webdav_password";

fn entry(name: &str) -> keyring::Result<keyring::Entry> {
    keyring::Entry::new(SERVICE, name)
}

fn flag(name: &str) -> String {
    format!("{name}_set")
}

pub fn set(db: &Db, name: &str, value: &str) -> Result<(), String> {
    match entry(name).and_then(|e| e.set_password(value)) {
        Ok(()) => db.delete_setting(name)?,
        // Keep working without a credential store rather than losing the value.
        Err(_) => db.set_setting(name, value)?,
    }
    db.set_setting(&flag(name), "1")
}

pub fn get(db: &Db, name: &str) -> Result<Option<String>, String> {
    if let Some(plain) = db.get_setting(name)?.filter(|v| !v.is_empty()) {
        return Ok(Some(plain));
    }
    if !is_set(db, name)? {
        return Ok(None);
    }
    match entry(name).and_then(|e| e.get_password()) {
        Ok(value) => Ok(Some(value)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(err) => Err(format!("读取系统凭据失败：{err}")),
    }
}

pub fn is_set(db: &Db, name: &str) -> Result<bool, String> {
    Ok(db.get_setting(&flag(name))?.as_deref() == Some("1")
        || db.get_setting(name)?.is_some_and(|v| !v.is_empty()))
}

/// Move plaintext values written by older versions into the credential store.
pub fn migrate(db: &Db) {
    for name in [API_KEY, WEBDAV_PASSWORD] {
        let Ok(Some(plain)) = db.get_setting(name) else { continue };
        if plain.is_empty() {
            continue;
        }
        if entry(name).and_then(|e| e.set_password(&plain)).is_ok() {
            let _ = db.set_setting(&flag(name), "1");
            let _ = db.delete_setting(name);
        }
    }
}
