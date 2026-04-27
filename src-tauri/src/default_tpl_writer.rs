//! Writes the keyed Default.tpl into one or more MT5 `templates/`
//! folders. When MT5 sees a `Default.tpl` in this directory, it
//! auto-applies it to every NEW chart the user opens — meaning the EA
//! and the Cayman indicator both auto-attach without the user having
//! to drag anything from the Navigator panel.
//!
//! The template body comes from the server (`api::fetch_default_tpl`)
//! with the user's API key already inlined, so we just write the bytes
//! verbatim.

use std::path::Path;

const TEMPLATE_FILENAME: &str = "Default.tpl";

/// Write the keyed template to `templates_dir/Default.tpl`. Creates the
/// directory if needed (a fresh MT5 install may not have a `templates/`
/// folder yet). Returns the absolute path on success.
pub fn write_to_templates(templates_dir: &Path, content: &str) -> Result<String, String> {
    if !templates_dir.exists() {
        std::fs::create_dir_all(templates_dir)
            .map_err(|e| format!("create templates dir failed: {e}"))?;
    }
    if !templates_dir.is_dir() {
        return Err(format!(
            "templates path is not a directory: {}",
            templates_dir.display()
        ));
    }
    let out = templates_dir.join(TEMPLATE_FILENAME);
    std::fs::write(&out, content)
        .map_err(|e| format!("write Default.tpl failed: {e}"))?;

    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("xattr")
            .arg("-c")
            .arg(&out)
            .status();
    }

    Ok(out.display().to_string())
}
