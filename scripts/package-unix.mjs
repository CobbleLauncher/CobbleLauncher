// SPDX-License-Identifier: MIT
/** Native Unix packages. Independently implemented; no launcher source adapted.
 * License inventory reuses the Windows collector with an explicit native Cargo target.
 * System GTK/WebKit and macOS frameworks remain OS prerequisites, not bundled dependencies.
 */
import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { chmodSync, copyFileSync, existsSync, lstatSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, renameSync, rmSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { collectDependencies, desktopBuildArguments, embeddedCredential, withoutProviderCredentials } from './package-windows.mjs';
import { officialPluginIds } from './official-plugins.mjs';
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const usage = 'node scripts/package-unix.mjs [--skip-build] [--embedded-curseforge-key] [--adhoc-sign] [--platform linux|darwin] [--arch x64|arm64] [--binary-dir PATH] [--plugins-dir PATH] [--output-dir PATH] [--mesh-project PATH] [--mesh-binary PATH]';
export function unixDesktopBuildEnvironment(environment, providerKey) {
  return providerKey
    ? { ...withoutProviderCredentials(environment), COURSE_FORGE_API: providerKey }
    : withoutProviderCredentials(environment);
}
export function parseUnixArguments(argv) {
  const result = { platform: process.platform, arch: process.arch, skipBuild: false, embeddedCurseforgeKey: false, adhocSign: false };
  const keys = { '--platform': 'platform', '--arch': 'arch', '--binary-dir': 'binaryDir', '--plugins-dir': 'pluginsDir', '--output-dir': 'outputDir', '--mesh-project': 'meshProject', '--mesh-binary': 'meshBinary' };
  const seen = new Set();
  for (let i = 0; i < argv.length; i++) {
    const option = argv[i];
    if (seen.has(option)) throw new Error(`Duplicate option: ${option}`);
    seen.add(option);
    if (option === '--skip-build') { result.skipBuild = true; continue; }
    if (option === '--embedded-curseforge-key') { result.embeddedCurseforgeKey = true; continue; }
    if (option === '--adhoc-sign') { result.adhocSign = true; continue; }
    if (option === '--help' && argv.length === 1) return { help: true };
    if (!keys[option] || !argv[i + 1] || argv[i + 1].startsWith('--')) throw new Error(`Invalid option: ${option}\n${usage}`);
    result[keys[option]] = argv[++i];
  }
  unixTarget(result.platform, result.arch);
  if (!!result.meshBinary !== !!result.meshProject) throw new Error('--mesh-binary and --mesh-project must be supplied together.');
  if (result.embeddedCurseforgeKey && result.skipBuild) throw new Error('Embedded-key packaging requires a fresh build; do not combine with --skip-build.');
  if (result.adhocSign && result.platform !== 'darwin') throw new Error('--adhoc-sign is supported only for macOS packages.');
  return result;
}
export function unixTarget(platform, arch) {
  if (!['linux', 'darwin'].includes(platform) || !['x64', 'arm64'].includes(arch)) throw new Error('Supported native package targets: Linux/macOS x64/arm64.');
  return `${arch === 'x64' ? 'x86_64' : 'aarch64'}-${platform === 'linux' ? 'unknown-linux-gnu' : 'apple-darwin'}`;
}
export function validateUnixBinary(file, platform, arch) {
  if (!lstatSync(file).isFile()) throw new Error(`Executable must be a regular file: ${file}`);
  const bytes = readFileSync(file);
  const machine = arch === 'x64' ? 62 : 183;
  const elf = bytes.length >= 64 && bytes.subarray(0, 4).equals(Buffer.from([127, 69, 76, 70])) && bytes[4] === 2 && bytes[5] === 1 && [2, 3].includes(bytes.readUInt16LE(16)) && bytes.readUInt16LE(18) === machine;
  const mach = bytes.length >= 32 && bytes.readUInt32LE(0) === 0xfeedfacf && bytes.readUInt32LE(12) === 2 && bytes.readUInt32LE(4) === (arch === 'x64' ? 0x1000007 : 0x100000c);
  if (!(platform === 'linux' ? elf : mach)) throw new Error(`Expected ${platform}/${arch} executable: ${file}`);
}
// Reject links throughout inputs rather than dereference paths outside the intended package.
export function copyRegularTree(source, destination) {
  const metadata = lstatSync(source);
  if (metadata.isSymbolicLink()) throw new Error(`Package input must not be a link: ${source}`);
  if (metadata.isDirectory()) {
    mkdirSync(destination, { recursive: true }); chmodSync(destination, 0o755);
    for (const entry of readdirSync(source).sort()) copyRegularTree(path.join(source, entry), path.join(destination, entry));
  } else if (metadata.isFile()) {
    mkdirSync(path.dirname(destination), { recursive: true }); copyFileSync(source, destination); chmodSync(destination, 0o644);
  } else throw new Error(`Unsupported package input: ${source}`);
}
function safeDestination(base, relative) {
  const destination = path.resolve(base, relative);
  if (!relative || path.isAbsolute(relative) || !destination.startsWith(path.resolve(base) + path.sep)) throw new Error(`Unsafe inventory path: ${relative}`);
  return destination;
}
export function assembleUnixPackage({ directory, repository = root, platform, arch, version, binaryDir, pluginsDir, inventory, pluginIds = officialPluginIds, meshBinary, adhocSign = false, codeSign = null }) {
  unixTarget(platform, arch);
  if (!/^[0-9A-Za-z.+-]+$/.test(version)) throw new Error('Invalid application version.');
  const app = platform === 'darwin' ? path.join(directory, 'Cobble Launcher.app/Contents') : directory;
  const executables = platform === 'darwin' ? path.join(app, 'MacOS') : app;
  const resources = platform === 'darwin' ? path.join(app, 'Resources') : app;
  mkdirSync(executables, { recursive: true }); mkdirSync(resources, { recursive: true });
  const executableNames = ['cobble-desktop', 'cobble-plugin-host', ...(meshBinary ? ['cobble-mesh-companion'] : [])];
  for (const name of executableNames) {
    const source = name === 'cobble-mesh-companion' ? meshBinary : path.join(binaryDir, name);
    validateUnixBinary(source, platform, arch);
    copyRegularTree(source, path.join(executables, name)); chmodSync(path.join(executables, name), 0o755);
  }
  for (const id of pluginIds) {
    if (!/^[a-z0-9-]+$/.test(id)) throw new Error('Invalid plugin ID.');
    const source = path.join(pluginsDir, `${id}-package`);
    const manifest = JSON.parse(readFileSync(path.join(source, 'plugin.json'), 'utf8'));
    if (manifest.id !== id || !lstatSync(path.join(source, 'component.wasm')).isFile()) throw new Error(`Invalid bundled plugin: ${id}`);
    copyRegularTree(source, path.join(resources, 'plugins', id));
  }
  for (const name of ['LICENSE', 'THIRD_PARTY_NOTICES.md']) copyRegularTree(path.join(repository, name), path.join(resources, name));
  copyRegularTree(path.join(repository, 'third-party/licenses'), path.join(resources, 'third-party/licenses'));
  for (const copy of inventory.copies) copyRegularTree(copy.source, safeDestination(resources, copy.destination));
  if (meshBinary) {
    const companion = inventory.companion;
    if (!companion) throw new Error('Missing companion dependency inventory.');
    copyRegularTree(companion.license, safeDestination(resources, companion.licenseDestination));
    copyRegularTree(companion.notices, safeDestination(resources, companion.noticesDestination));
  }
  for (const item of inventory.packages) {
    if (!item.license_files?.length) throw new Error(`Missing license inventory: ${item.name}`);
    for (const license of item.license_files) if (!lstatSync(safeDestination(resources, license)).isFile()) throw new Error(`Missing license file: ${license}`);
  }
  writeFileSync(path.join(resources, 'DEPENDENCIES.json'), JSON.stringify({ application: 'Cobble Launcher', version, target: unixTarget(platform, arch), scope: 'Locked desktop and plugin-host target-filtered Cargo normal/build graphs and installed pnpm production packages. Includes build tools and potentially tree-shaken frontend packages. System shared libraries/frameworks are excluded; this is not a symbol-level linked-binary inventory.', packages: inventory.packages }, null, 2) + '\n');
  if (platform === 'darwin') {
    copyRegularTree(path.join(repository, 'apps/desktop/src-tauri/icons/icon.icns'), path.join(resources, 'icon.icns'));
    writeFileSync(path.join(app, 'Info.plist'), `<?xml version="1.0" encoding="UTF-8"?>\n<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">\n<plist version="1.0"><dict><key>CFBundleExecutable</key><string>cobble-desktop</string><key>CFBundleIdentifier</key><string>dev.cobble-launcher.internal</string><key>CFBundleName</key><string>Cobble Launcher</string><key>CFBundlePackageType</key><string>APPL</string><key>CFBundleIconFile</key><string>icon.icns</string><key>CFBundleShortVersionString</key><string>${version.split('-')[0]}</string><key>CFBundleVersion</key><string>${version.split('-')[0]}</string><key>NSHighResolutionCapable</key><true/></dict></plist>\n`);
  } else {
    copyRegularTree(path.join(repository, 'apps/desktop/src-tauri/icons/128x128.png'), path.join(resources, 'cobble-launcher.png'));
    writeFileSync(path.join(directory, 'cobble-launcher.desktop'), '[Desktop Entry]\nType=Application\nName=Cobble Launcher\nExec=cobble-desktop\nIcon=cobble-launcher\nTerminal=false\nCategories=Game;\n');
  }
  writeFileSync(path.join(directory, 'START-HERE.txt'), `Cobble Launcher ${version} ${platform === 'darwin' && adhocSign ? 'macOS preview' : 'internal'} ${platform}/${arch} package\n${platform === 'darwin' ? (adhocSign ? 'Ad-hoc signed for local execution; not Developer ID-signed or notarized. Gatekeeper acceptance has not been established. Open Cobble Launcher.app.' : 'Unsigned and not notarized; native acceptance is pending. Open Cobble Launcher.app.') : 'Extract the archive and run ./cobble-desktop in a graphical session. Requires GTK 3, WebKitGTK 4.1 (2.52.6 or newer), and Secret Service for credential storage.'}\nKeep all files together. Java is not included; select an installed compatible runtime. ${platform === 'darwin' ? 'Plugins use host-rendered views pending native acceptance.' : 'Isolated plugin frontends require WebKitGTK 4.1 version 2.52.6 or newer.'}\nDependency sources and licenses: ${platform === 'darwin' ? 'Cobble Launcher.app/Contents/Resources/' : ''}DEPENDENCIES.json and third-party/licenses.\n`);
  writeFileSync(path.join(directory, 'README-INTERNAL.txt'), `Cobble Launcher ${version} - ${platform === 'darwin' && adhocSign ? 'macOS preview' : 'internal native'} package

${platform === 'linux' ? 'Linux: extract the entire archive and run ./cobble-desktop in a graphical desktop session. Tested runtime: Ubuntu 24.04.2 under WSL2, using Xvfb with software rendering. Requires GTK 3, WebKitGTK 4.1 version 2.52.6 or newer, librsvg, and Secret Service. Other distributions need equivalent runtime libraries and a supported WebKitGTK version. No system installation is performed. Isolated custom plugin frontends require WebKitGTK 4.1 version 2.52.6 or newer; Config Doctor frontend and Wasm consumption have been validated. The optional .desktop entry expects cobble-desktop on PATH and cobble-launcher.png installed as an application icon; launching the binary directly requires neither.' : `macOS: open Cobble Launcher.app. ${adhocSign ? 'The app and helper executables are ad-hoc signed, not Developer ID-signed or notarized; Gatekeeper acceptance has not been established.' : 'This bundle is unsigned and not notarized; native UI and Gatekeeper acceptance remain pending.'} Helpers reside in Contents/MacOS and plugins/notices in Contents/Resources. Plugins use host-rendered views pending native frontend acceptance.`}

${platform === 'linux' ? 'Linux x64 supports managed Temurin 17 downloads and Java 21 runtime archive extraction. Minecraft 1.21.1 launch to the main menu and normal quit were verified on Ubuntu 24.04.2 under WSL2/Xvfb. Launching the game through the native UI and recovering after SIGTERM were verified separately from the normal menu-quit run. Secret Service credential storage passed isolated roundtrip and locked tests; Microsoft sign-in remains unverified.' : 'Select an installed compatible Java runtime. Managed-Java and real-game acceptance remain pending on macOS. Microsoft sign-in and native credential storage acceptance remain unverified.'} Java, Minecraft, mods and worlds are not included. ${platform === 'linux' ? 'Linux Mesh game hosting includes native listener ownership verification; real Minecraft multiplayer acceptance remains unverified.' : 'macOS Mesh game hosting remains gated until native listener ownership checks pass.'}

Keep the entire package together. Settings and credentials use the operating-system user profile. Java, Minecraft, mods and worlds are not included. Third-party licenses, source URLs and target-filtered dependency records are retained in ${platform === 'darwin' ? 'Cobble Launcher.app/Contents/Resources/' : ''}DEPENDENCIES.json and third-party/licenses. The package-root PACKAGE-INVENTORY.json records packaged file hashes and modes.
`);
  if (adhocSign) {
    if (platform !== 'darwin' || process.platform !== 'darwin' && !codeSign) throw new Error('Ad-hoc signing requires a native macOS package job.');
    const sign = codeSign ?? (args => execFileSync('codesign', args, { stdio: 'inherit', env: withoutProviderCredentials(process.env) }));
    for (const name of executableNames) {
      const identifier = name === 'cobble-desktop' ? 'dev.cobble-launcher.internal' : `dev.cobble-launcher.internal.${name}`;
      sign(['--force', '--sign', '-', '--timestamp=none', '--identifier', identifier, path.join(executables, name)]);
    }
    const appBundle = path.join(directory, 'Cobble Launcher.app');
    sign(['--force', '--sign', '-', '--timestamp=none', appBundle]);
    sign(['--verify', '--deep', '--strict', appBundle]);
  }
  const files = [];
  function scan(dir) { for (const name of readdirSync(dir).sort()) { const file = path.join(dir, name); if (lstatSync(file).isDirectory()) scan(file); else files.push({ path: path.relative(directory, file).split(path.sep).join('/'), sha256: createHash('sha256').update(readFileSync(file)).digest('hex'), mode: lstatSync(file).mode & 0o777 }); } }
  scan(directory);
  writeFileSync(path.join(directory, 'PACKAGE-INVENTORY.json'), JSON.stringify({ platform, arch, signing: platform === 'darwin' ? (adhocSign ? 'ad-hoc' : 'unsigned') : 'not-applicable', notarized: false, files }, null, 2) + '\n');
  return { executables, resources };
}
function run(executable, args, options = {}) { return execFileSync(executable, args, { cwd: root, env: withoutProviderCredentials(process.env), stdio: 'inherit', ...options }); }
function pnpm(args, options = {}) { return process.env.npm_execpath ? run(process.execPath, [process.env.npm_execpath, ...args], options) : run('pnpm', args, options); }
export function main(argv = process.argv.slice(2)) {
  const options = parseUnixArguments(argv);
  if (options.help) { console.log(usage); return; }
  if (process.platform !== options.platform || process.arch !== options.arch) throw new Error('Package on the matching native OS and architecture to collect its actual dependency graph.');
  const providerKey = options.embeddedCurseforgeKey ? embeddedCredential(process.env) : null;
  const target = unixTarget(options.platform, options.arch);
  // Check the complete target license inventory before spending time compiling.
  const inventory = collectDependencies(options.meshProject ? path.resolve(options.meshProject) : null, target, true);
  const desktop = inventory.metadata.packages.find(item => item.name === 'cobble-desktop' && item.source === null);
  const config = JSON.parse(readFileSync(path.join(root, 'apps/desktop/src-tauri/tauri.conf.json'), 'utf8'));
  if (!desktop || desktop.version !== config.version) throw new Error('Tauri/Cargo application versions must agree.');
  if (!options.skipBuild) {
    pnpm(['plugins:build']); run('cargo', ['build', '--locked', '--release', '-p', 'cobble-plugin-host']);
    if (options.embeddedCurseforgeKey) pnpm(['build']);
    pnpm(desktopBuildArguments(options.embeddedCurseforgeKey), {
      ...(providerKey ? { env: unixDesktopBuildEnvironment(process.env, providerKey) } : {}),
    });
    if (options.meshProject) run('cargo', ['build', '--locked', '--release', '--manifest-path', path.join(path.resolve(options.meshProject), 'Cargo.toml'), '--bin', 'cobble-mesh-companion']);
  }
  const output = path.resolve(options.outputDir ?? path.join(root, 'target/test-release'));
  mkdirSync(output, { recursive: true });
  const name = `${desktop.version}-${options.platform}-${options.arch}-${options.adhocSign ? 'preview' : 'internal'}`;
  const destination = path.join(output, name);
  const archive = destination + '.tar.gz';
  for (const file of [destination, archive, archive + '.sha256']) if (existsSync(file)) throw new Error(`Refusing to replace an existing artifact: ${file}`);
  const staging = mkdtempSync(path.join(output, '.unix-package-'));
  try {
    const directory = path.join(staging, name);
    assembleUnixPackage({ ...options, directory, version: desktop.version, binaryDir: path.resolve(options.binaryDir ?? path.join(inventory.metadata.target_directory, 'release')), pluginsDir: path.resolve(options.pluginsDir ?? inventory.metadata.target_directory), inventory, meshBinary: options.meshBinary ? path.resolve(options.meshBinary) : undefined });
    run('tar', ['-czf', path.join(staging, name + '.tar.gz'), '-C', staging, name]);
    const digest = createHash('sha256').update(readFileSync(path.join(staging, name + '.tar.gz'))).digest('hex');
    renameSync(directory, destination); renameSync(path.join(staging, name + '.tar.gz'), archive);
    writeFileSync(archive + '.sha256', `${digest}  ${path.basename(archive)}\n`, { flag: 'wx' });
    console.log(`Native package: ${archive}`);
  } finally { rmSync(staging, { recursive: true, force: true }); }
}
if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) {
  try { main(); } catch (error) { console.error(`Packaging failed: ${error.message}`); process.exitCode = 1; }
}
