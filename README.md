# Cobble Launcher

**一个不折腾人的 Minecraft 启动器。**
A straightforward, local-first launcher for Minecraft: Java Edition.

Cobble Launcher is an independently developed, open-source desktop project built with Rust, Tauri 2 and Vue 3. Its goal is a clean interface for launching Minecraft and managing separate game installations, mods and modpacks, without requiring a launcher-specific cloud account.

## Development status

**Early development — resource installation is available through the CLI; game launching is not implemented yet.** No stable release is available.

Implemented:

- A frameless desktop interface with custom window controls, light/dark themes and accessible settings controls.
- Typed Core interfaces and SQLite initialization.
- Restricted ZIP32 archive extraction with path, collision and resource-limit checks.
- A content-addressed artifact store with digest verification and independent writable copies.
- An HTTPS network broker with destination validation, scoped credentials, bounded responses and cancellation.
- Resumable artifact downloads with durable checkpoints and whole-file digest verification before storage.
- Shared in-process downloads with independent cancellation and bounded retries.
- A live Minecraft version catalog available through the CLI.
- Modern Vanilla resource installation, verified cache reuse and installed Java detection through the CLI.
- Official, BMCLAPI and custom BMCLAPI-compatible download sources for game resources.

The current implementation has been tested on Windows. macOS and Linux are target platforms; they have not yet been validated on physical devices.

Next: desktop installation controls, automatic Java installation, Microsoft sign-in and actual game launching. Fabric, Forge/NeoForge and modpack workflows are planned, not currently supported features.

## Microsoft / Minecraft integration

The intended sign-in flow uses this project's own registered Microsoft application and user consent. Planned integration covers Microsoft authentication, Xbox services and Minecraft Services to verify account ownership and obtain the profile required for legitimate Minecraft: Java Edition launches.

Authentication and ownership verification are **not implemented yet**. Availability depends on the project's own application registration and any required platform approval. The project does not use another launcher's client ID, collect Microsoft account passwords, or provide an authentication bypass. Players will need a valid account and the required game entitlement. No game binaries, account credentials or production signing keys are included.

This is an independent community project, not an official Minecraft product and not affiliated with or endorsed by Mojang or Microsoft. Minecraft and related marks belong to their respective owners.

## Build from source

Requirements:

- Rust **1.99.0** (pinned in `rust-toolchain.toml`).
- Node.js **24.19.0** and pnpm **11.25.0**.
- The native [Tauri development prerequisites](https://v2.tauri.app/start/prerequisites/) for your platform, including Windows C++ build tools and WebView2 on Windows.

```sh
git clone https://github.com/CobbleLauncher/CobbleLauncher.git
cd CobbleLauncher
pnpm install --frozen-lockfile
pnpm desktop
```

Build the unsigned development executable:

```sh
pnpm desktop:build
```

This builds the current desktop shell; it does not enable unfinished game features.

List available Minecraft versions:

```sh
cargo run -p cobble-cli --locked -- versions
cargo run -p cobble-cli --locked -- versions --all
```

The default lists the latest 20 releases. `--all` includes snapshots and historical versions; listing a version does not imply that installation or launch is supported yet.

Inspect and download modern Vanilla game resources into an empty folder:

```sh
cargo run -p cobble-cli --locked -- info 1.21.1
cargo run -p cobble-cli --locked -- install 1.21.1 --dir ./game
cargo run -p cobble-cli --locked -- install 1.21.1 --dir ./game --source bmclapi
cargo run -p cobble-cli --locked -- java 21
```

Select a source in the desktop settings; the CLI uses the same saved preference. The default is official. An explicit `--source` or `--mirror` overrides it for one command. `--mirror <https-base>` selects a custom BMCLAPI-compatible mirror, including a privately configured overseas endpoint. It cannot be combined with `--source`. Source selection applies to the version catalog, version metadata and game resources during installation. Files are checked against the hashes in the selected catalog and version metadata. Choose a mirror you trust. Repeating the command reuses verified files and cached content. Existing unrelated game folders are not overwritten. Java installation, sign-in and launching are separate unfinished features.

[BMCLAPI](https://bmclapidoc.bangbang93.com/) is provided by bangbang93 and its contributors. Mirrored files retain their upstream copyright. No particular overseas mirror is bundled or endorsed.

A redirected mirror request currently restarts an interrupted file from the beginning; already verified files remain reusable.

## Tests

```sh
pnpm lint
pnpm typecheck
pnpm test
pnpm build
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
```

Build the frontend before running the Rust desktop checks, because Tauri embeds the frontend assets.

Browser tests additionally require Playwright's Chromium installation:

```sh
pnpm --filter @cobble/desktop exec playwright install chromium
pnpm test:e2e
```

The network tests use a publicly known, synthetic TLS identity trusted only in test builds. It is not a production certificate or signing credential.

## Source layout

- `apps/desktop` — Vue interface and Tauri desktop host.
- `apps/cli` — command-line entry point.
- `crates` — independent Rust libraries; several planned subsystems are still placeholders.
- `contracts` — schemas, generated bindings and database migration used by the code.
- `fixtures/security` — original hostile archive samples used by tests.
- `scripts` — binding generation and dependency-license checks.

## License and contact

Project code is licensed under [MIT](LICENSE). Dependencies retain their own licenses; see [third-party notices](THIRD_PARTY_NOTICES.md).

For project questions and bug reports, use [GitHub Issues](https://github.com/CobbleLauncher/CobbleLauncher/issues).
