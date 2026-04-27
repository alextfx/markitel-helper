//! Writes the bundled Cayman Sentiment-Indicator.ex5 binary into one or
//! more MT5 Indicators folders. Mirrors the pattern in `ea_writer.rs`
//! — same caller pattern, same macOS-quarantine clearing, same
//! per-terminal call site.
//!
//! The binary itself is NOT compiled into the helper. We fetch it from
//! the Markitel server via `api::fetch_indicator()` so the team can
//! ship indicator updates without re-releasing the helper installer.

use std::path::Path;

const INDICATOR_FILENAME: &str = "Sentiment-Indicator.ex5";

/// Write the indicator binary to `indicators_dir/Sentiment-Indicator.ex5`.
/// Creates the directory if it doesn't exist (fresh MT5 installs may
/// not have an Indicators/ folder until the user opens an indicator
/// for the first time). Returns the absolute path written on success.
pub fn write_to_indicators(indicators_dir: &Path, bytes: &[u8]) -> Result<String, String> {
    if !indicators_dir.exists() {
        std::fs::create_dir_all(indicators_dir)
            .map_err(|e| format!("create indicators dir failed: {e}"))?;
    }
    if !indicators_dir.is_dir() {
        return Err(format!(
            "indicators path is not a directory: {}",
            indicators_dir.display()
        ));
    }
    let out = indicators_dir.join(INDICATOR_FILENAME);
    std::fs::write(&out, bytes)
        .map_err(|e| format!("write indicator failed: {e}"))?;

    // macOS: clear the quarantine xattr. Same belt-and-suspenders as
    // ea_writer — std::fs::write shouldn't set it, but if anything
    // along the way did, MT5 will silently refuse to load a quarantined
    // binary.
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("xattr")
            .arg("-c")
            .arg(&out)
            .status();
    }

    Ok(out.display().to_string())
}
