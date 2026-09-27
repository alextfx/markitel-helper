//! Writes the keyed Markitel_Bridge.tpl to MT5's named template folder.
//! The customer applies it to exactly one dedicated chart. The legacy
//! Markitel.tpl alias is refreshed with the same credentials.

use std::path::Path;

const TEMPLATE_FILENAME: &str = "Markitel_Bridge.tpl";
const LEGACY_ALIAS: &str = "Markitel.tpl";

/// Write both named templates under `MQL5/Profiles/Templates`.
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
    std::fs::write(&out, content).map_err(|e| format!("write Markitel_Bridge.tpl failed: {e}"))?;
    std::fs::write(templates_dir.join(LEGACY_ALIAS), content)
        .map_err(|e| format!("write Markitel.tpl failed: {e}"))?;

    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("xattr")
            .arg("-c")
            .arg(&out)
            .status();
    }

    Ok(out.display().to_string())
}

/// Disable only a Markitel-owned legacy global template. Preserve its bytes
/// in a sibling backup; never alter a customer's unrelated Default.tpl.
pub fn disable_legacy_auto_template(path: &Path) -> Result<(), String> {
    if !path.is_file() {
        return Ok(());
    }
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("read legacy Default.tpl failed: {e}"))?;
    if !content
        .lines()
        .any(|line| line.trim_end() == "name=Markitel_Bridge")
    {
        return Ok(());
    }
    let parent = path
        .parent()
        .ok_or("legacy template has no parent directory")?;
    let mut backup = parent.join("Default.markitel-legacy-disabled");
    if backup.exists() {
        backup = parent.join(format!(
            "Default.markitel-legacy-disabled.{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|e| format!("clock unavailable: {e}"))?
                .as_nanos(),
        ));
    }
    std::fs::rename(path, &backup)
        .map_err(|e| format!("disable legacy Markitel Default.tpl failed: {e}"))?;
    if path.exists() {
        return Err("unsafe legacy Markitel Default.tpl remains active".to_string());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_only_named_templates_and_preserves_an_unrelated_default() {
        let dir =
            std::env::temp_dir().join(format!("markitel-helper-template-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("Default.tpl"), "user chart").unwrap();
        write_to_templates(&dir, "name=Markitel_Bridge\nApiKey=test\n").unwrap();
        disable_legacy_auto_template(&dir.join("Default.tpl")).unwrap();
        assert_eq!(
            std::fs::read_to_string(dir.join("Default.tpl")).unwrap(),
            "user chart"
        );
        assert_eq!(
            std::fs::read_to_string(dir.join(TEMPLATE_FILENAME)).unwrap(),
            "name=Markitel_Bridge\nApiKey=test\n"
        );
        assert_eq!(
            std::fs::read_to_string(dir.join(LEGACY_ALIAS)).unwrap(),
            "name=Markitel_Bridge\nApiKey=test\n"
        );
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn disables_only_the_markitel_global_template() {
        let dir =
            std::env::temp_dir().join(format!("markitel-helper-legacy-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let legacy = dir.join("Default.tpl");
        std::fs::write(&legacy, "<expert>\nname=Markitel_Bridge\n</expert>\n").unwrap();
        disable_legacy_auto_template(&legacy).unwrap();
        assert!(!legacy.exists());
        assert!(dir.join("Default.markitel-legacy-disabled").is_file());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
