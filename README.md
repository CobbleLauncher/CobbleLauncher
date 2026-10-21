# CobbleLauncher

**A Minecraft launcher that stays out of your way.**

CobbleLauncher is an open-source launcher for Minecraft: Java Edition. It aims to make installing and starting the game simpler, with the settings you need close at hand. The project is in early development and has no stable release yet. Microsoft sign-in is not available yet; local play uses an offline profile.

## What you can do today

- Use a clean, frameless interface with light and dark themes.
- Create Vanilla or Fabric installations from the desktop or command line.
- Follow download progress and retry interrupted installations.
- Choose the official download source, BMCLAPI, or a custom BMCLAPI-compatible source.
- Choose Java automatically or select an installed runtime yourself.
- Save an offline player name and launch installed games.
- Import local Mods, view their details, and enable, disable, or remove them.
- Search Modrinth for compatible Fabric Mods and install their required dependencies together.

Modpack import, Forge, and NeoForge support are planned. Mod updates are not available yet.

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

For an installed Fabric game, search or install Mods with their required dependencies:

```sh
cargo run -p cobble-cli --locked -- mods-search 1.21.1 "mod menu"
cargo run -p cobble-cli --locked -- mod-install 1.21.1 modmenu --dir ./game
```

To use BMCLAPI, add `--source bmclapi`. Install a compatible Java runtime first. Offline profiles cannot authenticate to servers that require a Microsoft account.

## License

The project is licensed under the [MIT License](LICENSE).

Thanks to [CmlLib.Core](https://github.com/CmlLib/CmlLib.Core) and [XMCL](https://github.com/Voxelum/x-minecraft-launcher) for their open-source launcher references.

CobbleLauncher is an unofficial community project. It is not affiliated with, endorsed by, or associated with Mojang or Microsoft. Minecraft and related marks belong to their respective owners.

## Get in touch

For questions or bug reports, visit [GitHub Issues](https://github.com/CobbleLauncher/CobbleLauncher/issues).
