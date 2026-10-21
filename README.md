# CobbleLauncher

**A Minecraft launcher that stays out of your way.**

CobbleLauncher is an open-source launcher for Minecraft: Java Edition. It aims to make installing and starting the game simpler, with the settings you need close at hand. The project is in early development and has no stable release yet. Sign-in and game launching are not ready.

## What you can do today

- Use a clean, frameless interface with light and dark themes.
- Create Vanilla installations from the desktop or command line.
- Follow download progress and retry interrupted installations.
- Choose the official download source, BMCLAPI, or a custom BMCLAPI-compatible source.
- Choose Java automatically or select an installed runtime yourself.

Fabric, Forge, NeoForge, and modpack support are planned.

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
```

To use BMCLAPI, add `--source bmclapi`. Java installation, sign-in, and game launching are not implemented yet.

## License

The project is licensed under the [MIT License](LICENSE).

Thanks to [CmlLib.Core](https://github.com/CmlLib/CmlLib.Core) and [XMCL](https://github.com/Voxelum/x-minecraft-launcher) for their open-source launcher references.

CobbleLauncher is an unofficial community project. It is not affiliated with, endorsed by, or associated with Mojang or Microsoft. Minecraft and related marks belong to their respective owners.

## Get in touch

For questions or bug reports, visit [GitHub Issues](https://github.com/CobbleLauncher/CobbleLauncher/issues).
