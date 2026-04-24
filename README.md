# Markitel Helper

Native menu-bar / tray app that installs the Markitel MT5 bridge for the
user and displays connection status. Pairs with the website via a
`markitel://pair?code=XXXXXX` deep link — the web app issues a short-lived
pairing code and the helper exchanges it for the user's API key.

The helper replaces today's unsigned `.command` / `.bat` download flow.
See `../plans/i-would-like-to-hidden-wolf.md` (or its final location in
`~/.claude/plans/`) for the full plan context.

## Scope

1. Install MT5 bridge Expert Advisor with API key embedded
2. Whitelist `markitel.com` in MT5's `terminal.ini` / `common.ini`
3. Create an isolated `Markitel` MT5 profile so the EA auto-attaches on
   user's next profile switch — NEVER touches the default profile
4. Live connection status in the tray / menu bar
5. Key rotation + reinstall commands for compromise recovery
6. Best-effort install-funnel telemetry to
   `POST /api/v1/bridge/install-telemetry`

## Build status

This directory is a **scaffold**. Everything compiles in spirit but:

- Rust toolchain has not been run against it yet (scaffolded without
  `cargo build`). Expect small fix-ups on first build (mostly import
  adjustments for Tauri 2.x crate versions).
- `profile_writer.rs` is a **STUB** — the exact `chart01.chr` format
  needs the Phase 0 spike findings (see `ea/SPIKES.md` in repo root,
  to be written). Until then, profile creation logs "unimplemented" and
  the tray shows "drag EA onto chart manually" as fallback.
- Tauri icon assets under `src-tauri/icons/` must be generated per
  Tauri docs. Run `npx @tauri-apps/cli icon path/to/source.png` on a
  1024×1024 Markitel logo.
- Code signing is Phase 3 — unsigned dev builds only for now.

## Development

### Prerequisites

- Rust (install via [rustup.rs](https://rustup.rs))
- Node 18+ (the repo root's `.nvmrc` covers this)
- macOS: Xcode Command Line Tools (`xcode-select --install`)
- Windows: WebView2 (preinstalled on Win10 21H1+)
- Linux: `libwebkit2gtk-4.1-dev`, `libssl-dev`, `libayatana-appindicator3-dev`

### First build

```bash
cd helper-app
npm install
npm run tauri dev      # launches UI in a window; hot-reloads frontend
```

For a production build (unsigned, local only):

```bash
npm run tauri build
# → src-tauri/target/release/bundle/{dmg,msi,appimage,deb}/
```

### Environment

The helper reads its backend URL from `MARKITEL_API_BASE`, defaulting to
`https://markitel.com`. For local dev against the Next.js app:

```bash
MARKITEL_API_BASE=http://localhost:3000 npm run tauri dev
```

### Deep-link testing (macOS)

After any `tauri dev` / `tauri build` the helper registers `markitel://`
in the OS's protocol handler database. Test by pasting in Safari or:

```bash
open "markitel://pair?code=ABC123"
```

### Deep-link testing (Windows)

Tauri's `deep-link` plugin writes HKCR\markitel keys on build. For dev,
run the helper once in release mode to register.

## Layout

```
helper-app/
├── README.md              ← you are here
├── package.json           ← JS deps (Vite, React 18, @tauri-apps/api)
├── tsconfig.json
├── vite.config.ts
├── index.html
├── src/                   ← TS/React frontend (tray panel)
│   ├── main.tsx
│   ├── App.tsx
│   ├── styles.css
│   └── lib/
│       ├── api.ts         ← wraps tauri `invoke()` calls
│       └── types.ts
└── src-tauri/             ← Rust backend
    ├── Cargo.toml
    ├── tauri.conf.json
    ├── build.rs
    ├── icons/             ← populate with `tauri icon` CLI
    └── src/
        ├── main.rs        ← entry point
        ├── lib.rs         ← app setup + plugin wiring
        ├── commands.rs    ← #[tauri::command] handlers (invoked by UI)
        ├── config.rs      ← constants + env resolution
        ├── keychain.rs    ← OS credential store wrapper
        ├── api.rs         ← HTTP client for Markitel backend
        ├── telemetry.rs   ← install-funnel event shipping
        ├── pairing.rs     ← deep-link handler + code exchange
        ├── mt5_discovery.rs   ← finds MT5 installs on this machine
        ├── mt5_launcher.rs    ← starts MT5 for the user
        ├── ea_writer.rs       ← writes keyed Markitel_Bridge.mq5
        ├── ini_writer.rs      ← edits terminal.ini / common.ini
        └── profile_writer.rs  ← STUB (Phase 0)
```

## Backend contract

The helper talks to the following Markitel endpoints (all under
`/api/v1/bridge/` — already shipped as of the Phase 1 plan):

| Endpoint              | Auth            | Purpose                    |
|-----------------------|-----------------|----------------------------|
| `POST /pair/exchange` | code (in body)  | exchange code → apiKey     |
| `POST /install-telemetry` | optional X-Bridge-Key | funnel events        |
| `POST /rotate`        | X-Bridge-Key    | rotate current api key     |
| `GET /ea-source`      | none            | raw unkeyed EA source      |
| `GET /helper-version` | none            | update check               |

## Security notes

- API key stored in OS keychain (macOS Keychain, Windows Credential
  Manager) via the `keyring` crate. Never written to disk in plaintext.
- `terminal.ini` / `common.ini` edits back up the original first
  (`.markitel.bak`). Helper refuses to edit while MT5 is running.
- Deep-link URL scheme `markitel://` — future-proofing: if this ever
  collides we can switch to `markitel-helper://` by editing
  `src-tauri/tauri.conf.json`'s `plugins.deep-link.schemes`.

## Phase 0 dependency

The `profile_writer.rs` module needs the `chart01.chr` format understood
across the four target MT5 variants:

- MetaQuotes MT5 (Mac)
- MetaQuotes MT5 (Win)
- MTrading-branded MT5
- AMarkets-branded MT5

Capture findings in `ea/SPIKES.md` (repo root). Until then, `profile_writer`
logs a TODO and the UI surfaces "manual drag" instructions.
