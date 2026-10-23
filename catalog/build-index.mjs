// SPDX-License-Identifier: MIT
import { createHash } from 'node:crypto';
import { copyFileSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

export const OFFICIAL_PLUGIN_IDS = Object.freeze([
  'schematic-viewer',
  'crash-doctor',
  'modpack-inspector',
  'mcs-provider',
  'session-card',
]);

const sha256 = bytes => createHash('sha256').update(bytes).digest('hex');
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');

function parseArgs(argv) {
  const options = { packages: new Map() };
  for (let i = 0; i < argv.length; i++) {
    const flag = argv[i];
    const value = argv[++i];
    if (!value) throw new Error(`Missing value for ${flag}`);
    if (flag === '--component-base-url' || flag === '--output' || flag === '--asset-dir') {
      if (options[flag] !== undefined) throw new Error(`Duplicate option: ${flag}`);
      options[flag] = value;
    } else if (flag === '--package') {
      const separator = value.indexOf('=');
      if (separator <= 0 || separator === value.length - 1) throw new Error('Package must be ID=PATH');
      const id = value.slice(0, separator);
      if (options.packages.has(id)) throw new Error(`Duplicate package ID: ${id}`);
      options.packages.set(id, value.slice(separator + 1));
    } else {
      throw new Error(`Unknown option: ${flag}`);
    }
  }
  if (!options['--component-base-url'] || !options['--output'] || !options['--asset-dir']) {
    throw new Error('Usage: node catalog/build-index.mjs --component-base-url HTTPS_URL --output INDEX_JSON --asset-dir COMPONENT_DIR --package ID=PACKAGE_DIR ...');
  }
  const base = new URL(options['--component-base-url']);
  if (base.protocol !== 'https:' || base.username || base.password || base.search || base.hash) {
    throw new Error('--component-base-url must be an HTTPS base URL without credentials, query, or fragment');
  }
  const expected = [...OFFICIAL_PLUGIN_IDS].sort();
  const actual = [...options.packages.keys()].sort();
  if (JSON.stringify(actual) !== JSON.stringify(expected)) {
    throw new Error(`Exactly these packages are required: ${OFFICIAL_PLUGIN_IDS.join(', ')}`);
  }
  options.base = base.href.replace(/\/$/, '');
  return options;
}

function makeCatalog(options) {
  const plugins = [];
  const assets = [];
  for (const id of OFFICIAL_PLUGIN_IDS) {
    const packageDir = path.resolve(root, options.packages.get(id));
    const manifest = JSON.parse(readFileSync(path.join(packageDir, 'plugin.json'), 'utf8'));
    if (manifest.id !== id) throw new Error(`${id}: package manifest ID does not match`);
    const componentPath = path.join(packageDir, 'component.wasm');
    const component = readFileSync(componentPath);
    const digest = sha256(component);
    if (manifest.component_sha256 !== digest) throw new Error(`${id}: component hash does not match plugin.json`);
    const componentName = `${digest}.wasm`;
    const assetRelativePath = path.join(id, componentName);
    const assetPath = path.join(path.resolve(root, options['--asset-dir']), assetRelativePath);
    assets.push({ source: componentPath, destination: assetPath });

    let icon_png_base64;
    if (manifest.icon) {
      if (manifest.icon.file !== 'icon.png') throw new Error(`${id}: unsupported icon path`);
      const icon = readFileSync(path.join(packageDir, manifest.icon.file));
      if (sha256(icon) !== manifest.icon.sha256) throw new Error(`${id}: icon hash does not match plugin.json`);
      if (icon.length > 32768) throw new Error(`${id}: icon exceeds catalog limit`);
      icon_png_base64 = icon.toString('base64');
    }

    const description = typeof manifest.description === 'string'
      ? manifest.description
      : manifest.description?.en ?? '';
    const entry = {
      manifest,
      description,
      component_url: `${options.base}/${id}/${componentName}`,
      project_url: `https://github.com/CobbleLauncher/CobbleLauncher/tree/main/examples/${id}`,
    };
    if (icon_png_base64) entry.icon_png_base64 = icon_png_base64;
    plugins.push(entry);
  }
  return { index: { schema_version: 1, name: 'CobbleLauncher Official', plugins }, assets };
}

export function buildOfficialCatalog({ componentBaseUrl, outputPath, assetDir, packages }) {
  const options = parseArgs([
    '--component-base-url', componentBaseUrl,
    '--output', outputPath,
    '--asset-dir', assetDir,
    ...[...packages].flatMap(([id, packageDir]) => ['--package', `${id}=${packageDir}`]),
  ]);
  const { index, assets } = makeCatalog(options);
  for (const asset of assets) {
    mkdirSync(path.dirname(asset.destination), { recursive: true });
    copyFileSync(asset.source, asset.destination);
  }
  const output = path.resolve(root, options['--output']);
  mkdirSync(path.dirname(output), { recursive: true });
  writeFileSync(output, `${JSON.stringify(index, null, 2)}\n`);
  return index;
}

function main(argv) {
  const options = parseArgs(argv);
  const { index, assets } = makeCatalog(options);
  for (const asset of assets) {
    mkdirSync(path.dirname(asset.destination), { recursive: true });
    copyFileSync(asset.source, asset.destination);
  }
  const output = path.resolve(root, options['--output']);
  mkdirSync(path.dirname(output), { recursive: true });
  writeFileSync(output, `${JSON.stringify(index, null, 2)}\n`);
  console.log(`Wrote ${index.plugins.length} official plugin entries to ${output}`);
}

if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) {
  try { main(process.argv.slice(2)); }
  catch (error) { console.error(error instanceof Error ? error.message : String(error)); process.exitCode = 1; }
}
