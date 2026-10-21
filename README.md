# CobbleLauncher

**A Minecraft launcher that stays out of your way.**

CobbleLauncher is an open-source launcher for Minecraft: Java Edition. It aims to make installing and starting the game simpler, with the settings you need close at hand. The project is in early development and has no stable release yet. Microsoft sign-in is not available yet; local play uses an offline profile.

## What you can do today

- Use a clean, frameless interface with light and dark themes.
- Create Vanilla, Fabric, Forge, or NeoForge installations from the desktop or command line.
- Follow download progress and retry interrupted installations.
- Choose the official download source, BMCLAPI, or a custom BMCLAPI-compatible source.
- Choose Java automatically or select an installed runtime yourself.
- Save an offline player name and launch installed games.
- Import local Mods, view their details, and enable, disable, or remove them.
- Search Modrinth for compatible Mods and install their required dependencies together.
- Import Vanilla or Fabric `.mrpack` files, with optional client content and pack configurations.
- Rename games and open their folders or logs from the game details page.

Forge and NeoForge currently target Minecraft 1.21.1. Their official installers handle library downloads and patches. Mod and modpack updates are not available yet.

## Get started

You’ll need Rust 1.99, Node.js 24, pnpm 11, and the [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/) for your platform.

```sh
git clone https://github.com/CobbleLauncher/CobbleLauncher.git
cd CobbleLauncher
pnpm install
pnpm desktop
```

## Command line

List available versions or install Vanilla 1.21.1 into a new folder:

```sh
cargo run -p cobble-cli --locked -- versions
cargo run -p cobble-cli --locked -- install 1.21.1 --dir ./game
cargo run -p cobble-cli --locked -- play 1.21.1 --dir ./game --name Player
```

For Fabric, add `--fabric <loader-version>` to the install command. Run `cargo run -p cobble-cli --locked -- fabric-versions 1.21.1` to list loaders.

Forge and NeoForge use their official installers. For Minecraft 1.21.1:

```sh
cargo run -p cobble-cli --locked -- loader-versions forge 1.21.1
cargo run -p cobble-cli --locked -- install 1.21.1 --dir ./forge-game --forge 52.1.0
cargo run -p cobble-cli --locked -- loader-versions neoforge 1.21.1
cargo run -p cobble-cli --locked -- install 1.21.1 --dir ./neo-game --neoforge 21.1.264
```

Search or install Mods with their required dependencies:

```sh
cargo run -p cobble-cli --locked -- mods-search 1.21.1 "mod menu"
cargo run -p cobble-cli --locked -- mod-install 1.21.1 modmenu --dir ./game
```

Search defaults to Fabric; add `--loader forge` or `--loader neoforge` for other loaders. Installation detects the target game’s loader.

To use BMCLAPI, add `--source bmclapi`. Forge and NeoForge installers currently fetch their remaining libraries from upstream. Install a compatible Java runtime first. Offline profiles cannot authenticate to servers that require a Microsoft account.

You can also inspect and install a local Modrinth pack:

```sh
cargo run -p cobble-cli --locked -- pack-info ./pack.mrpack
cargo run -p cobble-cli --locked -- pack-install ./pack.mrpack --dir ./pack-game
```

Optional client files are off by default; add `--optional <path>` to include one. Each import creates a separate game, and interrupted imports can be retried.

## License

The project is licensed under the [MIT License](LICENSE).

Thanks to [CmlLib.Core](https://github.com/CmlLib/CmlLib.Core) and [XMCL](https://github.com/Voxelum/x-minecraft-launcher) for their open-source launcher references.

CobbleLauncher is an unofficial community project. It is not affiliated with, endorsed by, or associated with Mojang or Microsoft. Minecraft and related marks belong to their respective owners.

## Get in touch

For questions or bug reports, visit [GitHub Issues](https://github.com/CobbleLauncher/CobbleLauncher/issues).
