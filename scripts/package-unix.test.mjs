// SPDX-License-Identifier: MIT
import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, mkdirSync, writeFileSync, readFileSync, existsSync, statSync, rmSync, unlinkSync } from 'node:fs';
import { createHash } from 'node:crypto';
import os from 'node:os';
import path from 'node:path';
import { assembleUnixPackage, parseUnixArguments, unixDesktopBuildEnvironment, unixTarget, validateUnixBinary } from './package-unix.mjs';
import { desktopBuildArguments, embeddedCredential } from './package-windows.mjs';
function fixture(t, platform = 'linux', arch = 'x64') {
  const root = mkdtempSync(path.join(os.tmpdir(), 'cobble-package-test-'));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const repository = path.join(root, 'repo'); const binaryDir = path.join(root, 'bin'); const pluginsDir = path.join(root, 'plugins');
  for (const directory of [repository, binaryDir, path.join(repository, 'third-party/licenses'), path.join(pluginsDir, 'fixture-package')]) mkdirSync(directory, { recursive: true });
  mkdirSync(path.join(repository, 'apps/desktop/src-tauri/icons'), { recursive: true });
  for (const file of ['LICENSE', 'THIRD_PARTY_NOTICES.md', 'third-party/licenses/fixture.txt', 'apps/desktop/src-tauri/icons/icon.icns', 'apps/desktop/src-tauri/icons/128x128.png']) writeFileSync(path.join(repository, file), 'Fixture license');
  const binary = Buffer.alloc(64);
  if (platform === 'linux') { binary.set([127, 69, 76, 70, 2, 1]); binary.writeUInt16LE(3, 16); binary.writeUInt16LE(arch === 'x64' ? 62 : 183, 18); }
  else { binary.writeUInt32LE(0xfeedfacf, 0); binary.writeUInt32LE(2, 12); binary.writeUInt32LE(arch === 'x64' ? 0x1000007 : 0x100000c, 4); }
  for (const name of ['cobble-desktop', 'cobble-plugin-host', 'cobble-mesh-companion']) writeFileSync(path.join(binaryDir, name), binary);
  writeFileSync(path.join(pluginsDir, 'fixture-package/plugin.json'), JSON.stringify({ id: 'fixture' }));
  writeFileSync(path.join(pluginsDir, 'fixture-package/component.wasm'), Buffer.from([0, 97, 115, 109]));
  return { repository, binaryDir, pluginsDir, directory: path.join(root, 'package'), platform, arch, version: '0.1.0-beta.1', pluginIds: ['fixture'], inventory: { copies: [], packages: [{ name: 'fixture', license_files: ['third-party/licenses/fixture.txt'] }] } };
}
test('CLI bounds target and companion provenance options', () => {
  assert.equal(unixTarget('linux', 'x64'), 'x86_64-unknown-linux-gnu');
  assert.equal(unixTarget('darwin', 'arm64'), 'aarch64-apple-darwin');
  assert.throws(() => unixTarget('win32', 'x64'), /Supported/);
  assert.throws(() => parseUnixArguments(['--platform', 'linux', '--arch', 'x64', '--mesh-binary', '/tmp/helper']), /together/);
  assert.throws(() => parseUnixArguments(['--skip-build', '--skip-build']), /Duplicate/);
});
test('embedded CurseForge key requires a fresh opt-in build and is scoped to the desktop compiler', () => {
  assert.equal(parseUnixArguments(['--platform', 'linux', '--embedded-curseforge-key']).embeddedCurseforgeKey, true);
  assert.throws(() => parseUnixArguments(['--platform', 'linux', '--skip-build', '--embedded-curseforge-key']), /fresh build/);
  assert.throws(() => embeddedCredential({}), /valid COURSE_FORGE_API/);
  assert.deepEqual(unixDesktopBuildEnvironment({ COURSE_FORGE_API: 'credential', MCS_TEST_KEY: 'other', KEEP: 'yes' }, 'credential'), { KEEP: 'yes', COURSE_FORGE_API: 'credential' });
  assert.deepEqual(unixDesktopBuildEnvironment({ COURSE_FORGE_API: 'credential', MCS_TEST_KEY: 'other', KEEP: 'yes' }, null), { KEEP: 'yes' });
  const args = desktopBuildArguments(true);
  assert.ok(args.includes('embedded-curseforge-key'));
  assert.ok(args.includes('--config'));
  assert.equal(args.includes('MCS_TEST_KEY'), false);
});
test('ad-hoc signing is an explicit macOS-only package option', () => {
  assert.equal(parseUnixArguments(['--platform', 'darwin', '--arch', 'arm64', '--adhoc-sign']).adhocSign, true);
  assert.throws(() => parseUnixArguments(['--platform', 'linux', '--arch', 'x64', '--adhoc-sign']), /only for macOS/);
});
for (const platform of ['linux', 'darwin']) for (const arch of ['x64', 'arm64']) test(`${platform}/${arch} layout retains helpers, plugins, licenses and inventory`, t => {
  const options = fixture(t, platform, arch); const result = assembleUnixPackage(options);
  assert.equal(result.resources, platform === 'darwin' ? path.join(options.directory, 'Cobble Launcher.app/Contents/Resources') : options.directory);
  assert.ok(existsSync(path.join(result.executables, 'cobble-plugin-host')));
  assert.ok(existsSync(path.join(result.resources, 'plugins/fixture/plugin.json')));
  const dependencies = JSON.parse(readFileSync(path.join(result.resources, 'DEPENDENCIES.json')));
  assert.equal(dependencies.target, unixTarget(platform, arch));
  assert.match(readFileSync(path.join(options.directory, 'README-INTERNAL.txt'), 'utf8'), /host-rendered|isolated custom plugin frontends/i);
  assert.ok(existsSync(path.join(result.resources, platform === 'darwin' ? 'icon.icns' : 'cobble-launcher.png')));
  const inventory = JSON.parse(readFileSync(path.join(options.directory, 'PACKAGE-INVENTORY.json')));
  assert.equal(inventory.signing, platform === 'darwin' ? 'unsigned' : 'not-applicable');
  if (platform === 'darwin') assert.equal(existsSync(path.join(result.resources, 'PACKAGE-INVENTORY.json')), false);
  assert.ok(inventory.files.some(item => item.path.endsWith('cobble-plugin-host') && item.sha256.length === 64));
  if (process.platform !== 'win32') assert.equal(statSync(path.join(result.executables, 'cobble-plugin-host')).mode & 0o777, 0o755);
  if (platform === 'darwin') assert.match(readFileSync(path.join(options.directory, 'Cobble Launcher.app/Contents/Info.plist'), 'utf8'), /CFBundleExecutable/);
  else assert.match(readFileSync(path.join(options.directory, 'cobble-launcher.desktop'), 'utf8'), /Exec=cobble-desktop/);
});
test('macOS app is fully signed before the package-root inventory hashes final bundle bytes', t => {
  const options = fixture(t, 'darwin', 'arm64');
  const calls = [];
  const codeSign = args => {
    const target = args.at(-1);
    if (args[0] === '--verify') { calls.push('verify'); return; }
    if (target.endsWith('cobble-desktop')) {
      const helper = path.join(path.dirname(target), 'cobble-plugin-host');
      assert.match(readFileSync(helper).toString(), /executable-signature/);
    }
    if (target.endsWith('.app')) {
      calls.push('app');
      const main = path.join(target, 'Contents/MacOS/cobble-desktop');
      writeFileSync(main, Buffer.concat([readFileSync(main), Buffer.from('app-signature-final-bytes')]));
      const signature = path.join(target, 'Contents/_CodeSignature');
      mkdirSync(signature, { recursive: true });
      writeFileSync(path.join(signature, 'CodeResources'), 'sealed bundle');
      return;
    }
    calls.push(path.basename(target));
    writeFileSync(target, Buffer.concat([readFileSync(target), Buffer.from('executable-signature')]));
  };
  assembleUnixPackage({ ...options, adhocSign: true, codeSign });
  assert.deepEqual(calls, ['cobble-plugin-host', 'cobble-desktop', 'app', 'verify']);
  const inventory = JSON.parse(readFileSync(path.join(options.directory, 'PACKAGE-INVENTORY.json')));
  assert.equal(inventory.signing, 'ad-hoc');
  assert.equal(inventory.notarized, false);
  const relative = 'Cobble Launcher.app/Contents/MacOS/cobble-desktop';
  const finalBinary = readFileSync(path.join(options.directory, relative));
  assert.equal(inventory.files.find(item => item.path === relative)?.sha256, createHash('sha256').update(finalBinary).digest('hex'));
  assert.ok(inventory.files.some(item => item.path === 'Cobble Launcher.app/Contents/_CodeSignature/CodeResources'));
  assert.equal(existsSync(path.join(options.directory, 'Cobble Launcher.app/Contents/Resources/PACKAGE-INVENTORY.json')), false);
});
test('missing host and wrong architecture fail closed', t => {
  const options = fixture(t);
  assert.throws(() => validateUnixBinary(path.join(options.binaryDir, 'cobble-desktop'), 'linux', 'arm64'), /Expected/);
  unlinkSync(path.join(options.binaryDir, 'cobble-plugin-host'));
  assert.throws(() => assembleUnixPackage(options), /ENOENT/);
});
test('missing required license and inventory traversal fail closed', t => {
  const options = fixture(t); options.inventory.packages[0].license_files = ['third-party/licenses/missing'];
  assert.throws(() => assembleUnixPackage(options), /ENOENT/);
  options.inventory.packages[0].license_files = ['../outside'];
  assert.throws(() => assembleUnixPackage(options), /Unsafe/);
});
test('optional companion requires inventory and retains its source notices', t => {
  const options = fixture(t); options.meshBinary = path.join(options.binaryDir, 'cobble-mesh-companion');
  assert.throws(() => assembleUnixPackage(options), /Missing companion/);
  options.inventory.companion = { license: path.join(options.repository, 'LICENSE'), notices: path.join(options.repository, 'THIRD_PARTY_NOTICES.md'), licenseDestination: 'third-party/licenses/mesh/LICENSE', noticesDestination: 'third-party/licenses/mesh/THIRD_PARTY_NOTICES.md' };
  const result = assembleUnixPackage(options);
  assert.ok(existsSync(path.join(result.executables, 'cobble-mesh-companion')));
  assert.ok(existsSync(path.join(result.resources, options.inventory.companion.noticesDestination)));
});

test('plugin identity must match the selected official directory', t => {
  const options = fixture(t);
  writeFileSync(path.join(options.pluginsDir, 'fixture-package/plugin.json'), JSON.stringify({ id: 'unexpected' }));
  assert.throws(() => assembleUnixPackage(options), /Invalid bundled plugin/);
});
