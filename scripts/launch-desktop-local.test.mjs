// SPDX-License-Identifier: MIT
// Synthetic strings only; these tests never read the private local environment file.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { parseLocalCredential, runtimeEnvironment, main } from './launch-desktop-local.mjs';
import { desktopBuildArguments, embeddedCredential, parseArguments, withoutProviderCredentials } from './package-windows.mjs';
import { localPackageArguments } from './package-local-windows.mjs';

test('local experience packaging always opts into embedding on a fresh build', () => {
  assert.deepEqual(localPackageArguments(['--label', 'preview']), ['package:windows', '--embedded-curseforge-key', '--label', 'preview']);
  assert.throws(() => localPackageArguments(['--skip-build']), /fresh/);
});
test('loads only the exact provider assignment without executing shell syntax or expanding dollars', () => {
  assert.equal(parseLocalCredential('MCS_API=ignored\nCOURSE_FORGE_API="synthetic$literal-key"\nOTHER=ignored'), 'synthetic$literal-key');
  assert.equal(parseLocalCredential("export COURSE_FORGE_API='synthetic$(literal)-key'"), 'synthetic$(literal)-key');
  assert.throws(() => parseLocalCredential('MCS_API=ignored'), /exactly once/);
  assert.throws(() => parseLocalCredential('COURSE_FORGE_API=one\nCOURSE_FORGE_API=two'), /exactly once/);
  for (const text of ['COURSE_FORGE_API=', 'COURSE_FORGE_API="unclosed', 'COURSE_FORGE_API=value with spaces']) assert.throws(() => parseLocalCredential(text), /invalid/);
});
test('runtime launch excludes MCS values and never accepts release flags or executable overrides', () => {
  assert.deepEqual(runtimeEnvironment({ PATH: 'synthetic-path', COURSE_FORGE_API: 'inherited', MCS_API: 'ignored', mcs_release_token: 'ignored' }, 'synthetic-runtime'), { PATH: 'synthetic-path', COURSE_FORGE_API: 'synthetic-runtime' });
  assert.throws(() => main(['--release']), /does not accept build flags/);
  assert.throws(() => main(['--executable', 'other']), /does not accept build flags/);
});
test('embedding is explicit, requires a fresh build, and puts the feature before Cargo arguments', () => {
  assert.equal(parseArguments([]).embeddedCurseforgeKey, false);
  assert.equal(parseArguments(['--embedded-curseforge-key']).embeddedCurseforgeKey, true);
  for (const mode of ['--check', '--skip-build']) assert.throws(() => parseArguments([mode, '--embedded-curseforge-key']), /fresh build/);
  assert.deepEqual(desktopBuildArguments(false), ['desktop:build']);
  const args = desktopBuildArguments(true);
  assert.deepEqual(args.slice(args.indexOf('--features'), args.indexOf('--features') + 2), ['--features', 'embedded-curseforge-key']);
  assert.ok(args.indexOf('--features') < args.indexOf('--'));
  assert.deepEqual(JSON.parse(args[args.indexOf('--config') + 1]), { build: { beforeBuildCommand: null } });
  assert.deepEqual(args.slice(-2), ['--', '--locked']);
  assert.throws(() => embeddedCredential({}), /requires a valid COURSE_FORGE_API/);
  assert.equal(embeddedCredential({ COURSE_FORGE_API: 'synthetic-release-key' }), 'synthetic-release-key');
});
test('non-desktop build helpers receive no provider or MCS environment credentials', () => {
  assert.deepEqual(withoutProviderCredentials({ PATH: 'synthetic-path', COURSE_FORGE_API: 'synthetic-key', MCS_API: 'ignored' }), { PATH: 'synthetic-path' });
});
test('manual workflow exposes the one secret only to explicit embedded packaging and uploads only package outputs', () => {
  const source = readFileSync(new URL('../.github/workflows/windows-package.yml', import.meta.url), 'utf8');
  assert.match(source, /on:\s+workflow_dispatch:/);
  assert.doesNotMatch(source, /pull_request:|push:/);
  assert.match(source, /github\.event\.repository\.default_branch/);
  assert.equal((source.match(/secrets\./g) ?? []).length, 1);
  assert.match(source, /COURSE_FORGE_API: \$\{\{ secrets\.COURSE_FORGE_API \}\}/);
  assert.match(source, /run: pnpm package:windows --embedded-curseforge-key/);
  assert.match(source, /target\/test-release\/\*\.zip\r?\n\s+target\/test-release\/\*\.zip\.sha256/);
  assert.doesNotMatch(source, /MCS|\.secret\.env|env:.*VITE_/);
});
