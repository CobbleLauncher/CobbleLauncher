// SPDX-License-Identifier: MIT
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { buildOfficialCatalog, OFFICIAL_PLUGIN_IDS } from './build-index.mjs';

const hash = bytes => createHash('sha256').update(bytes).digest('hex');

function fixtures(directory) {
  const packages = new Map();
  for (const id of OFFICIAL_PLUGIN_IDS) {
    const packageDir = path.join(directory, id);
    mkdirSync(packageDir, { recursive: true });
    const component = Buffer.from(`component-${id}`);
    const manifest = {
      id,
      name: id,
      version: '0.1.0',
      api_version: '1.0.0',
      publisher: 'CobbleLauncher',
      capabilities: ['viewer'],
      file_extensions: ['schem'],
      component_sha256: hash(component),
    };
    writeFileSync(path.join(packageDir, 'component.wasm'), component);
    writeFileSync(path.join(packageDir, 'plugin.json'), JSON.stringify(manifest));
    packages.set(id, packageDir);
  }
  return packages;
}

test('builds a valid official index and content-addressed component assets', t => {
  const directory = mkdtempSync(path.join(os.tmpdir(), 'official-catalog-'));
  t.after(() => rmSync(directory, { recursive: true, force: true }));
  const packages = fixtures(path.join(directory, 'packages'));
  const outputPath = path.join(directory, 'catalog', 'index.json');
  const assetDir = path.join(directory, 'catalog', 'components');
  const index = buildOfficialCatalog({
    componentBaseUrl: 'https://downloads.example/plugins',
    outputPath,
    assetDir,
    packages,
  });
  assert.equal(index.plugins.length, 5);
  for (const plugin of index.plugins) {
    const source = readFileSync(path.join(packages.get(plugin.manifest.id), 'component.wasm'));
    const destination = path.join(assetDir, plugin.manifest.id, `${plugin.manifest.component_sha256}.wasm`);
    assert.equal(plugin.component_url, `https://downloads.example/plugins/${plugin.manifest.id}/${plugin.manifest.component_sha256}.wasm`);
    assert.deepEqual(readFileSync(destination), source);
  }
  assert.deepEqual(JSON.parse(readFileSync(outputPath, 'utf8')), index);
});

test('rejects a package whose component digest does not match its manifest', t => {
  const directory = mkdtempSync(path.join(os.tmpdir(), 'official-catalog-invalid-'));
  t.after(() => rmSync(directory, { recursive: true, force: true }));
  const packages = fixtures(path.join(directory, 'packages'));
  const manifestPath = path.join(packages.get(OFFICIAL_PLUGIN_IDS[0]), 'plugin.json');
  const manifest = JSON.parse(readFileSync(manifestPath, 'utf8'));
  manifest.component_sha256 = '0'.repeat(64);
  writeFileSync(manifestPath, JSON.stringify(manifest));
  assert.throws(() => buildOfficialCatalog({
    componentBaseUrl: 'https://downloads.example/plugins',
    outputPath: path.join(directory, 'catalog', 'index.json'),
    assetDir: path.join(directory, 'catalog', 'components'),
    packages,
  }), /component hash does not match/);
});
