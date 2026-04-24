//! Writes the Markitel_Bridge.mq5 EA to one or more MT5 Experts folders,
//! with the user's API key embedded as the default value of the
//! `input string ApiKey` parameter. Mirrors the template substitution in
//! `lib/services/bridge/ea-template.ts` so behavior stays consistent
//! with the existing server-rendered flow.

use std::path::Path;

/// The server's template has this marker as the default-value line. We
/// swap the empty-string default for the user's key. Must stay in sync
/// with `API_KEY_NEEDLE` in lib/services/bridge/ea-template.ts.
const API_KEY_NEEDLE: &str = r#"input string   ApiKey          = "";"#;

/// Regex-free validation mirroring `KEY_PATTERN = /^mk_[a-f0-9]{64}$/`.
pub fn is_valid_key(key: &str) -> bool {
    if !key.starts_with("mk_") {
        return false;
    }
    let tail = &key[3..];
    tail.len() == 64 && tail.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
}

/// Embed `api_key` into `source` and return the keyed EA as a String.
pub fn render_keyed(source: &str, api_key: &str) -> Result<String, String> {
    if !is_valid_key(api_key) {
        return Err("invalid api key format".to_string());
    }
    if !source.contains(API_KEY_NEEDLE) {
        return Err("EA source missing ApiKey marker".to_string());
    }
    let replacement = format!(r#"input string   ApiKey          = "{api_key}";"#);
    Ok(source.replace(API_KEY_NEEDLE, &replacement))
}

/// Write the keyed EA source to `experts_dir/Markitel_Bridge.mq5`.
/// Returns the absolute path written on success. Caller is expected to
/// have already validated that MT5 is closed — this function does not
/// retry on permission errors.
pub fn write_to_experts(experts_dir: &Path, keyed_source: &str) -> Result<String, String> {
    if !experts_dir.exists() {
        return Err(format!(
            "experts directory does not exist: {}",
            experts_dir.display()
        ));
    }
    if !experts_dir.is_dir() {
        return Err(format!(
            "experts path is not a directory: {}",
            experts_dir.display()
        ));
    }
    let out = experts_dir.join("Markitel_Bridge.mq5");
    std::fs::write(&out, keyed_source)
        .map_err(|e| format!("write EA failed: {e}"))?;

    // macOS: clear the quarantine xattr. Writing the file via std::fs
    // shouldn't set the attribute in the first place (that's an
    // AppleDouble-on-download thing) but we belt-and-suspenders it.
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("xattr")
            .arg("-c")
            .arg(&out)
            .status();
    }

    Ok(out.display().to_string())
}
