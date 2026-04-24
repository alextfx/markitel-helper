# Next-Session Handoff: Markitel Helper (Phase 2 continuation)

Paste the prompt in the "Prompt" section below into a fresh Claude Code
session to continue this work. The rest of this file is orientation for
you — the user — so you know what's built and what's not.

---

## What's done (Phase 1 + Phase 2 scaffold)

### Phase 1 — Web backend (merged, verified)

- Migration `supabase/migrations/20260501000000_bridge_pairing_codes.sql`
  adds `bridge_pairing_codes` and `bridge_install_events` tables.
- `lib/services/bridge/pairing.ts` issues + consumes codes atomically.
- `lib/services/bridge/connections.ts` gained `rotateApiKey`.
- Six new endpoints under `/api/v1/bridge/`:
  `pair/start`, `pair/exchange`, `rotate`, `install-telemetry`,
  `ea-source`, `helper-version`.
- `tests/bridge-pairing.test.ts` — 17 passing tests on the code lifecycle.
- Typecheck, lint, full test suite (276 tests) all green.

### Phase 2 scaffold — Tauri helper app (NOT YET BUILT OR RUN)

Complete `helper-app/` directory scaffold written, but:

- **Rust has not been installed on the dev machine** (no `~/.cargo`, no
  `~/.rustup`). Nothing has been compiled yet.
- `npm install` has not been run in `helper-app/`.
- Tauri icon assets under `src-tauri/icons/` do not yet exist — need
  `npx @tauri-apps/cli icon path/to/logo.png` on a 1024×1024 Markitel
  logo.
- `profile_writer.rs` is a **stub** — real `.chr` format needs Phase 0
  spike results (see `ea/SPIKES.md`, to be written).

## Known risk areas in the scaffold

These are places where the code is written from canonical Tauri 2.x
patterns but hasn't been compile-checked. Expect minor API drift:

1. **`src-tauri/src/lib.rs`** — tray icon + deep-link setup. Tauri 2.x
   has had small API moves between 2.0 and 2.1 around
   `TrayIconBuilder::show_menu_on_left_click` vs `menu_on_left_click`,
   and `DeepLinkExt::on_open_url` vs `on_open_url_event`. Fix on first
   compile error.
2. **`src-tauri/src/pairing.rs`** — the `DeepLinkExt` re-export may need
   to be `use tauri_plugin_deep_link::DeepLinkExt;` imported directly in
   `lib.rs` where it's used on `app.deep_link()`.
3. **`url` crate** — pairing.rs uses `url::Url` but I didn't add it to
   Cargo.toml explicitly because Tauri re-exports it. If the compiler
   complains, add `url = "2"` as a direct dep.
4. **`keyring` crate v3** — changed `delete_password()` to
   `delete_credential()`. I used the v3 name. If we end up on v2 for
   some reason, swap back.
5. **`tauri.conf.json`** — the `"bundle"` block is minimal; add
   `"resources"` if you need to ship any sidecar files.
6. **macOS `deep_link().register()`** — only works at runtime for
   debug/unsigned builds. Production needs the URL scheme baked into
   `Info.plist` via Tauri's bundle config (works via `deep-link`
   plugin's `schemes` field — already set).

## First-run checklist

```bash
# 1. Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
rustc --version   # should be >= 1.77

# 2. Install JS deps
cd /Users/alextatarnikov/Desktop/Markitel/helper-app
npm install

# 3. Generate icons (use any 1024x1024 Markitel logo PNG)
npx @tauri-apps/cli icon ../public/logo.png  # or any logo path

# 4. Dev-run against local Next.js backend
# (in a separate terminal, run `npm run dev` at the repo root first)
MARKITEL_API_BASE=http://localhost:3000 npm run tauri dev
```

First `cargo build` will pull ~400 crates and take ~5–10 min. Subsequent
builds are seconds.

## Next engineering steps, in order

1. Fix any compile errors from the first `cargo build` run. Most likely
   suspects listed in "Known risk areas" above.
2. Test the unpaired → paired flow end-to-end:
   - Start the web app (`npm run dev` in repo root).
   - Start the helper (`npm run tauri dev` in `helper-app/`).
   - In the web app's `/broker` page, trigger a `/pair/start` call
     (easiest: open DevTools and `fetch('/api/v1/bridge/pair/start',
     {method:'POST', headers:{Authorization:'Bearer <jwt>'}})`).
     Copy the returned `deepLink`.
   - In a terminal: `open "markitel://pair?code=XXXXXX"`.
   - Confirm: helper UI updates to "Paired", keychain has entry,
     backend logs show a `/pair/exchange` 200 and a `broker_connections`
     row with a fresh key.
3. Test install on a real MT5 install: put the helper through its
   discover → install path on a machine that actually has MT5. Expect
   `profile_writer` to return `NeedsManualDrag`; document the user flow.
4. Phase 0 spike work — write `ea/SPIKES.md`:
   - Across MetaQuotes Mac, MetaQuotes Win, MTrading, AMarkets builds:
     - Craft a `profiles/Markitel/chart01.chr` that auto-attaches
       `Markitel_Bridge` to EURUSD when the user switches to the
       profile. Capture byte-layout differences if any.
     - Confirm `[Experts] WebRequestURL_N=https://markitel.com` in
       `config/terminal.ini` survives an MT5 launch + quit cycle.
       Fall back to `config/common.ini` if it doesn't.
5. Replace the `profile_writer.rs` TODO with validated .chr generation.
6. Phase 4 — web wizard refactor:
   - Modify `components/broker/setup-wizard.tsx` to add a
     "Use Markitel Helper (recommended)" path that calls
     `/api/v1/bridge/pair/start`, shows the 6-char code + "Open
     Markitel Helper" button (`window.location.href = deepLink`),
     polls `/connection` for first heartbeat, advances on success.
   - Keep existing `.command` / `.bat` download path behind a
     "Use manual installer" link as fallback.
7. Phase 3 — signing:
   - Enroll in Apple Developer Program ($100/yr). Generate Developer
     ID Application + Installer certs, import into Keychain Access.
   - Set up notarization: `xcrun notarytool store-credentials` with
     app-specific password from appleid.apple.com.
   - Purchase Windows OV code-signing cert (~$200–300/yr, Sectigo or
     DigiCert are cheapest).
   - Add `.github/workflows/release-helper.yml` that builds, signs,
     notarizes on tag push.
   - Populate `HELPER_DOWNLOAD_MAC_URL` / `HELPER_DOWNLOAD_WIN_URL`
     env vars.

---

## Prompt (paste into next session)

```
I'm continuing the Markitel MT5 native-installer project. Phase 1 (web
backend for pairing, rotation, telemetry, EA-source, helper-version) is
merged and tested — 17 pairing-lifecycle tests passing, full suite green.

Phase 2 scaffold is in place under helper-app/ but has NEVER BEEN
COMPILED. I need you to:

1. Read helper-app/README.md and helper-app/NEXT_SESSION_PROMPT.md for
   the full context and known risk areas.
2. Install Rust via rustup (no user interaction — run the standard
   curl | sh install and source ~/.cargo/env).
3. Run `npm install` in helper-app/, then `npm run tauri dev` with
   MARKITEL_API_BASE=http://localhost:3000.
4. Fix any compile errors — the scaffold was written from canonical
   Tauri 2.x patterns without being compile-verified. Most-likely
   culprits are documented in NEXT_SESSION_PROMPT.md under "Known risk
   areas in the scaffold."
5. Generate Tauri icons if missing. Use any 1024×1024 logo png in the
   repo (check `public/` or `app/` for candidates). If nothing suitable
   exists, generate a placeholder with ImageMagick or skip icon gen and
   use Tauri defaults.
6. Once the helper builds and launches:
   a. Verify the "unpaired" UI renders.
   b. Test deep-link pairing end-to-end against the local Next.js app.
      See the "First-run checklist" + step 2 in NEXT_SESSION_PROMPT.md
      for the exact test recipe.
7. Report what worked, what didn't, and what's next.

Constraints:
- DO NOT start Phase 3 (code signing) — that needs purchased certs.
- DO NOT start Phase 4 (web wizard refactor) until the helper is proven
  to build and pair successfully. Building Phase 4 UI that points at a
  non-working deep link would be worse than useless.
- profile_writer.rs stays a stub until Phase 0 spikes land. Don't
  invent a .chr format.
- Use the Agent tool for exploration if you need to understand parts of
  the existing codebase — the Phase 1 endpoints, the EA source format,
  etc.

Start by reading helper-app/README.md and helper-app/NEXT_SESSION_PROMPT.md
in full. Then proceed.
```
