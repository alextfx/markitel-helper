//! Creates an isolated `Markitel` MT5 profile so the EA auto-attaches on
//! user's next profile switch — NEVER touches the default profile.
//!
//! STUB — the exact `chart01.chr` + `.tpl` format differs across MT5
//! broker builds. The Phase 0 spike (see `ea/SPIKES.md`) captures the
//! format for each of the 4 target variants. Until then this function:
//!
//!   - Creates the `profiles/Markitel/` directory
//!   - Writes a placeholder `chart01.chr` with a known-unreliable format
//!   - Returns `ProfileWriteOutcome::NeedsManualDrag` so the UI shows
//!     "drag Markitel_Bridge onto any chart" as the final step.
//!
//! When the spike lands, replace the body of `write_profile` with the
//! validated .chr byte sequence.

use serde::Serialize;
use std::path::Path;

#[derive(Serialize, Clone, Debug)]
pub enum ProfileWriteOutcome {
    /// Profile directory exists and chart is pre-attached. User just
    /// needs to File → Profiles → Markitel.
    Written,
    /// Profile folder created but auto-attach is not implemented yet.
    /// UI should fall back to "drag EA onto chart" guidance.
    NeedsManualDrag,
    /// We won't touch this terminal — either the profiles dir is
    /// missing or there's already a user-authored `Markitel` profile
    /// we'd have to clobber.
    Skipped { reason: String },
}

pub fn write_profile(profiles_dir: &Path) -> Result<ProfileWriteOutcome, String> {
    if !profiles_dir.exists() {
        return Ok(ProfileWriteOutcome::Skipped {
            reason: format!("profiles dir missing: {}", profiles_dir.display()),
        });
    }

    let target = profiles_dir.join("Markitel");
    if target.exists() {
        // Don't clobber user's existing "Markitel" profile. Our helper's
        // prior run left this, or they named their own profile this. Either
        // way — leave it alone.
        return Ok(ProfileWriteOutcome::Skipped {
            reason: "profile 'Markitel' already exists".to_string(),
        });
    }

    std::fs::create_dir_all(&target)
        .map_err(|e| format!("mkdir profiles/Markitel failed: {e}"))?;

    // TODO(phase-0): write a validated chart01.chr + .tpl here.
    // Until then, we fall back to manual-drag UX.
    Ok(ProfileWriteOutcome::NeedsManualDrag)
}
