//! Bearer token plumbing: JWT claim decoding, validity checks, and storage
//! in the OS credential store (macOS Keychain, Windows Credential Manager,
//! Linux secret service). Cookies alone cannot call Graph — we need a bearer
//! whose `aud` is Graph and whose `scp` contains Calendars.*.

use anyhow::{Context, Result};
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use serde_json::Value;
use std::time::{SystemTime, UNIX_EPOCH};

pub fn decode_jwt(token: &str) -> Option<Value> {
    let payload = token.split('.').nth(1)?;
    let bytes = URL_SAFE_NO_PAD.decode(payload).ok()?;
    serde_json::from_slice(&bytes).ok()
}

pub fn is_graph_aud(claims: &Value) -> bool {
    let auds: Vec<String> = match &claims["aud"] {
        Value::Array(a) => a
            .iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect(),
        Value::String(s) => vec![s.clone()],
        _ => vec![],
    };
    auds.iter().any(|a| {
        a == "https://graph.microsoft.com"
            || a == "graph.microsoft.com"
            || a == "00000003-0000-0000-c000-000000000000"
            || a.contains("graph.microsoft.com")
    })
}

pub fn calendar_scopes(claims: &Value) -> Vec<String> {
    claims["scp"]
        .as_str()
        .unwrap_or("")
        .split(' ')
        .filter(|s| s.to_lowercase().starts_with("calendars."))
        .map(String::from)
        .collect()
}

pub fn exp_epoch(claims: &Value) -> Option<u64> {
    claims["exp"].as_u64()
}

pub fn app_name(claims: &Value) -> String {
    claims["app_displayname"]
        .as_str()
        .unwrap_or("?")
        .to_string()
}

fn now_epoch() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Token still valid for at least two minutes, right audience, right scopes.
pub fn usable(claims: &Value) -> bool {
    matches!(exp_epoch(claims), Some(exp) if exp > now_epoch() + 120)
        && is_graph_aud(claims)
        && !calendar_scopes(claims).is_empty()
}

pub struct CachedToken {
    pub token: String,
    pub claims: Value,
}

const SERVICE: &str = "loom-teams";
/// Windows Credential Manager caps a blob at 2560 bytes; Graph bearers run
/// 2–5 KB. Tokens are therefore stored as `graph.0`, `graph.1`, … chunks
/// small enough for every platform's store.
const CHUNK: usize = 2000;

fn entry(index: usize) -> Result<keyring::Entry> {
    keyring::Entry::new(SERVICE, &format!("graph.{index}"))
        .context("opening the system credential store")
}

/// A still-valid token from the OS credential store, or nothing.
pub fn load() -> Option<CachedToken> {
    let mut token = String::new();
    let mut index = 0;
    while let Ok(part) = entry(index).ok()?.get_password() {
        token.push_str(&part);
        index += 1;
    }
    if token.is_empty() {
        return None;
    }
    let claims = decode_jwt(&token)?;
    usable(&claims).then_some(CachedToken { token, claims })
}

pub fn store(token: &str) -> Result<()> {
    // JWTs are ASCII, so byte chunks stay valid UTF-8.
    let chunks: Vec<&str> = token
        .as_bytes()
        .chunks(CHUNK)
        .map(std::str::from_utf8)
        .collect::<Result<_, _>>()
        .context("token is not valid UTF-8")?;
    for (index, part) in chunks.iter().enumerate() {
        entry(index)?
            .set_password(part)
            .context("storing the Graph token in the system credential store")?;
    }
    // Clear stale chunks a longer previous token may have left behind.
    let mut index = chunks.len();
    while entry(index).is_ok_and(|e| e.delete_credential().is_ok()) {
        index += 1;
    }
    Ok(())
}
