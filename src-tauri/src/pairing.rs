//! Pairing flow. Two entry points:
//!
//!   1. handle_deep_link(url) — called when the user clicks a
//!      `markitel://pair?code=XXXXXX` link in their browser. We parse
//!      the code out and run the exchange.
//!
//!   2. pair_with_code(code) — called from the UI when the user pastes
//!      a code into the helper's "Pair manually" input (fallback when
//!      the OS doesn't auto-hand the URL to us).
//!
//! After a successful exchange we:
//!   - persist the API key to the OS keychain
//!   - fire `pair-exchanged` telemetry
//!   - emit `helper://paired` to the frontend so it can advance the UI

use crate::{api, keychain, telemetry};
use serde::Serialize;
use tauri::{AppHandle, Emitter};
use url::Url;

#[derive(Serialize, Clone)]
pub struct PairedEvent {
    #[serde(rename = "userId")]
    pub user_id: String,
    #[serde(rename = "userEmail")]
    pub user_email: Option<String>,
    #[serde(rename = "apiKeyPrefix")]
    pub api_key_prefix: String,
    #[serde(rename = "brokerName")]
    pub broker_name: String,
}

/// Entry point for the Tauri deep-link plugin. Runs async on the tokio
/// runtime the plugin exposes.
pub fn handle_deep_link(app: &AppHandle, url: url::Url) {
    let code = match parse_code(&url) {
        Some(c) => c,
        None => {
            log::warn!("markitel:// URL missing ?code= param: {}", url);
            return;
        }
    };
    let app = app.clone();
    tokio::spawn(async move {
        if let Err(e) = exchange_and_persist(&app, &code).await {
            log::error!("pair exchange failed: {e}");
            telemetry::fire("error", Some(&code), Some(&e), None);
            let _ = app.emit("helper://pair-error", e);
        }
    });
}

/// Parse `?code=XXXXXX` from a markitel://pair URL, uppercasing + trimming.
fn parse_code(url: &Url) -> Option<String> {
    for (k, v) in url.query_pairs() {
        if k == "code" {
            let cleaned = v.trim().to_uppercase();
            if !cleaned.is_empty() {
                return Some(cleaned);
            }
        }
    }
    None
}

/// Callable from the UI (via the `pair_with_code` Tauri command) so users
/// can paste the 6-char code manually if the OS refused the deep link.
pub async fn exchange_and_persist(app: &AppHandle, code: &str) -> Result<PairedEvent, String> {
    telemetry::fire("pair-started", Some(code), None, None);

    let resp = api::exchange_pairing_code(code).await?;
    keychain::save(&resp.api_key)?;

    telemetry::fire("pair-exchanged", Some(code), None, None);

    let event = PairedEvent {
        user_id: resp.user_id.clone(),
        user_email: resp.user_email.clone(),
        api_key_prefix: resp.connection.api_key_prefix.clone(),
        broker_name: resp.connection.broker_name.clone(),
    };
    let _ = app.emit("helper://paired", event.clone());

    Ok(event)
}

// ── Tauri 2.x deep-link extension trait plumbing ──────────────────────
//
// In Tauri 2.x, the deep-link plugin installs itself onto AppHandle via
// a trait extension — `use tauri_plugin_deep_link::DeepLinkExt;`. That
// trait is NOT imported by default, so we re-export it here for
// convenience + to centralize the version dependency.
pub use tauri_plugin_deep_link::DeepLinkExt;
