//! Tauri `#[command]` handlers. These are the contract between the TS
//! frontend and the Rust backend — each is callable via `invoke()` from
//! JS. Keep the shapes stable and mirror them in `src/lib/types.ts`.

use crate::{
    api, ea_writer, indicator_writer, ini_writer, keychain, mt5_discovery, mt5_launcher,
    named_tpl_writer, pairing, profile_writer, telemetry,
};
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
    let prefix = loaded
        .as_deref()
        .map(|k| k.chars().take(11).collect::<String>());
    log::info!("status(): paired={}, prefix={:?}", loaded.is_some(), prefix,);
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

// `Clone` is needed because we emit this struct as a Tauri event
// payload, and `Emitter::emit` requires `S: Serialize + Clone`.
#[derive(Serialize, Clone)]
pub struct InstallEaResult {
    #[serde(rename = "writtenTo")]
    pub written_to: Vec<String>,
    #[serde(rename = "whitelistResults")]
    pub whitelist_results: Vec<WhitelistSummary>,
    #[serde(rename = "profileResults")]
    pub profile_results: Vec<ProfileSummary>,
}

#[derive(Serialize, Clone)]
pub struct WhitelistSummary {
    pub terminal: String,
    pub edited: bool,
    #[serde(rename = "alreadyPresent")]
    pub already_present: bool,
    pub error: Option<String>,
}

#[derive(Serialize, Clone)]
pub struct ProfileSummary {
    pub terminal: String,
    pub outcome: profile_writer::ProfileWriteOutcome,
}

/// Internal install body, callable from both the manual `install_ea`
/// command and the post-pair auto-install path in `pairing.rs`. Takes
/// the api_key as an explicit parameter so callers don't have to
/// double-load from the keychain (and so the post-pair flow can
/// install before the keychain entry's persistence has been stress
/// tested across launches — the caller already has the fresh key in
/// memory).
///
/// Bulletproof against any MT5 state: if MT5 is running, force-quits
/// it (graceful AppleScript / WM_CLOSE first, then SIGKILL / taskkill
/// /F), waits for file locks to release, runs the install (which
/// REPLACES any existing Markitel_Bridge files — std::fs::write
/// overwrites), then auto-relaunches MT5 so the user's only remaining
/// action is on markitel.com.
pub async fn install_ea_inner(api_key: &str) -> Result<InstallEaResult, String> {
    use std::time::Duration;

    // Acquire every required asset before interrupting a running terminal.
    // The compiled binary is generic; the named template carries both inputs.
    let ea = api::fetch_ea_source(None)
        .await?
        .ok_or_else(|| "unexpected 304 for fresh ea-source fetch".to_string())?;
    let keyed = ea_writer::render_keyed(&ea.source, api_key)?;
    let binary = api::fetch_ea_binary(api_key).await?;
    let named_template = api::fetch_markitel_tpl(api_key).await?;
    let discovery = mt5_discovery::discover().await;
    if discovery.terminals.is_empty() {
        telemetry::fire("mt5-not-found", None, None, None);
        return Err(
            "No MT5 terminal data folder was found. Open MT5 once, then retry the install."
                .to_string(),
        );
    }

    // Optional download also happens before MT5 is interrupted.
    let indicator_bytes = match api::fetch_indicator(api_key).await {
        Ok(bytes) => Some(bytes),
        Err(e) => {
            log::warn!("indicator fetch failed ({e}); per-asset Cayman scoring will use F&G fallback until indicator is placed manually");
            telemetry::fire("indicator-fetch-failed", None, Some(&e), None);
            None
        }
    };

    // If MT5 is running, kill it first. On Windows the .mq5/.ex5 files
    // are file-locked while MT5 has them loaded — overwriting would
    // fail with permission-denied. After SIGKILL/taskkill we poll for
    // the process to actually exit (file locks are released a few ms
    // after the process dies).
    let mt5_was_running = mt5_discovery::is_mt5_running();
    if mt5_was_running {
        log::info!("MT5 detected running — terminating before install");
        telemetry::fire("mt5-terminate-attempt", None, None, None);
        if let Err(e) = mt5_launcher::terminate() {
            log::warn!("terminate command returned error: {e}");
        }
        let exited = mt5_launcher::wait_until_exited(Duration::from_secs(8));
        if !exited {
            telemetry::fire("mt5-terminate-timeout", None, None, None);
            return Err(
                "MT5 is still running after force-quit attempt. Please close MT5 manually and try again.".to_string()
            );
        }
        // Brief grace period for the OS to release file handles MT5
        // had open. 500ms is conservative — empirically locks release
        // within ~50-150ms of process exit.
        std::thread::sleep(Duration::from_millis(500));
        telemetry::fire("mt5-terminated", None, None, None);
    }

    log::info!(
        "install_ea_inner: discovered {} MT5 terminal(s)",
        discovery.terminals.len()
    );
    for terminal in &discovery.terminals {
        log::info!(
            "  · broker={} data_dir={} experts={}",
            terminal.broker_build,
            terminal.data_dir,
            terminal.experts_dir
        );
    }

    telemetry::fire(
        "mt5-detected",
        None,
        None,
        Some(serde_json::json!({ "terminalCount": discovery.terminals.len() })),
    );

    let mut written_to = Vec::new();
    let mut whitelist_results = Vec::new();
    let mut profile_results = Vec::new();

    let write_result = (|| -> Result<(), String> {
        for terminal in &discovery.terminals {
            // Never report this terminal installed unless source, executable and
            // one-chart template all landed. A source-only install cannot run.
            let mql5_dir = Path::new(&terminal.experts_dir)
                .parent()
                .ok_or_else(|| "MT5 Experts folder has no MQL5 parent".to_string())?;
            let tpl_dir = mql5_dir.join("Profiles").join("Templates");
            let legacy_defaults = [
                tpl_dir.join("Default.tpl"),
                Path::new(&terminal.data_dir)
                    .join("templates")
                    .join("Default.tpl"),
                Path::new(&terminal.data_dir)
                    .parent()
                    .ok_or_else(|| "MT5 data folder has no parent".to_string())?
                    .join("templates")
                    .join("Default.tpl"),
            ];
            for legacy in &legacy_defaults {
                named_tpl_writer::disable_legacy_auto_template(legacy)?;
            }
            ea_writer::write_to_experts(Path::new(&terminal.experts_dir), &keyed)?;
            ea_writer::write_binary_to_experts(Path::new(&terminal.experts_dir), &binary)?;
            let path = named_tpl_writer::write_to_templates(&tpl_dir, &named_template)?;
            written_to.push(path);
            telemetry::fire("ea-written", None, None, None);

            // 1.5 Write Cayman sentiment indicator (best-effort).
            // Skipped silently if the upstream fetch failed earlier — the
            // EA still installs and the engine's universal F&G fallback
            // covers the gap until the indicator is placed manually.
            if let Some(ref bytes) = indicator_bytes {
                match indicator_writer::write_to_indicators(
                    Path::new(&terminal.indicators_dir),
                    bytes,
                ) {
                    Ok(path) => {
                        log::info!("wrote indicator to: {}", path);
                        telemetry::fire("indicator-written", None, None, None);
                    }
                    Err(e) => {
                        log::warn!(
                            "failed to write indicator to {}: {}",
                            terminal.indicators_dir,
                            e
                        );
                        telemetry::fire("error", None, Some(&e), None);
                    }
                }
            }

            // 2. Whitelist URL.
            match ini_writer::whitelist_markitel(Path::new(&terminal.config_dir)) {
                Ok(r) => {
                    log::info!(
                        "ini whitelist: {} edited={} already_present={}",
                        terminal.config_dir,
                        r.edited,
                        r.already_present
                    );
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
                    log::warn!("ini whitelist failed for {}: {}", terminal.config_dir, e);
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
                    log::warn!("profile_writer failed for {}: {}", terminal.profiles_dir, e);
                    telemetry::fire("error", None, Some(&e), None);
                }
            }
        }
        Ok(())
    })();

    // Auto-relaunch MT5 so the user's only remaining action is on
    // markitel.com. If MT5 wasn't running before install we still
    // launch it — the install just happened, the user wants to see
    // the EA + indicator working. Failure to launch is non-fatal:
    // we already wrote everything, the user can open MT5 themselves.
    match mt5_launcher::launch() {
        Ok(()) => {
            log::info!(
                "auto-relaunched MT5 (was_running_before={})",
                mt5_was_running
            );
            telemetry::fire("mt5-launched", None, None, None);
        }
        Err(e) => {
            log::warn!("auto-relaunch MT5 failed: {e}");
            telemetry::fire("mt5-launch-failed", None, Some(&e), None);
        }
    }

    write_result?;

    Ok(InstallEaResult {
        written_to,
        whitelist_results,
        profile_results,
    })
}

/// Tauri command exposed to the UI for manual / repair install.
/// Used when the user hits "Reinstall EA" from the tray.
///
/// No longer rejects when MT5 is running — `install_ea_inner` handles
/// the force-quit + relaunch flow internally so the install is
/// bulletproof regardless of MT5 state.
#[tauri::command]
pub async fn install_ea() -> Result<InstallEaResult, String> {
    let api_key =
        keychain::load()?.ok_or_else(|| "not paired — run pair_with_code first".to_string())?;
    install_ea_inner(&api_key).await
}

#[tauri::command]
pub async fn rotate_key() -> Result<String, String> {
    let current = keychain::load()?.ok_or_else(|| "not paired".to_string())?;
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
