//! terminal.ini / common.ini editing to whitelist `https://markitel.com`
//! for WebRequest. MT5 reads these at startup; writing while MT5 is
//! running gets clobbered on quit.
//!
//! Strategy:
//!   1. Refuse to edit if MT5 is currently running (caller's job to
//!      check mt5_discovery::is_mt5_running first, but we guard here too).
//!   2. Backup the existing file as `<name>.markitel.bak` on first edit.
//!   3. If `[Experts]` section exists, find the highest `WebRequestURL_N`
//!      key and add a new one one index above. If the URL is already
//!      listed, no-op.
//!   4. If `[Experts]` section doesn't exist, append it with WebRequestURL_0.
//!
//! NOTE: Phase 0 spike determines whether to edit `terminal.ini` (per
//! terminal) or `common.ini` (shared across terminals, may be more
//! persistent). We default to `terminal.ini` for now — the current
//! installer does the same.

use std::path::{Path, PathBuf};

const MARKITEL_URL: &str = "https://markitel.com";
const SECTION: &str = "[Experts]";
const URL_KEY_PREFIX: &str = "WebRequestURL_";

pub struct IniEditResult {
    pub edited: bool,
    pub already_present: bool,
    pub backed_up_to: Option<String>,
}

pub fn whitelist_markitel(config_dir: &Path) -> Result<IniEditResult, String> {
    let target = config_dir.join("terminal.ini");
    if !target.exists() {
        // MT5 creates terminal.ini on first launch; if it isn't here yet
        // we write a fresh one with just the section we need.
        return write_fresh(&target).map(|_| IniEditResult {
            edited: true,
            already_present: false,
            backed_up_to: None,
        });
    }

    let content = std::fs::read_to_string(&target)
        .map_err(|e| format!("read terminal.ini failed: {e}"))?;

    if url_is_whitelisted(&content) {
        return Ok(IniEditResult {
            edited: false,
            already_present: true,
            backed_up_to: None,
        });
    }

    let backup = backup_once(&target, &content)?;
    let updated = patch_content(&content);
    std::fs::write(&target, updated)
        .map_err(|e| format!("write terminal.ini failed: {e}"))?;

    Ok(IniEditResult {
        edited: true,
        already_present: false,
        backed_up_to: backup,
    })
}

fn backup_once(target: &Path, content: &str) -> Result<Option<String>, String> {
    let backup_path: PathBuf = target.with_extension("ini.markitel.bak");
    if backup_path.exists() {
        return Ok(Some(backup_path.display().to_string()));
    }
    std::fs::write(&backup_path, content)
        .map_err(|e| format!("write backup failed: {e}"))?;
    Ok(Some(backup_path.display().to_string()))
}

fn write_fresh(target: &Path) -> Result<(), String> {
    let fresh = format!("{SECTION}\n{URL_KEY_PREFIX}0={MARKITEL_URL}\n");
    std::fs::write(target, fresh)
        .map_err(|e| format!("write fresh terminal.ini failed: {e}"))
}

fn url_is_whitelisted(content: &str) -> bool {
    for line in content.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with(URL_KEY_PREFIX) {
            continue;
        }
        if let Some(eq) = trimmed.find('=') {
            let value = trimmed[eq + 1..].trim().trim_end_matches('/');
            if value == MARKITEL_URL.trim_end_matches('/') {
                return true;
            }
        }
    }
    false
}

fn patch_content(content: &str) -> String {
    // Find max existing index under [Experts].
    let mut in_experts = false;
    let mut max_idx: i32 = -1;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            in_experts = trimmed.eq_ignore_ascii_case(SECTION);
            continue;
        }
        if in_experts && trimmed.starts_with(URL_KEY_PREFIX) {
            if let Some(eq) = trimmed.find('=') {
                let key = &trimmed[..eq];
                let idx_str = &key[URL_KEY_PREFIX.len()..];
                if let Ok(idx) = idx_str.parse::<i32>() {
                    if idx > max_idx {
                        max_idx = idx;
                    }
                }
            }
        }
    }

    let new_idx = max_idx + 1;
    let new_line = format!("{URL_KEY_PREFIX}{new_idx}={MARKITEL_URL}");

    // Splice: append under existing [Experts] or add a fresh section at end.
    if content.lines().any(|l| l.trim().eq_ignore_ascii_case(SECTION)) {
        let mut out = String::new();
        let mut appended = false;
        let mut current_section_is_experts = false;
        let lines: Vec<&str> = content.lines().collect();
        for (i, line) in lines.iter().enumerate() {
            out.push_str(line);
            out.push('\n');
            let trimmed = line.trim();
            if trimmed.starts_with('[') && trimmed.ends_with(']') {
                current_section_is_experts = trimmed.eq_ignore_ascii_case(SECTION);
                continue;
            }
            // Append the new URL at the end of the [Experts] block — just
            // before the next `[Section]` header, or at EOF.
            let is_last = i + 1 == lines.len();
            let next_is_section = !is_last
                && lines[i + 1].trim().starts_with('[')
                && lines[i + 1].trim().ends_with(']');
            if current_section_is_experts && !appended && (is_last || next_is_section) {
                out.push_str(&new_line);
                out.push('\n');
                appended = true;
            }
        }
        if !appended {
            out.push_str(&new_line);
            out.push('\n');
        }
        out
    } else {
        let mut out = content.to_string();
        if !out.ends_with('\n') {
            out.push('\n');
        }
        out.push_str(SECTION);
        out.push('\n');
        out.push_str(&new_line);
        out.push('\n');
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_op_when_already_whitelisted() {
        let content = format!("{SECTION}\n{URL_KEY_PREFIX}0={MARKITEL_URL}\n");
        assert!(url_is_whitelisted(&content));
    }

    #[test]
    fn adds_to_existing_experts_section_with_next_index() {
        let content = format!(
            "{SECTION}\n{URL_KEY_PREFIX}0=https://other.com\n{URL_KEY_PREFIX}1=https://another.com\n"
        );
        let patched = patch_content(&content);
        assert!(patched.contains(&format!("{URL_KEY_PREFIX}2={MARKITEL_URL}")));
    }

    #[test]
    fn creates_experts_section_when_missing() {
        let content = "[Common]\nLanguage=English\n";
        let patched = patch_content(content);
        assert!(patched.contains(SECTION));
        assert!(patched.contains(&format!("{URL_KEY_PREFIX}0={MARKITEL_URL}")));
    }

    #[test]
    fn preserves_trailing_newline_behavior() {
        let content = "[Common]\nLanguage=English";
        let patched = patch_content(content);
        assert!(patched.ends_with('\n'));
    }
}
