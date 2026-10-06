//! Shared reqwest clients.
//!
//! Building a client per request throws away the connection pool, the DNS cache and the
//! TLS session, so every AI classification and every WebDAV round paid a fresh handshake.
//! Two clients are enough: one that follows redirects (AI providers) and one that does
//! not (WebDAV, where a redirect means the configured address is wrong).
//! Each caller sets its own per-request timeout.

use std::sync::OnceLock;

use reqwest::redirect::Policy;

static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
static NO_REDIRECT: OnceLock<reqwest::Client> = OnceLock::new();

pub fn client() -> Result<&'static reqwest::Client, String> {
    cached(&CLIENT, None)
}

pub fn client_no_redirect() -> Result<&'static reqwest::Client, String> {
    cached(&NO_REDIRECT, Some(Policy::none()))
}

fn cached(
    slot: &'static OnceLock<reqwest::Client>,
    redirect: Option<Policy>,
) -> Result<&'static reqwest::Client, String> {
    if let Some(client) = slot.get() {
        return Ok(client);
    }
    let mut builder = reqwest::Client::builder();
    if let Some(policy) = redirect {
        builder = builder.redirect(policy);
    }
    let built = builder
        .build()
        .map_err(|e| format!("无法创建网络客户端: {e}"))?;
    Ok(slot.get_or_init(|| built))
}
