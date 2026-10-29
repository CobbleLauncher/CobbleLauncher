# Cobble Launcher

**A Minecraft launcher that stays out of your way.**

Cobble is an open-source launcher for Minecraft: Java Edition, built with Rust, Tauri and Vue. Keep your games separate, find compatible content, and get back to playing without digging through folders.

**[Download Windows Beta](https://github.com/CobbleLauncher/CobbleLauncher/releases/download/v0.1.0-beta.2/CobbleLauncher-windows-x64-setup.exe)** · [Portable ZIP](https://github.com/CobbleLauncher/CobbleLauncher/releases/download/v0.1.0-beta.2/CobbleLauncher-windows-x64.zip) · [Website](https://cobble.yuzhes.com/) · [All releases](https://github.com/CobbleLauncher/CobbleLauncher/releases)

The Windows x64 beta is available as an installer or portable ZIP. The installer supports optional updates from the preview channel. For the portable version, extract the entire ZIP and open **Cobble Launcher.exe**; replace the application files manually when updating. Microsoft Edge WebView2 is required. Update packages have cryptographic signatures, but the Windows executables do not have an Authenticode certificate and Windows may show a publisher warning. The [website](https://cobble.yuzhes.com/) checks GitHub Releases for the latest package for each supported system, with a menu for other systems.

macOS previews are built in CI for Apple Silicon and Intel. They use ad-hoc signatures and are **not notarized or tested on a physical Mac**. Custom plugin frontends are disabled, and the separate Mesh companion is not included. Linux remains an internal test package.

## Play and manage your games

- **Your library, your way.** Continue your last game, pin instances, set custom icons and covers, and create Windows desktop shortcuts.
- **Bring your existing games.** Discover supported PCL games, reuse matching local files, and copy or move isolated game content through a guided import. Choose where games are stored during setup, or move them to another drive later.
- **Simple installation.** Install Vanilla, Fabric, Forge or NeoForge. Reuse matching Java runtimes, or automatically download one on Windows x64 when needed. Choose settings globally or per game. Downloads support automatic source selection, parallel transfers, cancellation and resume.
- **Content in one place.** Find Mods and modpacks on Modrinth and CurseForge, plus resource packs and shaders on Modrinth. Start with the Mods you want and review a compatible game setup before installing. Each provider keeps its own download rules.
- **Updates you can undo.** Pin Mod versions, ignore a particular update, choose stable or preview releases, and undo managed changes while preserving later local edits.
- **Accounts and appearance.** Switch between Microsoft accounts and offline profiles, assign an account to a game, and preview or apply skins.
- **Useful recovery.** Recover interrupted installations, check which files are missing for offline play, prepare Java and game files, and open the relevant logs when a launch fails.
- **While you play.** Keep logs, invitations, backup status and frequently used tools together in a game workspace. Game settings stay locked during an active session.

Use `Ctrl+K` to find games, tools and actions, including pinyin initials. Adjust text size, interface scale, spacing and colors in a live preview, then save the combination you prefer. Downloads and game copies continue in the task center while you browse elsewhere.

## Keep your worlds and creations

- Save named world snapshots with deduplicated storage, or enable automatic backups after a game exits. Restore earlier versions with a rollback option.
- Browse screenshots and edit supported Mod configuration files with backups. The optional Config Doctor compares current files with backups, restores selected JSON fields or a whole file, and copies compatible keybindings between games.
- Search, save and preview blueprints, inspect materials, and import files into a game's schematic folder. Optional providers can add online blueprint catalogs.
- Import existing launcher instances and local modpacks into separate games. Update a local MRPACK into an independent copy, review conflicting JSON fields and choose whether to copy worlds.

## Optional tools and multiplayer

Extensions add viewers, diagnostics and other tools. Browse installed extensions, review permissions, and subscribe to multiple static HTTPS catalogs, including catalogs hosted on GitHub. Plugins can use host-rendered interfaces or an isolated custom frontend, with access limited by their declared permissions.

Bundled examples include Schematic Viewer, Config Doctor, a Mod dependency map and a world snapshot timeline with file comparisons. World Chronicle visualizes local play statistics. Crash Doctor compares changes since a clean exit and can prepare an independent diagnostic copy with related Mods kept together. You decide when to run each experiment and record its result. It also redacts logs locally for review before sharing. Command Pocket generates a small set of Minecraft commands you can copy.

Anvil Planner finds a low-cost enchantment order for supported Java 1.20.1 and 1.21.1 equipment, including existing enchantments and prior work penalties. Enter up to eight single-enchantment books to see each combination and its level cost. It works offline; durability repairs, renaming and server-modified rules are outside its scope.

Update Trial Copy creates an independent game copy for trying changes, with worlds excluded by default. Play Configuration exports a reviewed `.mrpack` with verified Modrinth download references and explicitly selected configuration files. File Inbox identifies selected or dropped files and hands them to a compatible import or viewer after review. Storage Lens clears downloadable metadata cache and moves stopped games through a verified copy while retaining the original. Language Doctor checks selected language JSON or resource-pack translations for duplicate keys, missing entries and placeholder differences.

Seed Compass reads a selected world's seed with your permission, draws a biome map and finds nearby biome or structure locations. Coordinate Book saves places per game or world, receives locations from Seed Compass, and compares saved locations with projected Overworld/Nether portal coordinates. Material Planner turns blueprint material counts into saved preparation lists with collection progress, remaining quantities and adjustable stack sizes. Plugin development is experimental and the SDK is still evolving.

Coordinate Book also keeps notes and next-session reminders for a selected world or saved server, with links to your places. Command Pocket saves server-specific command templates, fills their parameters and copies the result for you to paste in-game. Geometry Builder previews circles, ellipses, spheres and domes layer by layer, counts blocks and saves generated shapes to your local blueprint library. These tools do not read live player positions or execute game commands.

The optional **Cobble Mesh** companion supports game invitations and local-network contacts. From a running game, create a group for a host and two guests, share an invitation, or join a friend's game. Invitations can include a reviewed game setup: compare the versions, download files from their original providers and prepare a separate instance before joining. Configuration sharing is optional.

Online mode connects after the home screen is ready and reconnects in the background. Manage your public identity, use a temporary contact identity, inspect the actual connection route, or switch Mesh off. Official coordination, test relays and user-authorized private or community relays have distinct roles; optional direct and hole-punch paths depend on suitable network endpoints.

## Current limits

- Forge and NeoForge support Minecraft 1.21.1. Forge 1.20.1 / 47.4.x and NeoForge 1.21.4 / 21.4.x are experimental; installation checks do not establish compatibility with every Mod or loader patch.
- Automatic Java downloads support Windows x64, with Linux x64 and macOS x64/arm64 adapters under internal testing. A manually selected Java executable is never silently replaced.
- Modpack updates support local MRPACK files and create independent copies. Online whole-pack updates, CurseForge pack updates and in-place upgrades are not supported.
- A diagnostic trial copy is a separate experiment; it does not perform a whole-pack upgrade. Play Configuration's MRPACK export excludes unverified, changed and CurseForge Mod files. Mesh setup invitations can reference both providers and preserve restricted-file requirements. Neither workflow embeds Mod jars; review selected configuration text before sharing.
- Blueprint previews support Sponge `.schem` v1–v3 local-palette files and `.litematic` v4–v7, with basic cube rendering. Textures may be read from your installed game; Minecraft textures are not distributed with Cobble.
- Offline skins are displayed in the launcher. Offline profiles cannot join servers that require Microsoft authentication.
- Windows test packages are unsigned. Real Minecraft multiplayer through Mesh has not yet completed end-to-end acceptance.
- Mesh game groups currently support three members, including the host. Public double-NAT hole punching remains unverified; these groups do not create a virtual LAN.
- Seed Compass supports Java Edition 1.12.2, 1.16.1/1.16.5, 1.17.1, 1.18.2, 1.19.2/1.19.4, 1.20.1/1.20.6 and 1.21.1/1.21.3 using [Cubiomes](https://github.com/Cubitect/cubiomes). Version 1.20.1 uses the upstream 1.20 generation family. Biomes, structures and height limits follow the selected version. Structure results are predictions with biome checks, not full terrain validation; custom generators and previously explored chunks may differ.
- Portal planning compares coordinates; actual portal links depend on the game version and nearby portals. Material collection progress is entered manually. Log redaction recognizes common patterns; review the result before sharing.
- Config Doctor restores individual fields only in supported JSON objects; other formats use a full-file preview and restore. Recovery's last clean launch starts with newly recorded exit-zero sessions; older sessions remain unknown. Recycled game originals occupy disk space until the system Recycle Bin is emptied. Language checks are bounded JSON checks, not a complete Minecraft resource-pack validator.

## Feedback

[Report a problem](https://github.com/CobbleLauncher/CobbleLauncher/issues/new?template=bug_report.yml) with your launcher version, operating system and the steps that reproduce it. Settings provides a diagnostic export; review it before attaching it to a public issue.

## Build and run

Install Rust 1.99, Node.js 24, pnpm 11 and the [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/), then:

```sh
git clone https://github.com/CobbleLauncher/CobbleLauncher.git
cd CobbleLauncher
pnpm install
pnpm desktop
```

To build a Windows test package:

```sh
pnpm package:windows
```

The folder and ZIP are written to `target/test-release/`. Extract the entire ZIP and run `Cobble Launcher.exe`. Microsoft Edge WebView2 Runtime is required.

To include Mesh, pass `--mesh-project <path-to-companion-checkout>` using a checkout that provides the `cobble-mesh-companion` target. Mesh is maintained separately from this repository.

For an internal Linux or macOS package, build on the target system:

```sh
pnpm package:unix
```

This produces a portable Linux archive or an unsigned macOS app bundle in `target/test-release/`, with native helpers, bundled plugins and dependency licenses. Linux requires GTK 3, WebKitGTK 4.1 (2.52.6 or newer), and Secret Service for credential storage. Isolated plugin frontends are available on Linux with a supported WebKitGTK runtime. macOS uses host-rendered plugin pages where available; custom frontends remain disabled pending native acceptance. The manual **Package internal Unix clients** workflow builds internal archives. **Publish macOS preview** builds both Mac architectures, adds ad-hoc signatures and publishes checksummed prerelease archives after both builds succeed. It does not change the Windows automatic update feeds.

CurseForge integration uses the project's own approved API key. The manual package workflows read the `COURSE_FORGE_API` Actions secret only for the desktop build. Ordinary builds do not embed a key. For local development, `scripts/launch-desktop-local.mjs` can launch an already-built debug executable with the key from `../.secret.env`. Maintainers can use `pnpm package:local` on Windows, or `pnpm package:unix --embedded-curseforge-key` with the key in the environment on Unix. Credential files must stay outside Git. The development-only MCS token is excluded from release builds.

## Build a plugin

Install `wasm-tools` 1.262.0 and the `wasm32-unknown-unknown` Rust target. Build the bundled plugins with:

```sh
pnpm plugins:build
```

Create and build a Rust viewer plugin:

```sh
cargo run -p cobble-plugin-sdk -- new my-viewer ./my-viewer
cargo run -p cobble-plugin-sdk -- build ./my-viewer ./my-viewer-package
cargo run -p cobble-plugin-sdk -- check ./my-viewer-package
```

The SDK also provides report, provider and tool-form templates. A tool form uses JSON Schema and JSON Forms controls to collect input, runs an isolated Wasm function, and returns plain text. Try `new-tool-form` and `test-tool-form` in the SDK CLI, or explore `examples/command-pocket`. Implement the generated example before using it with real data. See command help for packaging, fixture testing and catalog validation.

For richer custom frontends, explore `examples/modpack-inspector` (dependency graph, search and bounded navigation), `examples/world-dashboard` (snapshot timeline, paged file differences and host-confirmed recovery), `examples/seed-compass` (permission-scoped world metadata, cancellable native computations and a canvas map), `examples/coordinate-book` (private data, unsaved edits, portal geometry and typed handoffs), and `examples/material-planner` (blueprint material handoffs, saved plans and progress). Maintenance consumers include `examples/trial-copy`, `examples/play-manifest`, `examples/file-inbox`, `examples/storage-lens`, `examples/config-doctor`, `examples/crash-doctor` and `examples/lang-doctor`. They use the typed web SDK and declare their access and supported interfaces in `plugin.json`; host-owned confirmations, leases and revision checks protect native writes.

## License and feedback

Cobble Launcher is [MIT licensed](LICENSE). Thanks to [CmlLib.Core](https://github.com/CmlLib/CmlLib.Core) and [XMCL](https://github.com/Voxelum/x-minecraft-launcher) for their open-source launcher references.

Report bugs and suggest improvements through [GitHub Issues](https://github.com/CobbleLauncher/CobbleLauncher/issues).

Cobble is an unofficial community project, independent of Mojang and Microsoft. Minecraft and related marks belong to their respective owners.
