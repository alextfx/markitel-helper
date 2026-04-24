# Markitel Helper — Signing + Release Runbook

This is the end-to-end recipe for shipping a signed, notarized
Markitel Helper release. It has three blocks:

  1. **One-time setup** — enroll in Apple Developer Program, generate
     certs, add secrets to GitHub. Do this **once per machine/account**.
  2. **Every release** — tag, push, wait, update Vercel env vars.
  3. **Local dev** — what still works without any of the above.

---

## 1 · One-time setup

### 1a. Apple Developer Program (macOS signing + notarization)

Cost: $99/year. Required for macOS code signing and notarization —
without it Gatekeeper blocks your .dmg on every end-user machine.

  1. Log in at <https://developer.apple.com/programs/> with your
     Apple ID (`tatarnikov2001@gmail.com`) and enroll as an individual.
     Individual enrollment clears in ~24 hours.

  2. Once enrolled, go to
     <https://developer.apple.com/account/resources/certificates/add>
     and create a **Developer ID Application** certificate.
     - Pick "In-house" / "Developer ID Application"
     - Generate a CSR using Keychain Access → Certificate Assistant →
       Request a Certificate from a Certificate Authority
     - Download the `.cer` file Apple produces

  3. Double-click the `.cer` — it installs into your login Keychain.
     Find it under the `My Certificates` category (NOT `Certificates`)
     — the private key needs to live alongside it.

  4. Right-click the certificate → **Export…** → save as `.p12`.
     Pick a strong password — you'll use this as the
     `APPLE_CERTIFICATE_PASSWORD` secret.

  5. Note the **signing identity string**. In Terminal:
     ```bash
     security find-identity -v -p codesigning
     ```
     Copy the line that matches `Developer ID Application: Your Name (TEAMID)`
     exactly. That's your `APPLE_SIGNING_IDENTITY`.

  6. Note your **Team ID**. It's the 10-character alphanumeric string
     in the parentheses above, also visible at
     <https://developer.apple.com/account> → Membership.

  7. Generate an **app-specific password** for notarization:
     <https://appleid.apple.com/account/manage> → Sign-In and Security
     → App-Specific Passwords → Generate Password. Label it
     `markitel-helper-notary`. **This is NOT your primary Apple ID
     password — the notary service specifically requires the
     app-specific form and your main password will be rejected.**

### 1b. Windows OV code-signing cert

Cost: ~$200-300/year. Without it Windows SmartScreen shows a red
"Microsoft Defender blocked this app" banner that roughly 1 in 3 users
will click away from.

  1. Purchase an OV ("Organization Validated") code-signing cert.
     Cheapest reputable vendors as of 2026:
     - Sectigo ($179/yr)
     - SSL.com ($249/yr)

  2. The vendor emails you a `.pfx` after a validation call
     (~1-3 business days). Password-protect it during generation —
     that password becomes `WINDOWS_CERTIFICATE_PASSWORD`.

  3. Base64-encode for GitHub:
     ```bash
     base64 -i markitel-windows.pfx | pbcopy
     ```
     Paste as the `WINDOWS_CERTIFICATE` secret.

### 1c. GitHub repository secrets

Go to `Settings → Secrets and variables → Actions → New repository
secret` and add all eight of these:

| Secret                         | Value                                                                 |
|--------------------------------|-----------------------------------------------------------------------|
| `APPLE_CERTIFICATE`            | `base64 -i cert.p12 \| pbcopy`                                        |
| `APPLE_CERTIFICATE_PASSWORD`   | The password you set in step 1a.4                                     |
| `APPLE_SIGNING_IDENTITY`       | Full string from step 1a.5 (e.g. `Developer ID Application: …`)       |
| `APPLE_ID`                     | `tatarnikov2001@gmail.com`                                            |
| `APPLE_PASSWORD`               | App-specific password from step 1a.7                                  |
| `APPLE_TEAM_ID`                | 10-char ID from step 1a.6                                             |
| `WINDOWS_CERTIFICATE`          | `base64 -i cert.pfx \| pbcopy`                                        |
| `WINDOWS_CERTIFICATE_PASSWORD` | The password from step 1b.2                                           |

Double-check `APPLE_PASSWORD` is the app-specific one. Using the
primary password silently succeeds on cert import but fails at
notarization with `Error: Invalid credentials` — a particularly
confusing symptom.

---

## 2 · Cutting a release

Everything from here is automated by
[.github/workflows/release-helper.yml](../.github/workflows/release-helper.yml).

  1. Bump the helper version in three places:
     - [`helper-app/package.json`](./package.json) `.version`
     - [`helper-app/src-tauri/Cargo.toml`](./src-tauri/Cargo.toml)
       `[package] version`
     - [`helper-app/src-tauri/tauri.conf.json`](./src-tauri/tauri.conf.json)
       `.version`

     Keep them in lockstep or `cargo build` and `tauri build` will
     disagree about what version they're producing.

  2. Commit the bump and tag it:
     ```bash
     git commit -am "chore(helper): bump to 0.1.0"
     git tag helper-v0.1.0
     git push origin main helper-v0.1.0
     ```

  3. The release workflow builds both platforms in parallel
     (~10-15 min), signs + notarizes, and opens a **draft** GitHub
     Release with the .dmg and .msi attached.

  4. Download both artifacts and sanity-test locally:
     ```bash
     # macOS: should open without the "unidentified developer" warning
     spctl -a -v "/path/to/Markitel Helper.app"
     # Expected: "Markitel Helper.app: accepted / source=Notarized Developer ID"
     ```

  5. Click **Publish release** on GitHub.

  6. Update Vercel env vars so the web app's fallback download links
     resolve:
     ```
     HELPER_LATEST_VERSION   = 0.1.0
     HELPER_DOWNLOAD_MAC_URL = https://github.com/alextfx/Markitel/releases/download/helper-v0.1.0/Markitel_Helper_0.1.0_aarch64.dmg
     HELPER_DOWNLOAD_WIN_URL = https://github.com/alextfx/Markitel/releases/download/helper-v0.1.0/Markitel-Helper_0.1.0_x64-setup.exe
     HELPER_RELEASE_NOTES    = Short changelog for the helper's Update UI (optional)
     ```
     Vercel auto-redeploys on env-var change. The
     `/api/v1/bridge/helper-version` endpoint picks up the new values
     immediately.

### Minimum supported version gate

If you need to force-upgrade users off a broken build, set
`HELPER_MIN_SUPPORTED_VERSION` in Vercel to the lowest version you
want to keep supporting. The helper's `fetch_helper_version` endpoint
includes it in the response; older builds will show an "Update
required" modal and refuse to run further.

---

## 3 · Local development

Nothing in this doc blocks local development. You can still:

- `npm run tauri dev` — runs the unsigned debug binary. macOS launches
  it fine because Gatekeeper skips same-machine `com.apple.quarantine`
  xattr checks for your own builds.

- `npm run tauri build -- --bundles app` — produces an unsigned .app.
  Opens on the build machine; other machines show the
  "unidentified developer" warning and need the user to right-click
  → Open.

The `signingIdentity: "-"` entry in `tauri.conf.json` means "sign with
no real identity, just produce a bundle". In CI, tauri-action
overrides it to the real identity via the `APPLE_SIGNING_IDENTITY`
env var.
