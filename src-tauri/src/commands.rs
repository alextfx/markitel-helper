//! Tauri `#[command]` handlers. These are the contract between the TS
//! frontend and the Rust backend — each is callable via `invoke()` from
//! JS. Keep the shapes stable and mirror them in `src/lib/types.ts`.

use crate::{api, ea_writer, ini_writer, keychain, mt5_discovery, mt5_launcher, pairing, profile_writer, telemetry};
use serde::Serialize;
use std::path::Path;
use tauri::AppHandle;

#[derive(Serialize, Clone)]
pub struct HelperStatus {
    pub paired: bool,
    #[serde(rename = "apiKeyPrefix")]
    pub api_key_prefix: Option<String>,
    pub version: String,
}

#[tauri::command]
pub async fn status() -> HelperStatus {
    let loaded = keychain::load().ok().flatten();
    let prefix = loaded.as_deref().map(|k| k.chars().take(11).collect::<String>());
    HelperStatus {
        paired: loaded.is_some(),
        api_key_prefix: prefix,
        version: crate::config::HELPER_VERSION.to_string(),
    }
}

#[tauri::command]
pub async fn start_pairing() -> Result<(), String> {
    // Opens the website's connect page in the user's default browser.
    // The website-side flow (SetupWizard) will then POST /pair/start and
    // hand the user back here via a markitel:// deep link.
    let url = format!("{}/broker", crate::config::api_base());
    open_url(&url)
}

#[tauri::command]
pub async fn pair_with_code(app: AppHandle, code: String) -> Result<pairing::PairedEvent, String> {
    pairing::exchange_and_persist(&app, &code.trim().to_uppercase()).await
}

#[tauri::command]
pub async fn discover_mt5() -> mt5_discovery::DiscoveryResult {
    let result = mt5_discovery::discover().await;
    if result.terminals.is_empty() {
        telemetry::fire("mt5-not-found", None, None, None);
    } else {
        telemetry::fire(
            "mt5-detected",
            None,
            None,
            Some(serde_json::json!({ "terminalCount": result.terminals.len() })),
        );
    }
    result
}

#[tauri::command]
pub async fn is_mt5_running() -> bool {
    mt5_discovery::is_mt5_running()
}

#[tauri::command]
pub async fn launch_mt5() -> Result<(), String> {
    let r = mt5_launcher::launch();
    if r.is_ok() {
        telemetry::fire("mt5-launched", None, None, None);
    }
    r
}

#[derive(Serialize)]
pub struct InstallEaResult {
    #[serde(rename = "writtenTo")]
    pub written_to: Vec<String>,
    #[serde(rename = "whitelistResults")]
    pub whitelist_results: Vec<WhitelistSummary>,
    #[serde(rename = "profileResults")]
    pub profile_results: Vec<ProfileSummary>,
}

#[derive(Serialize)]
pub struct WhitelistSummary {
    pub terminal: String,
    pub edited: bool,
    #[serde(rename = "alreadyPresent")]
    pub already_present: bool,
    pub error: Option<String>,
}

#[derive(Serialize)]
pub struct ProfileSummary {
    pub terminal: String,
    pub outcome: profile_writer::ProfileWriteOutcome,
}

#[tauri::command]
pub async fn install_ea() -> Result<InstallEaResult, String> {
    if mt5_discovery::is_mt5_running() {
        return Err("MT5 is currently running. Please close MT5 and try again.".to_string());
    }

    let api_key = keychain::load()?
        .ok_or_else(|| "not paired — run pair_with_code first".to_string())?;

    // Fetch latest EA source from the backend (unkeyed), embed the key locally.
    let ea = api::fetch_ea_source(None)
        .await?
        .ok_or_else(|| "unexpected 304 for fresh ea-source fetch".to_string())?;
    let keyed = ea_writer::render_keyed(&ea.source, &api_key)?;

    let discovery = mt5_discovery::discover().await;
    let mut written_to = Vec::new();
    let mut whitelist_results = Vec::new();
    let mut profile_results = Vec::new();

    for terminal in &discovery.terminals {
        // 1. Write keyed EA.
        match ea_writer::write_to_experts(Path::new(&terminal.experts_dir), &keyed) {
            Ok(path) => {
                written_to.push(path);
                telemetry::fire("ea-written", None, None, None);
            }
            Err(e) => {
                telemetry::fire("error", None, Some(&e), None);
                // Keep going — other terminals might still succeed.
            }
        }

        // 2. Whitelist URL.
        match ini_writer::whitelist_markitel(Path::new(&terminal.config_dir)) {
            Ok(r) => {
                whitelist_results.push(WhitelistSummary {
                    terminal: terminal.data_dir.clone(),
                    edited: r.edited,
                    already_present: r.already_present,
                    error: None,
                });
                if r.edited {
                    telemetry::fire("ini-whitelisted", None, None, None);
                }
            }
            Err(e) => {
                whitelist_results.push(WhitelistSummary {
                    terminal: terminal.data_dir.clone(),
                    edited: false,
                    already_present: false,
                    error: Some(e),
                });
            }
        }

        // 3. Write Markitel profile (STUB — Phase 0).
        match profile_writer::write_profile(Path::new(&terminal.profiles_dir)) {
            Ok(outcome) => {
                if matches!(outcome, profile_writer::ProfileWriteOutcome::Written) {
                    telemetry::fire("profile-written", None, None, None);
                }
                profile_results.push(ProfileSummary {
                    terminal: terminal.data_dir.clone(),
                    outcome,
                });
            }
            Err(e) => {
                telemetry::fire("error", None, Some(&e), None);
            }
        }
    }

    Ok(InstallEaResult {
        written_to,
        whitelist_results,
        profile_results,
    })
}

#[tauri::command]
pub async fn rotate_key() -> Result<String, String> {
    let current = keychain::load()?
        .ok_or_else(|| "not paired".to_string())?;
    let resp = api::rotate_api_key(&current).await?;
    keychain::save(&resp.api_key)?;
    Ok(resp.connection.api_key_prefix)
}

#[tauri::command]
pub async fn log_telemetry(phase: String, error: Option<String>) {
    telemetry::fire(&phase, None, error.as_deref(), None);
}

#[tauri::command]
pub async fn get_helper_version() -> Result<api::HelperVersionInfo, String> {
    api::fetch_helper_version().await
}

// ── helpers ──────────────────────────────────────────────────────────

fn open_url(url: &str) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(url)
            .status()
            .map_err(|e| format!("open URL failed: {e}"))?;
        Ok(())
    }
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(["/C", "start", "", url])
            .status()
            .map_err(|e| format!("open URL failed: {e}"))?;
        Ok(())
    }
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(url)
            .status()
            .map_err(|e| format!("open URL failed: {e}"))?;
        Ok(())
    }
}
