// SPDX-License-Identifier: MIT
/** Portable Windows test distribution; this Node script is never shipped as a runtime. */
import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { copyFileSync, cpSync, existsSync, lstatSync, mkdirSync, readFileSync, readdirSync, realpathSync, rmSync, statSync, utimesSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const target = 'x86_64-pc-windows-msvc';
const args = new Set(process.argv.slice(2));
const usage = 'pnpm package:windows [--check | --skip-build]\n  --check       Validate local dependency license coverage; no build or package.\n  --skip-build  Package the existing, independently validated release executable.\n';
if (args.has('--help')) { console.log(usage); process.exit(0); }
for (const arg of args) if (!['--check', '--skip-build'].includes(arg)) throw new Error(`Unknown argument: ${arg}\n${usage}`);
if (args.has('--check') && args.has('--skip-build')) throw new Error('Choose --check or --skip-build.');
if (process.platform !== 'win32' || process.arch !== 'x64') throw new Error('Run this packager on Windows x64.');
const pnpmScript = process.env.npm_execpath;
if (!pnpmScript) throw new Error(`Run through the repository package script.\n${usage}`);

function command(executable, arguments_, options = {}) {
  return execFileSync(executable, arguments_, { cwd: root, encoding: 'utf8', maxBuffer: 64 * 1024 * 1024, ...options });
}
function pnpm(arguments_, options) { return command(process.execPath, [pnpmScript, ...arguments_], options); }
function json(file) { return JSON.parse(readFileSync(file, 'utf8')); }
function portable(file) { return file.split(path.sep).join('/'); }
function packageKey(name, version) { return `${name}@${version}`; }
function folderKey(name, version) { return `${name.replaceAll('/', '__').replaceAll('@', '')}-${version}`; }
const licenseName = /^(?:(?:unlicen[sc]e|licen[sc]e|copying|copyright|notice)(?:$|[._-])|third[-_ ]?party.*(?:notice|licen[sc]e))/i;

function licenseFiles(directory, inherited = false) {
  const files = [];
  for (const item of readdirSync(directory, { withFileTypes: true })) {
    if (item.isSymbolicLink()) continue;
    const file = path.join(directory, item.name);
    if (item.isDirectory() && !['node_modules', 'target', '.git', '.github'].includes(item.name)) {
      files.push(...licenseFiles(file, inherited || /^licen[sc]es?$/i.test(item.name)));
    } else if (item.isFile() && (inherited || licenseName.test(item.name))) {
      if (statSync(file).size > 2 * 1024 * 1024) throw new Error(`License text is unexpectedly large: ${item.name}`);
      if (!readFileSync(file).includes(0)) files.push(file);
    }
  }
  return files;
}

function collectDependencies() {
  const metadata = JSON.parse(command('cargo', ['metadata', '--locked', '--offline', '--format-version', '1', '--filter-platform', target]));
  const tree = command('cargo', ['tree', '--locked', '--offline', '--target', target, '-p', 'cobble-desktop', '--edges', 'normal,build', '--prefix', 'none', '--format', '{p}']);
  const selected = new Set(tree.split(/\r?\n/).flatMap((line) => {
    const match = /^(\S+) v(\S+)/.exec(line);
    return match ? [packageKey(match[1], match[2])] : [];
  }));
  const packages = [];
  const copies = [];
  const missing = [];
  for (const item of metadata.packages.filter((item) => item.source && selected.has(packageKey(item.name, item.version)))) {
    const directory = path.dirname(item.manifest_path);
    const retained = path.join(root, 'third-party', 'licenses', 'rust', `${item.name}-${item.version}`);
    let files = licenseFiles(directory);
    let sourceDirectory = directory;
    if (item.license_file) {
      const declared = path.resolve(directory, item.license_file);
      if (declared.startsWith(directory + path.sep) && existsSync(declared) && !files.includes(declared)) files.push(declared);
    }
    let licenseOrigin = 'Published crate archive';
    if (!files.length && existsSync(retained)) {
      files = licenseFiles(retained); sourceDirectory = retained; licenseOrigin = 'Retained upstream license; see third-party/licenses/packaging-sources.json and THIRD_PARTY_NOTICES.md';
    }
    const destination = `third-party/licenses/cargo/${folderKey(item.name, item.version)}`;
    const included = files.map((file) => {
      const relative = path.relative(sourceDirectory, file);
      copies.push({ source: file, destination: path.join(destination, relative) });
      return `${destination}/${portable(relative)}`;
    });
    if (!included.length) missing.push(`Cargo ${item.name} ${item.version}`);
    packages.push({ ecosystem: 'cargo', name: item.name, version: item.version, license: item.license,
      source: `https://crates.io/api/v1/crates/${item.name}/${item.version}/download`, repository: item.repository,
      license_origin: licenseOrigin, license_files: included });
  }
  const production = JSON.parse(pnpm(['licenses', 'list', '--prod', '--json']));
  for (const item of Object.values(production).flat()) {
    for (const version of item.versions) {
      const directory = item.paths.find((directory) => {
        const manifest = json(path.join(directory, 'package.json'));
        return manifest.name === item.name && manifest.version === version;
      });
      if (!directory) throw new Error(`Cannot locate production dependency ${item.name} ${version}`);
      const destination = `third-party/licenses/npm/${folderKey(item.name, version)}`;
      const included = licenseFiles(directory).map((file) => {
        const relative = path.relative(directory, file);
        copies.push({ source: file, destination: path.join(destination, relative) });
        return `${destination}/${portable(relative)}`;
      });
      if (!included.length) missing.push(`npm ${item.name} ${version}`);
      packages.push({ ecosystem: 'npm', name: item.name, version, license: item.license,
        source: `https://registry.npmjs.org/${item.name}/${version}`, license_files: included });
    }
  }
  if (missing.length) throw new Error(`Missing original license texts; retain them before packaging:\n${missing.join('\n')}`);
  packages.sort((left, right) => `${left.ecosystem}/${left.name}@${left.version}`.localeCompare(`${right.ecosystem}/${right.name}@${right.version}`, 'en'));
  return { metadata, packages, copies };
}

function main() {
  if (!args.has('--check') && !args.has('--skip-build')) pnpm(['desktop:build'], { stdio: 'inherit' });
  const { metadata, packages, copies } = collectDependencies();
  const desktop = metadata.packages.find((item) => item.name === 'cobble-desktop' && item.source === null);
  if (!desktop || !/^[0-9A-Za-z.+-]+$/.test(desktop.version)) throw new Error('Invalid application version.');
  const config = json(path.join(root, 'apps/desktop/src-tauri/tauri.conf.json'));
  if (config.version !== desktop.version) throw new Error('Tauri and Cargo versions must agree before packaging.');
  console.log(`License coverage: ${packages.filter((item) => item.ecosystem === 'cargo').length} Windows Cargo normal/build packages, ${packages.filter((item) => item.ecosystem === 'npm').length} production frontend packages.`);
  if (args.has('--check')) { console.log('Read-only check passed; no build or distribution created.'); return; }
  const executable = path.join(metadata.target_directory, 'release/cobble-desktop.exe');
  const bytes = readFileSync(executable);
  const pe = bytes.readUInt32LE(0x3c);
  if (bytes.toString('ascii', 0, 2) !== 'MZ' || bytes.toString('ascii', pe, pe + 4) !== 'PE\0\0' || bytes.readUInt16LE(pe + 4) !== 0x8664) throw new Error('The release executable must be a Windows x64 PE binary.');
  const outputRoot = path.resolve(root, 'target/test-release');
  const baseName = `${desktop.version}-windows-x64`;
  const directory = path.resolve(outputRoot, baseName);
  const archive = directory + '.zip';
  if (path.dirname(directory) !== outputRoot || path.basename(directory) !== baseName) throw new Error('Invalid generated package path.');
  mkdirSync(outputRoot, { recursive: true });
  if (path.relative(realpathSync(root), realpathSync(outputRoot)).toLowerCase() !== path.join('target', 'test-release').toLowerCase()) throw new Error('The test-release output root must stay inside this workspace.');
  for (const output of [directory, archive, archive + '.sha256']) {
    if (path.dirname(output) !== outputRoot || ![baseName, baseName + '.zip', baseName + '.zip.sha256'].includes(path.basename(output))) throw new Error('Invalid generated artifact path.');
    if (existsSync(output) && lstatSync(output).isSymbolicLink()) throw new Error('Generated artifacts must not be links.');
  }
  rmSync(directory, { recursive: true, force: true });
  rmSync(archive, { force: true });
  rmSync(archive + '.sha256', { force: true });
  mkdirSync(directory, { recursive: true });
  copyFileSync(executable, path.join(directory, 'Cobble Launcher.exe'));
  copyFileSync(path.join(root, 'LICENSE'), path.join(directory, 'LICENSE'));
  copyFileSync(path.join(root, 'THIRD_PARTY_NOTICES.md'), path.join(directory, 'THIRD_PARTY_NOTICES.md'));
  cpSync(path.join(root, 'third-party/licenses'), path.join(directory, 'third-party/licenses'), { recursive: true, dereference: false });
  for (const file of copies) {
    const destination = path.join(directory, file.destination);
    mkdirSync(path.dirname(destination), { recursive: true }); copyFileSync(file.source, destination);
  }
  // Registry archives can date licenses before ZIP's supported timestamp range.
  const zipMinimum = new Date(1980, 0, 2);
  for (const file of licenseFiles(path.join(directory, 'third-party/licenses'))) {
    const metadata = statSync(file);
    if (metadata.mtime < zipMinimum) utimesSync(file, metadata.atime, zipMinimum);
  }
  writeFileSync(path.join(directory, 'DEPENDENCIES.json'), JSON.stringify({
    application: 'Cobble Launcher', version: desktop.version, target,
    scope: 'Locked desktop Cargo normal and build dependency graph filtered for Windows x64, plus installed pnpm production dependencies. Includes build tools and potentially tree-shaken frontend packages; not a symbol-level linked-binary inventory. System Windows libraries and the separately installed WebView2 Runtime are excluded.',
    packages,
  }, null, 2) + '\n');
  writeFileSync(path.join(directory, 'START-HERE.txt'), `Cobble Launcher ${desktop.version} - Windows x64 test build\n\nThis is an unsigned testing build, not a stable release. Windows may show an unknown publisher warning. macOS and Linux have not been verified by this package.\n\n1. Extract the entire ZIP into a writable folder.\n2. Install Microsoft Edge WebView2 Evergreen Runtime if it is missing: https://developer.microsoft.com/microsoft-edge/webview2/\n3. Open Cobble Launcher.exe. No Node.js, Python or .NET installation is needed.\n4. Install Java separately for Minecraft. Minecraft 1.21.1 requires Java 21; select Java in the launcher's settings.\n\nSettings and account credentials use your user profile; game locations are configurable. A Microsoft account is optional for local offline play.\n\nLICENSE covers Cobble's own code. Third-party licenses are in third-party/licenses; DEPENDENCIES.json lists package versions, original source URLs and retained license texts. Minecraft, Java, Mods, game data and credentials are not included.\n`);
  command('powershell.exe', ['-NoProfile', '-NonInteractive', '-Command', "$ErrorActionPreference = 'Stop'; Compress-Archive -LiteralPath $env:COBBLE_PACKAGE_DIR -DestinationPath $env:COBBLE_PACKAGE_ZIP -CompressionLevel Optimal"], {
    env: { ...process.env, COBBLE_PACKAGE_DIR: directory, COBBLE_PACKAGE_ZIP: archive }, stdio: 'inherit',
  });
  const checksum = createHash('sha256').update(readFileSync(archive)).digest('hex');
  writeFileSync(archive + '.sha256', `${checksum}  ${path.basename(archive)}\n`);
  console.log(`Test distribution: ${directory}\nZIP: ${archive}`);
}
try { main(); } catch (error) { console.error(`Packaging failed: ${error.message}`); process.exitCode = 1; }
