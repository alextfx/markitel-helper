//! MT5 installation discovery.
//!
//! Heuristics ported from the existing shell / batch installers in
//! `ea/installer_mac.template.sh` and `ea/installer_win.template.bat`.
//! The goal is to find every `<terminal>/MQL5/Experts` directory on the
//! machine where we might want to place the Markitel EA, plus enough
//! context (is MT5 running? which broker build?) for the UI to make
//! good decisions.

use serde::Serialize;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Serialize, Clone, Debug)]
pub struct Terminal {
    /// Absolute path to the terminal data directory — the one that
    /// contains both `MQL5/` and `config/`.
    pub data_dir: String,
    /// Path to `MQL5/Experts`, where we drop the keyed EA.
    pub experts_dir: String,
    /// Path to `MQL5/Indicators`, where we drop the bundled
    /// Sentiment-Indicator.ex5 binary so the EA's iCustom() can resolve
    /// it without manual user placement. Sibling of `experts_dir`.
    pub indicators_dir: String,
    /// Path to `config/`, where terminal.ini and common.ini live.
    pub config_dir: String,
    /// Path to `profiles/`, where our "Markitel" profile would go.
    pub profiles_dir: String,
    /// Best-effort broker build identifier. We lowercase + substring-match
    /// the data_dir path against known broker keywords. Defaults to
    /// "unknown" so the UI can decide whether to surface a warning.
    pub broker_build: String,
}

#[derive(Serialize, Clone, Debug)]
pub struct DiscoveryResult {
    pub terminals: Vec<Terminal>,
    pub mt5_running: bool,
}

const MAX_SCAN_DEPTH: usize = 10;

/// Discover MT5 installations on this machine.
pub async fn discover() -> DiscoveryResult {
    let roots = scan_roots();
    let terminals: Vec<Terminal> = roots
        .into_iter()
        .flat_map(|root| scan_root(&root))
        .collect();

    DiscoveryResult {
        terminals: dedupe(terminals),
        mt5_running: is_mt5_running(),
    }
}

/// List of top-level directories to walk in search of MT5 installs.
fn scan_roots() -> Vec<PathBuf> {
    let mut out = Vec::new();
    #[cfg(target_os = "macos")]
    {
        if let Some(home) = dirs::home_dir() {
            out.push(home.join("Library/Application Support"));
            out.push(home.join("Applications"));
        }
        out.push(PathBuf::from("/Applications"));
    }
    #[cfg(target_os = "windows")]
    {
        // %APPDATA%\MetaQuotes\Terminal\*
        if let Some(appdata) = dirs::data_dir() {
            out.push(appdata.join("MetaQuotes").join("Terminal"));
            out.push(appdata.join("MetaQuotes").join("Terminal").join("Community"));
        }
    }
    #[cfg(target_os = "linux")]
    {
        // Linux users run MT5 under Wine — there's no single canonical
        // path. Scan ~/.wine by default; let the user point at a custom
        // prefix via env var.
        if let Some(home) = dirs::home_dir() {
            out.push(home.join(".wine/drive_c/users"));
        }
    }
    out
}

fn scan_root(root: &Path) -> Vec<Terminal> {
    let mut found = Vec::new();
    if !root.exists() {
        return found;
    }

    for entry in WalkDir::new(root)
        .max_depth(MAX_SCAN_DEPTH)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        // We're looking for directories named "Experts" that live under "MQL5".
        if path.file_name().and_then(|s| s.to_str()) != Some("Experts") {
            continue;
        }
        let parent = match path.parent() {
            Some(p) => p,
            None => continue,
        };
        if parent.file_name().and_then(|s| s.to_str()) != Some("MQL5") {
            continue;
        }
        let data_dir = match parent.parent() {
            Some(p) => p,
            None => continue,
        };

        let config_dir = data_dir.join("config");
        let profiles_dir = data_dir.join("profiles");
        // Sibling of Experts under the same MQL5 parent — used by the
        // helper to drop the bundled Sentiment-Indicator.ex5.
        let indicators_dir = parent.join("Indicators");

        found.push(Terminal {
            data_dir: data_dir.display().to_string(),
            experts_dir: path.display().to_string(),
            indicators_dir: indicators_dir.display().to_string(),
            config_dir: config_dir.display().to_string(),
            profiles_dir: profiles_dir.display().to_string(),
            broker_build: classify_broker(data_dir),
        });
    }
    found
}

fn classify_broker(path: &Path) -> String {
    let lower = path.to_string_lossy().to_lowercase();
    for (needle, label) in [
        ("mtrading", "MTrading"),
        ("amarkets", "AMarkets"),
        ("metaquotes", "MetaQuotes"),
        ("exness", "Exness"),
        ("ic markets", "IC Markets"),
        ("icmarkets", "IC Markets"),
    ] {
        if lower.contains(needle) {
            return label.to_string();
        }
    }
    "Unknown".to_string()
}

fn dedupe(mut terminals: Vec<Terminal>) -> Vec<Terminal> {
    terminals.sort_by(|a, b| a.data_dir.cmp(&b.data_dir));
    terminals.dedup_by(|a, b| a.data_dir == b.data_dir);
    terminals
}

/// True when MT5 is currently running. We use `sysinfo` to list processes
/// and look for the terminal executable. Best effort — a false negative
/// only means we might fail to update `terminal.ini` live, which is fine
/// because the helper flow explicitly asks the user to close MT5 first.
pub fn is_mt5_running() -> bool {
    #[cfg(any(target_os = "windows", target_os = "macos"))]
    {
        use sysinfo::System;
        let mut sys = System::new();
        sys.refresh_processes();
        let names = ["terminal64.exe", "terminal.exe", "MetaTrader 5", "metatrader"];
        for (_pid, proc_) in sys.processes() {
            let n = proc_.name().to_lowercase();
            if names.iter().any(|needle| n.contains(&needle.to_lowercase())) {
                return true;
            }
        }
        false
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        // TODO(linux): parse /proc. Defer until Linux is a supported target.
        false
    }
}
