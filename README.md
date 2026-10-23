# Cobble Launcher

**A Minecraft launcher that stays out of your way.**

Cobble is an open-source launcher for Minecraft: Java Edition, built with Rust, Tauri and Vue. Keep your games separate, find compatible content, and get back to playing without digging through folders.

Cobble is in early development. Windows is the primary testing platform; macOS and Linux support is still being validated.

## Play and manage your games

- **Your library, your way.** Continue your last game, pin instances, set custom icons and covers, and create Windows desktop shortcuts.
- **Simple installation.** Install Vanilla, Fabric, Forge or NeoForge. Detect installed Java runtimes and choose settings globally or per game. Downloads support automatic source selection, parallel transfers, cancellation and resume.
- **Content in one place.** Browse Modrinth and CurseForge for modpacks, Mods, resource packs and shaders. Review compatible versions and dependencies before installing.
- **Updates you can undo.** Manage enabled Mods, choose versions, review recent changes and undo managed installs or updates while preserving later local edits.
- **Accounts and appearance.** Switch between Microsoft accounts and offline profiles, assign an account to a game, and preview or apply skins.
- **Useful recovery.** Recover interrupted installations, repair launcher-managed game files, and open the relevant logs when a launch fails.

## Keep your worlds and creations

- Save named world snapshots with deduplicated storage, or enable automatic backups after a game exits. Restore earlier versions with a rollback option.
- Browse screenshots and edit supported Mod configuration files with backups. The optional Config Doctor can also copy compatible keybindings between games.
- Search, save and preview blueprints, inspect materials, and import files into a game's schematic folder. Optional providers can add online blueprint catalogs.
- Import existing launcher instances and local modpacks into separate games. Preview differences between local MRPACK versions before making changes.

## Optional tools and multiplayer

Extensions add viewers, diagnostics and other tools. Browse installed extensions, review permissions, and subscribe to multiple static HTTPS catalogs, including catalogs hosted on GitHub. Plugins can use host-rendered interfaces or an isolated custom frontend, with access limited by their declared permissions.

Bundled examples include Schematic Viewer, Crash Doctor, Config Doctor and Modpack Inspector. Plugin development is experimental and the SDK is still evolving.

The optional **Cobble Mesh** companion supports relay-based game invitations and local-network contacts. You can manage your public identity, use a temporary contact identity, inspect local Mesh status, or switch Mesh off. Automatic persistent connections and direct/hole-punch routing are being developed; they are not part of the current integration.

## Current limits

- Forge and NeoForge currently support Minecraft 1.21.1. Other game/loader combinations are being expanded.
- Java is detected automatically but must currently be installed separately.
- Whole-modpack upgrades currently provide a difference preview; individual managed Mod changes support installation, updates and undo.
- Blueprint previews support Sponge `.schem` v1–v3 local-palette files and `.litematic` v4–v7, with basic cube rendering. Textures may be read from your installed game; Minecraft textures are not distributed with Cobble.
- Offline skins are displayed in the launcher. Offline profiles cannot join servers that require Microsoft authentication.
- Windows test packages are unsigned. Real Minecraft multiplayer through Mesh has not yet completed end-to-end acceptance.

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

CurseForge integration uses the project's own approved API key. The manual **Package Windows client** workflow reads the `COURSE_FORGE_API` Actions secret and enables the release integration. Ordinary builds do not embed a key. For local development, `scripts/launch-desktop-local.mjs` can launch an already-built debug executable with the key from `../.secret.env`; credential files must stay outside Git.

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

The SDK also provides report and provider templates. Implement the generated example before using it with real data. See command help for packaging, fixture testing and catalog validation.

## License and feedback

Cobble Launcher is [MIT licensed](LICENSE). Thanks to [CmlLib.Core](https://github.com/CmlLib/CmlLib.Core) and [XMCL](https://github.com/Voxelum/x-minecraft-launcher) for their open-source launcher references.

Report bugs and suggest improvements through [GitHub Issues](https://github.com/CobbleLauncher/CobbleLauncher/issues).

Cobble is an unofficial community project, independent of Mojang and Microsoft. Minecraft and related marks belong to their respective owners.
