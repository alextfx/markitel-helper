//! Cross-platform "launch MT5" helper. We don't attach to MT5's process
//! or wait for it — we just fire the OS "open this app" call and return.

#[cfg(target_os = "macos")]
pub fn launch() -> Result<(), String> {
    use std::process::Command;
    // The official MetaQuotes MT5 Mac build registers as "MetaTrader 5".
    // Broker-branded builds often use their own name; the helper should
    // display a fallback "Open MT5 manually" button if this fails.
    Command::new("open")
        .arg("-a")
        .arg("MetaTrader 5")
        .status()
        .map_err(|e| format!("open MT5 failed: {e}"))?;
    Ok(())
}

#[cfg(target_os = "windows")]
pub fn launch() -> Result<(), String> {
    use std::process::Command;
    // On Windows the canonical launch is via the Start-menu shortcut,
    // but the executable is always `terminal64.exe` inside the install
    // dir. We let Windows resolve the default "MetaTrader 5" app via
    // `start` + the protocol association it creates at install time.
    Command::new("cmd")
        .args(["/C", "start", "", "MetaTrader 5"])
        .status()
        .map_err(|e| format!("start MT5 failed: {e}"))?;
    Ok(())
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub fn launch() -> Result<(), String> {
    // Linux users run MT5 under Wine — no reliable programmatic launch.
    // The UI should fall back to "Please open MT5 yourself."
    Err("launch_mt5 unsupported on this OS".to_string())
}
