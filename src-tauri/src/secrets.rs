//! API key and WebDAV password live in the OS credential store (Windows Credential
//! Manager, macOS Keychain, Secret Service on Linux) instead of plaintext SQLite.
//! If the store is unavailable (e.g. no Secret Service on a headless Linux), the value
//! falls back to the settings table as before, and `PLAINTEXT_FLAG` records that so the
//! settings page can say the secrets are not encrypted instead of hiding it.
//!
//! A non-secret `<name>_set` flag records whether a value exists, so showing the
//! settings page never touches the credential store (no macOS keychain prompt).

use crate::db::Db;

#[cfg(not(any(target_os = "android", target_os = "ios")))]
const SERVICE: &str = "com.eisenhower.app";
pub const API_KEY: &str = "api_key";
pub const WEBDAV_PASSWORD: &str = "webdav_password";
pub const OPENAI_API_KEY: &str = "openai_api_key";

/// Set while at least one secret sits in the database in cleartext.
const PLAINTEXT_FLAG: &str = "secrets_plaintext";

const ALL: [&str; 3] = [API_KEY, WEBDAV_PASSWORD, OPENAI_API_KEY];

/// Android and iOS builds ship without a credential-store backend, so `keyring` is only a
/// dependency on desktop and these two helpers are the only place that knows about it.
#[cfg(not(any(target_os = "android", target_os = "ios")))]
fn entry(name: &str) -> keyring::Result<keyring::Entry> {
    keyring::Entry::new(SERVICE, name)
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
fn keyring_store(name: &str, value: &str) -> bool {
    entry(name).and_then(|e| e.set_password(value)).is_ok()
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
fn keyring_load(name: &str) -> Result<Option<String>, String> {
    match entry(name).and_then(|e| e.get_password()) {
        Ok(value) => Ok(Some(value)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(err) => Err(format!("读取系统凭据失败：{err}")),
    }
}

/// Mobile has no OS credential store in this app: the value stays in the app's private
/// database and `PLAINTEXT_FLAG` makes the settings page say it is not encrypted.
#[cfg(any(target_os = "android", target_os = "ios"))]
fn keyring_store(_name: &str, _value: &str) -> bool {
    false
}

#[cfg(any(target_os = "android", target_os = "ios"))]
fn keyring_load(_name: &str) -> Result<Option<String>, String> {
    Ok(None)
}

fn flag(name: &str) -> String {
    format!("{name}_set")
}

pub fn set(db: &Db, name: &str, value: &str) -> Result<(), String> {
    // Storing "" would set the `<name>_set` flag while get() returns nothing, so the
    // settings page would claim a key is configured that no request can use.
    if value.trim().is_empty() {
        return Err("凭据不能为空".into());
    }
    if keyring_store(name, value) {
        db.delete_setting(name)?;
        refresh_plaintext_flag(db);
    } else {
        // Keep working without a credential store rather than losing the value; the flag
        // below is what tells the user the value is not protected by the OS.
        db.set_setting(name, value)?;
        let _ = db.set_setting(PLAINTEXT_FLAG, "1");
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
    keyring_load(name)
}

pub fn is_set(db: &Db, name: &str) -> Result<bool, String> {
    Ok(db.get_setting(&flag(name))?.as_deref() == Some("1")
        || db.get_setting(name)?.is_some_and(|v| !v.is_empty()))
}

/// Whether any secret is currently stored in the database in cleartext.
pub fn stored_in_plaintext(db: &Db) -> Result<bool, String> {
    Ok(db.get_setting(PLAINTEXT_FLAG)?.as_deref() == Some("1"))
}

/// Recompute the flag from what is actually in the settings table.
fn refresh_plaintext_flag(db: &Db) {
    let any_plain = ALL
        .iter()
        .any(|name| matches!(db.get_setting(name), Ok(Some(value)) if !value.is_empty()));
    if any_plain {
        let _ = db.set_setting(PLAINTEXT_FLAG, "1");
    } else {
        let _ = db.delete_setting(PLAINTEXT_FLAG);
    }
}

/// Move plaintext values written by older versions into the credential store.
pub fn migrate(db: &Db) {
    for name in ALL {
        let Ok(Some(plain)) = db.get_setting(name) else { continue };
        if plain.is_empty() {
            continue;
        }
        if keyring_store(name, &plain) {
            let _ = db.set_setting(&flag(name), "1");
            let _ = db.delete_setting(name);
        }
    }
    // Whatever is still in the settings table could not be moved: say so.
    refresh_plaintext_flag(db);
}
