// SPDX-License-Identifier: MIT
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { allowedExpression, developmentException } from './check_node_licenses.mjs';

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
