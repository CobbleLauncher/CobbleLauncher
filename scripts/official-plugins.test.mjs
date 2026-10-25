// SPDX-License-Identifier: MIT
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { existsSync, readFileSync } from 'node:fs';
import { officialPlugins, officialPluginIds } from './official-plugins.mjs';

test('packaged optional plugins remain discoverable through the native allowlist', () => {
  const source = readFileSync(new URL('../apps/desktop/src-tauri/src/bundled_plugins.rs', import.meta.url), 'utf8');
  const match = /const BUNDLED_IDS: \[&str; (\d+)\] = \[([\s\S]*?)\];/.exec(source);
  assert.ok(match, 'native bundled allowlist declaration');
  const native = [...match[2].matchAll(/"([a-z0-9-]+)"/g)].map(value => value[1]);
  assert.equal(native.length, Number(match[1]), 'native array length');
  assert.equal(new Set(native).size, native.length, 'native duplicate IDs');
  assert.equal(new Set(officialPluginIds).size, officialPluginIds.length, 'package duplicate IDs');
  assert.deepEqual(native.toSorted(), officialPluginIds.toSorted());
});

test('each official package maps to its actual workspace crate and manifest', () => {
  const workspace = readFileSync(new URL('../Cargo.toml', import.meta.url), 'utf8');
  const members = /members\s*=\s*\[([\s\S]*?)\]/.exec(workspace);
  assert.ok(members, 'workspace members');
  const crates = new Map([...members[1].matchAll(/"([^"]+)"/g)].map(([, path]) => {
    const cargo = readFileSync(new URL(`../${path}/Cargo.toml`, import.meta.url), 'utf8');
    const name = /^name\s*=\s*"([^"]+)"/m.exec(cargo)?.[1];
    assert.ok(name, `package name for ${path}`);
    return [name, path];
  }));
  for (const [id, crate] of officialPlugins) {
    assert.ok(crates.has(crate), `${id} crate is a workspace member`);
    assert.ok(existsSync(new URL(`../${crates.get(crate)}/src/lib.rs`, import.meta.url)), `${id} guest library exists`);
    const manifest = JSON.parse(readFileSync(new URL(`../examples/${id}/plugin.json`, import.meta.url), 'utf8'));
    assert.equal(manifest.id, id, `${id} manifest identity`);
  }
});
