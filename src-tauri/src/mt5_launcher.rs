//! Cross-platform "launch / terminate MT5" helpers.
//!
//! `launch()` opens MT5 via the OS app handler and returns immediately
//! — we don't attach to its process. `terminate()` force-quits any
//! running MT5 process(es) so the helper can replace EA/indicator files
//! safely (MT5 holds open file handles on .mq5/.ex5 while running and
//! Windows refuses to overwrite them). After `terminate()` the caller
//! should sleep ~500ms before file writes to let the OS release locks.
//!
//! Both functions are idempotent: launch when already-running is a
//! no-op for `open -a` / `cmd start`; terminate when not-running just
//! returns Ok.

use std::time::Duration;

// ── launch ────────────────────────────────────────────────────────────

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

// ── terminate ─────────────────────────────────────────────────────────
//
// Used by the install flow to make file replacement safe. We try a
// graceful quit first (gives MT5 a chance to flush state), then a hard
// kill if the process is still alive. After a successful terminate the
// caller should call `wait_until_exited()` to confirm the OS has
// released the file locks before writing.

#[cfg(target_os = "macos")]
pub fn terminate() -> Result<(), String> {
    use std::process::Command;
    // 1. Graceful quit via AppleScript — lets MT5 close charts cleanly.
    let _ = Command::new("osascript")
        .args(["-e", r#"tell application "MetaTrader 5" to quit"#])
        .status();
    // 2. Hard kill any stragglers. `pkill -x` matches the exact process
    //    name; -9 is SIGKILL. Returns non-zero when nothing matched —
    //    we ignore that, idempotent.
    let _ = Command::new("pkill")
        .args(["-9", "-x", "MetaTrader 5"])
        .status();
    // Some broker builds rename the binary; sweep common variants.
    for name in ["terminal64", "terminal", "MetaTrader5"] {
        let _ = Command::new("pkill").args(["-9", "-x", name]).status();
    }
    Ok(())
}

#[cfg(target_os = "windows")]
pub fn terminate() -> Result<(), String> {
    use std::process::Command;
    // taskkill returns non-zero when the image isn't found — fine,
    // idempotent. /F = force, /T = include child processes.
    for image in ["terminal64.exe", "terminal.exe", "MetaTrader 5.exe"] {
        let _ = Command::new("taskkill")
            .args(["/F", "/T", "/IM", image])
            .status();
    }
    Ok(())
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub fn terminate() -> Result<(), String> {
    Err("terminate_mt5 unsupported on this OS".to_string())
}

// ── wait_until_exited ─────────────────────────────────────────────────
//
// Polls `mt5_discovery::is_mt5_running()` until it returns false (or
// the timeout elapses). Used after `terminate()` so the install loop
// only attempts file writes once the OS has released MT5's file locks.

pub fn wait_until_exited(timeout: Duration) -> bool {
    use std::thread::sleep;
    use std::time::Instant;
    let start = Instant::now();
    loop {
        if !crate::mt5_discovery::is_mt5_running() {
            return true;
        }
        if start.elapsed() >= timeout {
            return false;
        }
        sleep(Duration::from_millis(150));
    }
}
