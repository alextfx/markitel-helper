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

use crate::{api, commands, keychain, mt5_discovery, telemetry};
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

/// Entry point for the Tauri deep-link plugin. The plugin fires its
/// callback on the main thread — NOT inside a Tokio runtime — so we
/// must use `tauri::async_runtime::spawn` (which knows how to attach
/// to Tauri's own runtime) rather than raw `tokio::spawn` (which
/// would panic with "no reactor running").
pub fn handle_deep_link(app: &AppHandle, url: url::Url) {
    let code = match parse_code(&url) {
        Some(c) => c,
        None => {
            log::warn!("markitel:// URL missing ?code= param: {}", url);
            return;
        }
    };
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
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
///
/// Post-pair flow (as of v0.0.6): we DON'T just save the key and bail.
/// We chain straight into `install_ea_inner` so the EA actually lands
/// in MT5 without a second user click. The wizard markets this path
/// as "auto-installs the EA"; this is what makes that promise true.
///
/// If MT5 is running we skip the install (it would need to clobber a
/// loaded EA), emit a `helper://needs-mt5-closed` event, and let the
/// UI prompt the user to close MT5 + click the manual Install button.
pub async fn exchange_and_persist(app: &AppHandle, code: &str) -> Result<PairedEvent, String> {
    telemetry::fire("pair-started", Some(code), None, None);

    let resp = api::exchange_pairing_code(code).await?;
    keychain::save(&resp.api_key)?;

    // Read-back sanity check. On macOS, ad-hoc-signed dev builds can
    // accept SecItemAdd silently without persisting the entry (the
    // access group is tied to a code-sign identity that varies per
    // build). On properly signed production builds the entitlements
    // declare an explicit keychain-access-groups, which fixes this.
    match keychain::load() {
        Ok(Some(_)) => log::info!("keychain save+read-back OK"),
        Ok(None) => log::warn!("keychain save reported OK but read-back returned None"),
        Err(e) => log::warn!("keychain read-back errored: {e}"),
    }

    telemetry::fire("pair-exchanged", Some(code), None, None);

    let event = PairedEvent {
        user_id: resp.user_id.clone(),
        user_email: resp.user_email.clone(),
        api_key_prefix: resp.connection.api_key_prefix.clone(),
        broker_name: resp.connection.broker_name.clone(),
    };
    let _ = app.emit("helper://paired", event.clone());

    // ── Auto-install on pair ────────────────────────────────────────
    // The wizard tells the user this is "one click — auto-installs
    // the EA". Make that true by chaining install right here. We
    // separately emit `helper://needs-mt5-closed` if MT5 is running so
    // the UI can show the actionable Close-MT5 hint instead of
    // pretending nothing happened.
    if mt5_discovery::is_mt5_running() {
        log::info!("MT5 is running — deferring auto-install");
        telemetry::fire("install-deferred-mt5-running", None, None, None);
        let _ = app.emit("helper://needs-mt5-closed", ());
        return Ok(event);
    }

    match commands::install_ea_inner(&resp.api_key).await {
        Ok(install_result) => {
            log::info!(
                "auto-install completed: {} terminal(s) written",
                install_result.written_to.len()
            );
            // Emit the install summary so the UI can show the result
            // screen directly instead of an intermediate "click to
            // install" gate.
            let _ = app.emit("helper://paired-and-installed", install_result);
        }
        Err(e) => {
            log::warn!("auto-install after pair failed: {e}");
            telemetry::fire("install-error", None, Some(&e), None);
            let _ = app.emit("helper://install-error", e);
        }
    }

    Ok(event)
}

