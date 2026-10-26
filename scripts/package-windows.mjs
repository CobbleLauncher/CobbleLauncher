// SPDX-License-Identifier: MIT
/** Portable Windows test distribution; this Node script is never shipped as a runtime. */
import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { closeSync, copyFileSync, cpSync, existsSync, lstatSync, mkdirSync, mkdtempSync, openSync, readFileSync, readdirSync, realpathSync, renameSync, rmSync, statSync, utimesSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { officialPluginIds } from './official-plugins.mjs';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const target = 'x86_64-pc-windows-msvc';
const usage = 'pnpm package:windows [--check | --skip-build] [--mesh-project <absolute-repo-path>] [--embedded-curseforge-key] [--label <name>]\n  --check       Validate local dependency license coverage; no build or package.\n  --skip-build  Package the existing, independently validated release executable.\n  --embedded-curseforge-key  Explicit release opt-in; requires COURSE_FORGE_API.\n  --mesh-project Include and build the optional Mesh companion from an existing repository.\n  --label       Write a separately named package, leaving the default package untouched.\n';
const pnpmScript = process.env.npm_execpath;

export function withoutProviderCredentials(environment) {
  return Object.fromEntries(Object.entries(environment).filter(([name]) => name.toUpperCase() !== 'COURSE_FORGE_API' && !name.toUpperCase().startsWith('MCS')));
}
export function desktopBuildArguments(embedded) {
  return embedded ? ['--filter', '@cobble/desktop', 'tauri', 'build', '--no-bundle', '--features', 'embedded-curseforge-key', '--config', JSON.stringify({ build: { beforeBuildCommand: null } }), '--', '--locked'] : ['desktop:build'];
}
export function embeddedCredential(environment) {
  const value = environment.COURSE_FORGE_API?.trim();
  if (!value || value.length > 4096 || !/^[\x21-\x7e]+$/.test(value)) throw new Error('Embedded-key packaging requires a valid COURSE_FORGE_API environment value.');
  return value;
}
function command(executable, arguments_, options = {}) {
  return execFileSync(executable, arguments_, { env: withoutProviderCredentials(process.env), cwd: root, encoding: 'utf8', maxBuffer: 64 * 1024 * 1024, ...options });
}
function pnpm(arguments_, options) { return pnpmScript ? command(process.execPath, [pnpmScript, ...arguments_], options) : command('pnpm', arguments_, options); }
function json(file) { return JSON.parse(readFileSync(file, 'utf8')); }
function portable(file) { return file.split(path.sep).join('/'); }
function packageKey(name, version) { return `${name}@${version}`; }
function folderKey(name, version) { return `${name.replaceAll('/', '__').replaceAll('@', '')}-${version}`; }
export function retainedLicenseOrigin(entries) {
  const canonicalLicenses = [...new Set(entries.filter((entry) => entry.provenance === 'canonical-license').map((entry) => entry.license))];
  if (canonicalLicenses.length) return `Canonical ${canonicalLicenses.join(', ')} text selected under upstream grant; see third-party/licenses/packaging-sources.json and THIRD_PARTY_NOTICES.md`;
  return 'Retained upstream license; see third-party/licenses/packaging-sources.json and THIRD_PARTY_NOTICES.md';
}
export function parseArguments(argv) {
  const options = { check: false, skipBuild: false, help: false, meshProject: null, embeddedCurseforgeKey: false };
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === '--embedded-curseforge-key') { options.embeddedCurseforgeKey = true; continue; }
    if (argument === '--help') { options.help = true; continue; }
    if (argument === '--check') { options.check = true; continue; }
    if (argument === '--skip-build') { options.skipBuild = true; continue; }
    if (argument === '--label') {
      const value = argv[++index];
      if (options.label || !value || !/^[a-z0-9][a-z0-9-]{0,31}$/.test(value)) throw new Error('--label requires one short lowercase package label.');
      options.label = value;
      continue;
    }
    if (argument === '--mesh-project') {
      if (options.meshProject) throw new Error('--mesh-project may be provided only once.');
      const value = argv[index + 1];
      if (!value || value.startsWith('--')) throw new Error('--mesh-project requires an absolute repository path.');
      if (!path.isAbsolute(value)) throw new Error('--mesh-project requires an absolute repository path.');
      options.meshProject = value;
      index += 1;
      continue;
    }
    throw new Error(`Unknown argument: ${argument}\n${usage}`);
  }
  if (options.embeddedCurseforgeKey && (options.check || options.skipBuild)) throw new Error('Embedded-key packaging requires a fresh build; do not combine with --check or --skip-build.');
  if (options.check && options.skipBuild) throw new Error('Choose --check or --skip-build.');
  if (options.help && (argv.length !== 1 || argv[0] !== '--help')) throw new Error('--help cannot be combined with other options.');
  return options;
}

export function mergeCargoPackages(groups) {
  const merged = new Map();
  for (const group of groups) {
    for (const item of group.packages) {
      const key = packageKey(item.name, item.version);
      const existing = merged.get(key);
      if (!existing) {
        merged.set(key, { ...item, source_projects: [group.project] });
        continue;
      }
      for (const field of ['license', 'source', 'repository']) {
        if (existing[field] !== item[field]) throw new Error(`Conflicting Cargo metadata for ${key} (${field}).`);
      }
      if (JSON.stringify(existing.license_files) !== JSON.stringify(item.license_files)) throw new Error(`Conflicting Cargo license files for ${key}.`);
      if (!existing.source_projects.includes(group.project)) existing.source_projects.push(group.project);
    }
  }
  return [...merged.values()];
}

function validateMeshProject(value) {
  if (!path.isAbsolute(value)) throw new Error('--mesh-project requires an absolute repository path.');
  if (!existsSync(value) || !statSync(value).isDirectory()) throw new Error(`Mesh project directory does not exist: ${value}`);
  const projectRoot = realpathSync(value);
  const gitRoot = command('git', ['-C', projectRoot, 'rev-parse', '--show-toplevel']).trim();
  if (path.resolve(gitRoot).toLowerCase() !== projectRoot.toLowerCase()) throw new Error('--mesh-project must name the Git repository root.');
  const manifest = path.join(projectRoot, 'Cargo.toml');
  if (!existsSync(manifest) || !statSync(manifest).isFile()) throw new Error('--mesh-project must contain a Cargo.toml.');
  for (const file of ['LICENSE', 'THIRD_PARTY_NOTICES.md']) {
    const filePath = path.join(projectRoot, file);
    if (!existsSync(filePath) || !lstatSync(filePath).isFile()) throw new Error(`Mesh project is missing a regular ${file} file.`);
  }
  return projectRoot;
}

function cargoMetadata(projectRoot, platformTarget = target) {
  const args = ['metadata', '--locked', '--offline', '--format-version', '1', '--filter-platform', platformTarget];
  if (projectRoot !== root) args.push('--manifest-path', path.join(projectRoot, 'Cargo.toml'));
  return JSON.parse(command('cargo', args, { cwd: projectRoot }));
}

function collectCargoProject(projectRoot, packageName, project, metadata = null, platformTarget = target) {
  metadata ??= cargoMetadata(projectRoot, platformTarget);
  const treeArgs = ['tree', '--locked', '--offline', '--target', platformTarget, '-p', packageName, '--edges', 'normal,build', '--prefix', 'none', '--format', '{p}'];
  if (projectRoot !== root) treeArgs.push('--manifest-path', path.join(projectRoot, 'Cargo.toml'));
  const tree = command('cargo', treeArgs, { cwd: projectRoot });
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
      files = licenseFiles(retained); sourceDirectory = retained;
      const provenance = json(path.join(root, 'third-party/licenses/packaging-sources.json')).filter((entry) => entry.file.startsWith(`rust/${item.name}-${item.version}/`));
      licenseOrigin = retainedLicenseOrigin(provenance);
    }
    const destination = `third-party/licenses/cargo/${folderKey(item.name, item.version)}`;
    const included = files.map((file) => {
      const relative = path.relative(sourceDirectory, file);
      copies.push({ source: file, destination: path.join(destination, relative) });
      return `${destination}/${portable(relative)}`;
    });
    if (!included.length) missing.push(`Cargo ${item.name} ${item.version} (${project})`);
    packages.push({ ecosystem: 'cargo', name: item.name, version: item.version, license: item.license,
      source: `https://crates.io/api/v1/crates/${item.name}/${item.version}/download`, repository: item.repository,
      license_origin: licenseOrigin, license_files: included });
  }
  return { metadata, packages, copies, missing };
}

function uniqueCopies(copies) {
  const unique = new Map();
  for (const copy of copies) {
    const existing = unique.get(copy.destination);
    if (existing && path.resolve(existing.source) !== path.resolve(copy.source)) throw new Error(`Conflicting license sources for ${copy.destination}.`);
    unique.set(copy.destination, copy);
  }
  return [...unique.values()];
}

function collectCompanion(projectRoot, platformTarget = target) {
  const metadata = cargoMetadata(projectRoot, platformTarget);
  const matches = metadata.packages.filter((item) => item.source === null && item.targets.some((target) => target.name === 'cobble-mesh-companion' && target.kind.includes('bin')));
  if (matches.length !== 1) throw new Error('Mesh project must define exactly one cobble-mesh-companion binary target.');
  const graph = collectCargoProject(projectRoot, matches[0].name, 'mesh-companion', metadata, platformTarget);
  const targetDirectory = metadata.target_directory;
  return {
    ...graph,
    executable: path.join(targetDirectory, 'release', 'cobble-mesh-companion.exe'),
    license: path.join(projectRoot, 'LICENSE'),
    notices: path.join(projectRoot, 'THIRD_PARTY_NOTICES.md'),
    licenseDestination: path.join('third-party', 'licenses', 'mesh-companion', 'LICENSE'),
    noticesDestination: path.join('third-party', 'licenses', 'mesh-companion', 'THIRD_PARTY_NOTICES.md'),
  };
}
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

export function collectDependencies(meshProject = null, platformTarget = target, includePluginHost = false) {
  const launcher = collectCargoProject(root, 'cobble-desktop', 'launcher', null, platformTarget);
  if (includePluginHost) {
    const host = collectCargoProject(root, 'cobble-plugin-host', 'plugin-host', launcher.metadata, platformTarget);
    launcher.packages = mergeCargoPackages([{ project: 'launcher', packages: launcher.packages }, { project: 'plugin-host', packages: host.packages }]);
    launcher.copies = uniqueCopies([...launcher.copies, ...host.copies]);
    launcher.missing = [...new Set([...launcher.missing, ...host.missing])];
  }
  let cargoPackages = launcher.packages;
  let copies = launcher.copies;
  let missing = launcher.missing;
  let companion = null;
  if (meshProject) {
    companion = collectCompanion(meshProject, platformTarget);
    cargoPackages = mergeCargoPackages([
      { project: 'launcher', packages: launcher.packages },
      { project: 'mesh-companion', packages: companion.packages },
    ]);
    copies = uniqueCopies([...launcher.copies, ...companion.copies]);
    missing = [...new Set([...launcher.missing, ...companion.missing])];
  }
  const packages = [...cargoPackages];
  packages.push({ ecosystem: 'vendored-c', name: 'cubiomes', version: 'e61f90580cbdd883214a8054670dacae655e59c0', license: 'MIT',
    source: 'https://github.com/Cubitect/cubiomes/tree/e61f90580cbdd883214a8054670dacae655e59c0',
    license_files: ['third-party/licenses/cubiomes-e61f905/LICENSE'] });
  if (!existsSync(path.join(root, 'third-party/licenses/cubiomes-e61f905/LICENSE'))) missing.push('Vendored Cubiomes');
  const production = JSON.parse(pnpm(['licenses', 'list', '--prod', '--json']));
  for (const item of Object.values(production).flat()) {
    for (const version of item.versions) {
      const directory = item.paths.find((directory) => {
        const manifest = json(path.join(directory, 'package.json'));
        return manifest.name === item.name && manifest.version === version;
      });
      if (!directory) throw new Error(`Cannot locate production dependency ${item.name} ${version}`);
      const destination = `third-party/licenses/npm/${folderKey(item.name, version)}`;
      const retained = path.join(root, 'third-party/licenses/npm', folderKey(item.name, version));
      const publishedFiles = licenseFiles(directory);
      const sourceDirectory = publishedFiles.length ? directory : retained;
      const included = (publishedFiles.length ? publishedFiles : existsSync(retained) ? licenseFiles(retained) : []).map((file) => {
        const relative = path.relative(sourceDirectory, file);
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
  return { metadata: launcher.metadata, packages, copies, companion };
}

// Assemble first; publication is a reversible sequence of renames, not an atomic set.
export function publishPackage(outputRoot, baseName, assemble, rename = renameSync) {
  outputRoot = path.resolve(outputRoot);
  if (!/^[a-zA-Z0-9][a-zA-Z0-9.-]*$/.test(baseName)) throw new Error('Invalid package basename.');
  const names = [baseName, baseName + '.zip', baseName + '.zip.sha256'];
  for (const name of names) {
    const current = path.join(outputRoot, name);
    if (existsSync(current) && lstatSync(current).isSymbolicLink()) throw new Error('Package artifacts must not be links.');
  }
  // Windows refuses writable opens of mapped executables. Check before renaming
  // the package so a running distribution stays at its original location.
  function checkExecutables(directory) {
    for (const item of readdirSync(directory, {withFileTypes:true})) {
      const file = path.join(directory, item.name);
      if (item.isSymbolicLink()) throw new Error('Existing package contains a link.');
      if (item.isDirectory()) checkExecutables(file);
      else if (process.platform === 'win32' && item.name.toLowerCase().endsWith('.exe')) {
        let handle; try { handle = openSync(file, 'r+'); } catch { throw new Error('Close the running package before replacing it. The previous package was retained.'); }
        finally { if (handle !== undefined) closeSync(handle); }
      }
    }
  }
  const stage = mkdtempSync(path.join(outputRoot, '.package-stage-'));
  let preserve = false;
  const previous = path.join(stage, '.previous');
  const saved = []; const published = [];
  try {
    assemble(path.join(stage, baseName), path.join(stage, baseName + '.zip'));
    for (const name of names) if (!existsSync(path.join(stage,name))) throw new Error('Package assembly did not produce every artifact.');
    if (existsSync(path.join(outputRoot,baseName))) checkExecutables(path.join(outputRoot,baseName));
    mkdirSync(previous);
    try {
      for (const name of names) if (existsSync(path.join(outputRoot,name))) { rename(path.join(outputRoot,name),path.join(previous,name)); saved.push(name); }
      for (const name of names) { rename(path.join(stage,name),path.join(outputRoot,name)); published.push(name); }
    } catch (error) {
      try {
        for (const name of published.reverse()) rename(path.join(outputRoot,name),path.join(stage,name));
        for (const name of saved.reverse()) rename(path.join(previous,name),path.join(outputRoot,name));
      } catch (rollback) { preserve = true; throw new Error('Package replacement and rollback failed; retained recovery files at ' + stage + ': ' + rollback.message); }
      throw new Error('Package replacement failed; previous package retained. Close any running package and retry: ' + error.message);
    }
  } finally { if (!preserve) rmSync(stage,{recursive:true,force:true}); }
}

function main() {
  const options = parseArguments(process.argv.slice(2));
  if (options.help) { console.log(usage); return; }
  if (process.platform !== 'win32' || process.arch !== 'x64') throw new Error('Run this packager on Windows x64.');
  if (!pnpmScript) throw new Error(`Run through the repository package script.\n${usage}`);
  const providerKey = options.embeddedCurseforgeKey ? embeddedCredential(process.env) : null;
  const meshProject = options.meshProject ? validateMeshProject(options.meshProject) : null;
  if (!options.check && !options.skipBuild) {
    pnpm(['plugins:build'], { stdio: 'inherit' });
    command('cargo', ['build', '--locked', '--release', '-p', 'cobble-plugin-host'], { stdio: 'inherit' });
    if (options.embeddedCurseforgeKey) pnpm(['build'], { stdio: 'inherit' });
    pnpm(desktopBuildArguments(options.embeddedCurseforgeKey), { stdio: 'inherit', ...(providerKey ? { env: { ...withoutProviderCredentials(process.env), COURSE_FORGE_API: providerKey } } : {}) });
    if (meshProject) command('cargo', ['build', '--locked', '--offline', '--release', '--manifest-path', path.join(meshProject, 'Cargo.toml'), '--bin', 'cobble-mesh-companion'], { cwd: meshProject, stdio: 'inherit' });
  }
  const { metadata, packages, copies, companion } = collectDependencies(meshProject);
  const desktop = metadata.packages.find((item) => item.name === 'cobble-desktop' && item.source === null);
  if (!desktop || !/^[0-9A-Za-z.+-]+$/.test(desktop.version)) throw new Error('Invalid application version.');
  const config = json(path.join(root, 'apps/desktop/src-tauri/tauri.conf.json'));
  if (config.version !== desktop.version) throw new Error('Tauri and Cargo versions must agree before packaging.');
  console.log(`License coverage: ${packages.filter((item) => item.ecosystem === 'cargo').length} Windows Cargo normal/build packages, ${packages.filter((item) => item.ecosystem === 'npm').length} production frontend packages.`);
  if (options.check) { console.log('Read-only check passed; no build or distribution created.'); return; }
  const executable = path.join(metadata.target_directory, 'release/cobble-desktop.exe');
  const bytes = readFileSync(executable);
  const pe = bytes.readUInt32LE(0x3c);
  if (bytes.toString('ascii', 0, 2) !== 'MZ' || bytes.toString('ascii', pe, pe + 4) !== 'PE\0\0' || bytes.readUInt16LE(pe + 4) !== 0x8664) throw new Error('The release executable must be a Windows x64 PE binary.');
  if (companion && (!existsSync(companion.executable) || !lstatSync(companion.executable).isFile())) throw new Error(`Mesh companion release executable is missing: ${companion.executable}`);
  const outputRoot = path.resolve(root, 'target/test-release');
  const baseName = `${desktop.version}-windows-x64${options.label ? `-${options.label}` : ''}`;
  const directory = path.resolve(outputRoot, baseName);
  const archive = directory + '.zip';
  if (path.dirname(directory) !== outputRoot || path.basename(directory) !== baseName) throw new Error('Invalid generated package path.');
  mkdirSync(outputRoot, { recursive: true });
  if (path.relative(realpathSync(root), realpathSync(outputRoot)).toLowerCase() !== path.join('target', 'test-release').toLowerCase()) throw new Error('The test-release output root must stay inside this workspace.');
  for (const output of [directory, archive, archive + '.sha256']) {
    if (path.dirname(output) !== outputRoot || ![baseName, baseName + '.zip', baseName + '.zip.sha256'].includes(path.basename(output))) throw new Error('Invalid generated artifact path.');
    if (existsSync(output) && lstatSync(output).isSymbolicLink()) throw new Error('Generated artifacts must not be links.');
  }
  publishPackage(outputRoot, baseName, (directory, archive) => {
  mkdirSync(directory, { recursive: true });
  copyFileSync(executable, path.join(directory, 'Cobble Launcher.exe'));
  copyFileSync(path.join(metadata.target_directory, 'release/cobble-plugin-host.exe'), path.join(directory, 'cobble-plugin-host.exe'));
  if (companion) {
    copyFileSync(companion.executable, path.join(directory, 'cobble-mesh-companion.exe'));
    const licensePath = path.join(directory, companion.licenseDestination);
    const noticesPath = path.join(directory, companion.noticesDestination);
    mkdirSync(path.dirname(licensePath), { recursive: true });
    copyFileSync(companion.license, licensePath);
    copyFileSync(companion.notices, noticesPath);
  }
  for (const id of officialPluginIds) {
    cpSync(path.join(metadata.target_directory, `${id}-package`), path.join(directory, `plugins/${id}`), { recursive: true });
  }
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
    scope: `Locked desktop Cargo normal and build dependency graph filtered for Windows x64, plus installed pnpm production dependencies${companion ? ' and the selected Mesh companion Cargo normal and build dependency graph' : ''}. Includes build tools and potentially tree-shaken frontend packages; not a symbol-level linked-binary inventory. System Windows libraries and the separately installed WebView2 Runtime are excluded.`,
    packages,
  }, null, 2) + '\n');
  writeFileSync(path.join(directory, 'START-HERE.txt'), `Cobble Launcher ${desktop.version} - Windows x64 test build\n\nThis is an unsigned testing build, not a stable release. Windows may show an unknown publisher warning. macOS and Linux have not been verified by this package.\n\n1. Extract the entire ZIP into a writable folder.\n2. Install Microsoft Edge WebView2 Evergreen Runtime if it is missing: https://developer.microsoft.com/microsoft-edge/webview2/\n3. Open Cobble Launcher.exe. No Node.js, Python or .NET installation is needed.\n4. Install or select a game. Cobble can prepare a matching Java runtime automatically on Windows x64; existing runtimes are reused when compatible.\n\nSettings and account credentials use your user profile; game locations are configurable. A Microsoft account is optional for local offline play.\n\nLICENSE covers Cobble's own code. Third-party licenses are in third-party/licenses; DEPENDENCIES.json lists package versions, original source URLs and retained license texts. Minecraft, Java, Mods, game data and player account credentials are not included.\n`);
  command('powershell.exe', ['-NoProfile', '-NonInteractive', '-Command', "$ErrorActionPreference = 'Stop'; Compress-Archive -LiteralPath $env:COBBLE_PACKAGE_DIR -DestinationPath $env:COBBLE_PACKAGE_ZIP -CompressionLevel Optimal"], {
    env: { ...withoutProviderCredentials(process.env), COBBLE_PACKAGE_DIR: directory, COBBLE_PACKAGE_ZIP: archive }, stdio: 'inherit',
  });
  const checksum = createHash('sha256').update(readFileSync(archive)).digest('hex');
  writeFileSync(archive + '.sha256', `${checksum}  ${path.basename(archive)}\n`);
  });
  console.log(`Test distribution: ${directory}\nZIP: ${archive}`);
}
if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  try { main(); } catch (error) { console.error(`Packaging failed: ${error.message}`); process.exitCode = 1; }
}
