# AGENTS.md

Guidance for AI coding agents working in this repository.

## Project

Termimus is a self-hosted SSH/server manager desktop app built with **Tauri v2 + Rust backend + React frontend**, plus a small self-hosted **Go sync relay** in `server/`. See `README.md` for the full feature list and user-facing docs.

This repository (`CKL-Cargo/termimus-ssh`) is **CKL's fork** of the open-source Termimus project, deployed internally under CKL. The **CKL Data Egress Policy** and **Licensing & Branding** sections below are hard requirements and take precedence over upstream conventions.

## Stack & Layout

- **Frontend** (`src/`): React 19 + TypeScript + Vite, Tailwind CSS v4, Zustand, `@xterm/xterm` 6 (fit/search/webgl addons), `i18next`.
  - `src/components/<domain>/`: UI grouped by feature (`hosts`, `keychain`, `workspaces`, `terminal`, `sftp`, `tunnels`, `snippets`, `vault`, `settings`, `sync`, `layout`, `ui`).
  - `src/stores/`: one Zustand store per domain (`useHostStore`, `useSessionStore`, `useSftpStore`, `useTunnelStore`, `useSnippetStore`, `useVaultStore`, `useKeychainStore`, `useWorkspaceStore`, `useSyncStore`, `useAiStore`, `useSettingsStore`, `useTerminalThemeStore`, `useRightPanelStore`, `useKnownHostsStore`, `usePingStore`, `useConfirmStore`, `useUpdateStore`). `useUpdateStore` only holds `currentVersion`; there is no network update check (see Egress Policy).
  - `src/lib/api.ts`: the only place that calls `invoke(...)`. Every Tauri command has a typed wrapper here. Add new commands here, not as ad-hoc `invoke` calls in components.
  - `src/lib/layoutTree.ts`: split-pane model, a binary tree of `PaneSplit` / `PaneLeaf` nodes. `workspaceLayout.ts` + `workspaceLauncher.ts` build and launch multi-server workspace presets from it.
  - `src/lib/aiContext.ts`: builds AI Copilot requests (prompt, sanitized buffer, host/env context). Other helpers: `format.ts` (bytes, dates, paths), `clipboard.ts`, `terminalThemes.ts`, `terminalHighlighter.ts`/`highlightPatterns.ts`, `inputTracker.ts`, `openUrl.ts`.
  - `src/hooks/`: `useAutoLock` (vault idle lock), `useLiveSync` (sync relay WebSocket).
  - `src/i18n/`: locales `en`, `id`, `zh`. `id`/`zh` files are typed as `typeof <en module>`, so **adding or removing a key must be done in all three locales** or `tsc` fails.
- **Backend** (`src-tauri/src/`):
  - `ssh/`: SSH session manager (`russh`), PTY streaming, host key verification (`SshClientHandler`), keyboard-interactive 2FA prompts.
  - `sftp/`: SFTP manager (`russh-sftp`), local filesystem helpers, remote↔remote transfer.
  - `tunnel/`: local (`-L`), remote (`-R`) and dynamic SOCKS5 (`-D`, `socks5.rs`) forwarding.
  - `pty/`: local shells via `portable-pty` (zsh/bash, PowerShell/cmd via ConPTY).
  - `vault/`: AES-256-GCM encryption + Argon2id key derivation. The derived key lives only in memory (`VaultManager`) or, if the user opts in, in the OS keyring. It is never written to disk in plaintext.
  - `biometric/`: macOS Touch ID unlock (`macos.rs`); `unsupported.rs` stub elsewhere.
  - `sshkey/`: key generation and public-key/fingerprint derivation.
  - `ai/`: AI Copilot. `client.rs` (OpenAI-compatible / Anthropic / Ollama HTTP + SSE), `prompt.rs`, `command.rs` (response parsing + risk classification), `environment.rs` (remote OS/shell probe), `sanitizer.rs` (redacts secrets from terminal buffers).
  - `netpolicy/`: **CKL outbound network policy**: private-network-only AI endpoints, TLS-only sync. See below.
  - `db/`: SQLite via `rusqlite`. `models.rs` has all row structs; `mod.rs` has the `Database` struct with one method per query. Schema lives in `init_tables()` (tables: `vault_meta`, `folders`, `credentials`, `hosts`, `port_forwards`, `snippets`, `workspace_presets`, `known_hosts`, `tombstones`, `ai_providers`).
  - `commands/mod.rs`: every `#[tauri::command]` handler (~90), backup export/import, sync push/pull, plus `AppState` (holds `Arc`s of `Database`, `VaultManager`, `SessionManager`, `SftpManager`, `TunnelManager`, `PtyManager`, `AiClient`). Register every new command in `src-tauri/src/lib.rs`'s `generate_handler!` list too.
- **Sync relay** (`server/`, Go): `cmd/server/main.go` routes `/health` and bearer-token-protected `/api/v1/sync/{status,bundle,devices,ws}`. It stores only end-to-end-encrypted bundles in SQLite (`modernc.org/sqlite`) and pushes `SYNC_UPDATED` over WebSocket (`internal/hub`). Published as `ghcr.io/ckl-cargo/termimus-sync` by `.github/workflows/docker.yml`.

## CKL Data Egress Policy (mandatory)

CKL data (hostnames, IPs, usernames, credentials, terminal output, file contents) must never leave the company network or reach a third party. The app is allowed to make exactly these outbound connections:

| Destination | Who configures it | Enforcement |
|---|---|---|
| SSH hosts (terminal, SFTP, tunnels, ping) | User | `SshClientHandler` TOFU host-key verification |
| Sync relay | User / CKL IT | `netpolicy::require_secure_sync_url`: `https://` only, `http://` for loopback only; redirects not followed; bundle E2EE-encrypted with the sync passphrase before upload |
| AI endpoint | User / CKL IT | `netpolicy::ensure_private_endpoint` + `PrivateOnlyResolver`: must resolve to loopback, RFC 1918 or `fc00::/7`; proxies and redirects disabled on the AI client |

Rules for any change:

- **No new outbound destinations** without explicit approval from the user/CKL. That covers telemetry, analytics, crash reporting, update checks, CDNs, remote fonts/images, link previews, and "phone home" calls of any kind. The upstream GitHub update check was removed on purpose; don't reintroduce it.
- **All HTTP from the app goes through Rust (`reqwest`)**, not the webview's `fetch()`. Any new `reqwest::Client` that can carry CKL data must apply the matching `netpolicy` check. Never build one with `unwrap_or_else(|_| Client::new())` fallbacks that silently drop the policy.
- **AI providers must stay self-hosted.** Don't add public cloud presets (OpenAI, Anthropic, DeepSeek, Groq, Gemini, Zhipu, …) to `AiProviderModal.tsx`, and don't weaken `netpolicy::is_private_ip`. Terminal buffers must pass through `ai::sanitize_terminal_buffer` before being sent anywhere. Redaction is best-effort, so it is not a substitute for the private-network rule.
- **CSP** (`src-tauri/tauri.conf.json`): `connect-src` is `ipc: http://ipc.localhost wss: ws://localhost:* ws://127.0.0.1:*`. Don't add hosts or broaden `ws:`. Fonts and assets are bundled locally (`@fontsource/*`, `src/assets/fonts`); never load from Google Fonts or a CDN.
- **External links** (`openExternalUrl`) only on explicit user click, and only to CKL-controlled or neutral documentation URLs. No donation/sponsor links.
- **Logging**: never log secrets, decrypted credentials, API keys, sync passphrases/tokens, or terminal content (`eprintln!`/`console.*` included).

## Licensing & Branding

- Upstream Termimus is **MIT-licensed**: CKL may use, modify and deploy it commercially/internally. The one obligation is to **keep the original copyright and permission notice** (`LICENSE`, "Copyright (c) 2026 Fadilah Riczky") in all copies, including distributed installers. Never remove or alter that line. CKL's own line sits below it.
- MIT grants no trademark rights. The "Termimus" name and logo are kept as-is; don't present the app as an official upstream release, and don't add upstream donation/support links. Repo links point to `github.com/CKL-Cargo/termimus-ssh`, images to `ghcr.io/ckl-cargo/termimus-sync`. Upstream identifiers that are intentionally unchanged: bundle id `com.termimus.app`, Go module path `github.com/termimus/termimus-server`.
- **New dependencies must use permissive licenses** (MIT, Apache-2.0, BSD-2/3-Clause, ISC, Zlib, Unicode, CC0, MPL-2.0 as an unmodified library, OFL-1.1 for fonts). **Do not add GPL/AGPL/LGPL-linked, SSPL, BUSL or "non-commercial" dependencies** without explicit approval. Flag the license in the PR description when adding any dependency.
- Don't copy code, assets or text from other products (e.g. Termius) into the repo.

## Conventions

- **Every SSH-family connection (terminal, SFTP, tunnel) must go through `SshClientHandler`** with `db: Arc<Database>` passed in, so Trust-On-First-Use host key verification and MITM protection apply uniformly. Don't construct a bare `russh::client::connect` without it.
- **Credentials are never stored or transmitted as plaintext.** `Credential.ciphertext`/`nonce` are AES-256-GCM output; encrypt/decrypt only through `VaultManager`. The vault must be unlocked (`state.vault.is_unlocked()`) before touching secrets. AI API keys are vault credentials (`kind = "ai_api_key"`); sync secrets live in the OS keyring, not `localStorage`.
- **New Tauri commands**: add the `#[tauri::command]` fn in `commands/mod.rs`, add it to `generate_handler![...]` in `lib.rs`, then add a typed wrapper in `src/lib/api.ts`. All three steps are required or the command silently won't be callable from the frontend.
- **Tauri v2 Capabilities & Permissions**: plugins and window APIs require explicit entries in `src-tauri/capabilities/default.json`. Beware of plugins whose default capability is empty (e.g. `tauri-plugin-clipboard-manager` has `clipboard-manager:default` with `permissions = []` for security); always grant explicit permissions such as `"clipboard-manager:allow-read-text"` and `"clipboard-manager:allow-write-text"`. Grant the narrowest permission that works.
- **Clipboard & Terminal I/O**: WebKitGTK / Tauri on Linux blocks `navigator.clipboard.readText()` when pasting from external applications without explicit per-origin browser permission. Always use `@tauri-apps/plugin-clipboard-manager` (`readText`/`writeText`) as the primary path and fall back to `navigator.clipboard`. In `XtermView.tsx`, use debounce guards (~350ms window with content tracking) to prevent double-paste across keyboard shortcuts and native DOM paste events.
- **Terminal Lifecycle & Pooling**: instances are cached in `terminalPool` (`XtermView.tsx`) to survive tab switches, splits, and layout changes without reconnecting SSH. Initialize `isConnected` synchronously from the pool entry (never in a delayed effect) to prevent flashing the `ConnectionProgress` overlay during split remounts.
- **Zustand Performance**: always use `useShallow` from `zustand/react/shallow` when subscribing to non-primitive selectors (arrays, objects) in stores (`useSessionStore`, `usePingStore`, etc.) to prevent cascading re-renders across the component tree.
- **SSH PTY Streaming**: backend coalesces PTY chunks in `ssh/mod.rs` (~8ms batches / 64KB cap) before emitting `ssh-data-${sessionId}` events to minimize IPC overhead.
- **New DB tables**: add the `CREATE TABLE IF NOT EXISTS` to `init_tables()` in `db/mod.rs`, add the row struct to `models.rs`, add CRUD methods to `Database`. If the table should be portable, also wire it into `BackupBundle`/`export_backup_bundle`/`import_backup_bundle` (and handle deletions via `tombstones` so sync merges correctly).
- **Terminal visibility**: never hide the terminal container with `display: none`. Use `visibility: hidden` + `pointerEvents: none` (see `App.tsx`, `XtermView.tsx`). `display: none` collapses the container to 0×0, which makes `ResizeObserver`/`FitAddon` compute a bogus tiny size and corrupts remote curses apps (htop, vim) via a bad SIGWINCH. Any resize call must guard against near-zero `cols`/`rows` (frontend guards in `XtermView.tsx`, backend guard in `ssh/mod.rs`'s resize handler).
- **Styling**: Tailwind v4 utility classes with CSS variables defined in `src/index.css` (`--canvas`, `--surface-*`, `--primary`, `--text-*`, etc. — the "Terminal Obsidian" palette). Reuse these variables; don't hardcode hex colors in components.
- **Fonts**: Inter for UI text, JetBrains Mono/Fira Code for anything monospace (IPs, ports, commands, code). Bundled via `@fontsource/*` imports in `src/index.css` (Nerd Font symbols in `src/assets/fonts`). No remote font loading.
- **User-facing strings** go through `t(...)` with keys in all three locales.
- Window is frameless (`decorations: false`); the custom titlebar/window controls live in `src/components/layout/Header.tsx`. Don't reintroduce native window chrome.
- **Version Bump Checklist**: when bumping the app version, update all 6 files simultaneously:
  1. `package.json` (`version`)
  2. `src-tauri/tauri.conf.json` (`version`)
  3. `src-tauri/Cargo.toml` (`version`)
  4. `src/stores/useUpdateStore.ts` (`currentVersion`)
  5. `README.md` (header subtitle `·  vX.Y.Z`, badge URL, and the Downloads table rows)
  6. `server/internal/handlers/handlers.go` (`version` field in the `/health` response)
  Then run `cargo check` in `src-tauri` so `src-tauri/Cargo.lock` reflects the new package version.

## Verification

- Frontend: `bun run build` (runs `tsc` + `vite build`). Must pass with zero TypeScript errors before a frontend change counts as done.
- Backend: `cd src-tauri && cargo check --all-targets && cargo test --all-targets && cargo clippy --all-targets`. Must compile clean; treat new warnings as things to fix, not ignore. CI (`.github/workflows/ci.yml`) runs all of these plus the frontend build on every PR.
- Unit tests live in `#[cfg(test)]` modules next to the code (`ai/*`, `netpolicy`, `pty`, `ssh`, `sshkey`, `tunnel/socks5`, `db/models`). Add tests for non-trivial backend logic (crypto, parsing, DB migrations, network policy). Any change to `netpolicy` must keep or extend its tests.
- Sync server: `cd server && go build ./... && go vet ./...`.
- Manual verification via `bun run tauri dev` is the norm for UI changes.
- Full production build: `bun run tauri build --bundles deb` (Linux). Slow (multi-minute Rust compile). Don't run it for routine changes, only when explicitly asked to produce a release artifact.

## Git

- Commit only when asked. Stage specific files, not `git add .`, when unrelated changes are present in the working tree (this repo occasionally has loose reference/design files at the root; don't sweep those into unrelated commits).
- Never commit secrets: `.env` files, sync tokens, API keys, private keys, exported backups, or `*.db` files.
- Attribution lines for commits/PRs are specified by the harness system reminder, not this file.
