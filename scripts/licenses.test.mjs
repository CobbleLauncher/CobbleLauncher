// SPDX-License-Identifier: MIT
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { allowedExpression, developmentException } from './check_node_licenses.mjs';
import { mergeCargoPackages, parseArguments, retainedLicenseOrigin } from './package-windows.mjs';

const allowed = new Set(['MIT', 'Apache-2.0', 'Apache-2.0 WITH LLVM-exception']);
test('OR permits a reviewed alternative while AND requires every obligation', () => {
  assert.equal(allowedExpression('(MIT OR GPL-3.0-only)', allowed), true);
  assert.equal(allowedExpression('(MIT AND GPL-3.0-only)', allowed), false);
  assert.equal(allowedExpression('(MIT AND Apache-2.0)', allowed), true);
});
test('unknown, malformed, missing and custom licenses require review', () => {
  for (const expression of [null, '', 'NOASSERTION', 'MIT-ish', 'LicenseRef-Unknown', 'MIT OR']) {
    assert.equal(allowedExpression(expression, allowed), false);
  }
});
test('exceptions and plus expressions cannot borrow a base-license allowance', () => {
  assert.equal(allowedExpression('Apache-2.0 WITH LLVM-exception', allowed), true);
  assert.equal(allowedExpression('GPL-2.0-only WITH Classpath-exception-2.0', allowed), false);
  assert.equal(allowedExpression('GPL-2.0+', allowed), false);
});
test('build-only exceptions fail when a package moves into the production graph', () => {
  const entry = { name: 'lightningcss-win32-x64-msvc', version: '1.33.0', license: 'MPL-2.0' };
  const rule = { name: 'lightningcss-*', version: '1.33.0', license: 'MPL-2.0' };
  assert.equal(developmentException(entry, new Set(), [rule]), rule);
  assert.equal(developmentException(entry, new Set([`${entry.name}@${entry.version}`]), [rule]), null);
  assert.equal(developmentException({ ...entry, version: '1.34.0' }, new Set(), [rule]), null);
  assert.equal(developmentException({ ...entry, license: 'GPL-3.0-only' }, new Set(), [rule]), null);
});

test('retained canonical license text is distinguished from original upstream files', () => {
  assert.match(retainedLicenseOrigin([{ provenance: 'canonical-license', license: 'Apache-2.0' }]), /^Canonical Apache-2\.0 text selected under upstream grant;/);
  assert.equal(retainedLicenseOrigin([{ provenance: 'upstream-original' }]), 'Retained upstream license; see third-party/licenses/packaging-sources.json and THIRD_PARTY_NOTICES.md');
});

test('canonical Apache texts have pinned crate grants and official text provenance', () => {
  const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
  const sources = JSON.parse(readFileSync(path.join(root, 'third-party/licenses/packaging-sources.json'), 'utf8'));
  const expected = [
    ['aes256ctr_poly1305aes-0.2.1', '142ef9cb9691ed378f78ee0c07f3aec3cf713e41'],
    ['pariter-0.6.0', '238cf7bc205668be2ade69d6f742664dbdfba375'],
  ];
  for (const [packageName, revision] of expected) {
    const entry = sources.find((candidate) => candidate.package === packageName);
    assert.equal(entry.provenance, 'canonical-license');
    assert.equal(entry.license, 'Apache-2.0');
    assert.equal(entry.source, 'https://www.apache.org/licenses/LICENSE-2.0.txt');
    assert.ok(entry.grant_sources.some((source) => source.includes(revision)));
    const licensePath = path.join(root, 'third-party/licenses', entry.file.replaceAll('/', path.sep));
    assert.match(readFileSync(licensePath, 'utf8'), /^\s*Apache License\s+Version 2\.0, January 2004/);
  }
});

test('Mesh packaging is opt-in and requires an absolute repository path', () => {
  assert.deepEqual(parseArguments([]), { check: false, skipBuild: false, help: false, meshProject: null, embeddedCurseforgeKey: false });
  assert.deepEqual(parseArguments(['--check', '--mesh-project', path.resolve('companion repo')]), {
    check: true, skipBuild: false, help: false, meshProject: path.resolve('companion repo'), embeddedCurseforgeKey: false,
  });
  assert.throws(() => parseArguments(['--mesh-project', 'relative/path']), /absolute repository path/);
  assert.throws(() => parseArguments(['--mesh-project']), /requires an absolute repository path/);
  assert.throws(() => parseArguments(['--check', '--skip-build']), /Choose --check or --skip-build/);
});

test('Mesh and launcher Cargo package entries deduplicate by name and version', () => {
  const entry = {
    ecosystem: 'cargo', name: 'shared', version: '1.2.3', license: 'MIT', source: 'registry', repository: 'repo',
    license_origin: 'Published crate archive', license_files: ['third-party/licenses/cargo/shared-1.2.3/LICENSE'],
  };
  const merged = mergeCargoPackages([
    { project: 'launcher', packages: [entry] },
    { project: 'mesh-companion', packages: [{ ...entry }] },
  ]);
  assert.equal(merged.length, 1);
  assert.deepEqual(merged[0].source_projects, ['launcher', 'mesh-companion']);
});
