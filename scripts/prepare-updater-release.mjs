// SPDX-License-Identifier: MIT
// Prepare a Tauri NSIS overlay from a complete, license-checked portable staging tree.
import { existsSync, lstatSync, readdirSync, readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

export function installerConfiguration(stage, publicKey) {
  if (!path.isAbsolute(stage) || !publicKey?.trim()) throw new Error('Absolute complete staging directory and updater public key required.');
  for (const name of ['cobble-plugin-host.exe', 'plugins', 'LICENSE', 'THIRD_PARTY_NOTICES.md', 'DEPENDENCIES.json', 'third-party/licenses']) {
    if (!existsSync(path.join(stage, name))) throw new Error(`Incomplete staging tree: ${name}`);
  }
  const resources = {};
  function visit(directory, relative = '') {
    for (const entry of readdirSync(directory)) {
      const name = path.join(relative, entry);
      const absolute = path.join(stage, name);
      const info = lstatSync(absolute);
      if (info.isSymbolicLink()) throw new Error('Symlinks are not allowed in release staging.');
      if (info.isDirectory()) visit(absolute, name);
      else if (info.isFile() && !['Cobble Launcher.exe', 'START-HERE.txt'].includes(name)) resources[absolute] = name.replaceAll('\\', '/');
      else if (!info.isFile()) throw new Error('Only regular release resources are supported.');
    }
  }
  visit(stage);
  return { bundle: { active: true, targets: ['nsis'], createUpdaterArtifacts: true, resources }, plugins: { updater: { pubkey: publicKey.trim(), requireSignedVersion: true, windows: { installMode: 'passive' } } } };
}

export function updateManifest(version, installerUrl, signature) {
  if (!/^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/.test(version)) throw new Error('A release SemVer is required.');
  const url = new URL(installerUrl);
  if (url.protocol !== 'https:' || url.username || url.password) throw new Error('Public HTTPS installer URL required.');
  if (!signature.trim() || signature.length > 8192) throw new Error('Tauri installer signature required.');
  return { version, platforms: { 'windows-x86_64': { url: url.href, signature: signature.trim() } } };
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const [mode, ...args] = process.argv.slice(2);
  let result;
  let output;
  if (mode === 'config' && args.length === 2) {
    result = installerConfiguration(path.resolve(args[0]), process.env.COBBLE_UPDATER_PUBLIC_KEY);
    output = args[1];
  } else if (mode === 'manifest' && args.length === 4) {
    result = updateManifest(args[0], args[1], readFileSync(args[2], 'utf8'));
    output = args[3];
  } else throw new Error('Usage: config <stage> <output> | manifest <version> <https-installer-url> <sig-file> <output>');
  writeFileSync(output, JSON.stringify(result, null, 2) + '\n');
}
