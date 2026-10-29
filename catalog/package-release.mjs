// SPDX-License-Identifier: MIT
import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { cpSync, existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
import { officialPluginIds } from '../scripts/official-plugins.mjs';

export const digest = bytes => createHash('sha256').update(bytes).digest('hex');
export function catalogEntry(manifest, archive, base, filename) {
  const url = new URL(base);
  if (url.protocol !== 'https:' || url.username || url.password || url.search || url.hash) throw new Error('Expected an HTTPS release asset base URL');
  if (!/^[a-z0-9][a-z0-9._-]*\.zip$/.test(filename)) throw new Error('Unsafe release asset name');
  return { manifest, description: manifest.description?.['zh-CN'] || manifest.description?.en || manifest.name,
    package: { url: `${url.href.replace(/\/$/, '')}/${filename}`, sha256: digest(archive), size: archive.length },
    project_url: 'https://github.com/CobbleLauncher/CobbleLauncher' };
}

function main() {
  const [output, signing, tag = 'v0.1.0-beta.2'] = process.argv.slice(2);
  if (!output || !signing || !/^v\d+\.\d+\.\d+-beta\.\d+$/.test(tag)) throw new Error('Usage: node catalog/package-release.mjs NEW_OUTPUT SIGNING_DIRECTORY [v0.1.0-beta.2]');
  if (existsSync(output)) throw new Error('Output must be a new directory');
  const root = path.resolve(signing, '1.root.json');
  const approved = path.join(process.env.APPDATA, 'CobbleLauncher/plugin-trust/roots', `${digest(readFileSync(root))}.json`);
  if (!existsSync(approved) || !readFileSync(root).equals(readFileSync(approved))) throw new Error('Signing root must already be explicitly approved');
  const run = (exe, args) => execFileSync(exe, args, { encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] });
  const trust = path.resolve('target/debug/cobble-plugin-trust.exe');
  const sdk = path.resolve('target/debug/cobble-plugin-sdk.exe');
  const sequence = String(Math.floor(Date.now() / 1000));
  const expires = new Date(Date.now() + 365 * 86400000).toISOString();
  mkdirSync(output, { recursive: true });
  const packages = path.join(output, 'packages');
  const assets = path.join(output, 'assets');
  mkdirSync(packages); mkdirSync(assets);
  const plugins = [];
  const evidence = [];
  for (const id of officialPluginIds) {
    const source = path.resolve(`target/${id}-package`);
    const destination = path.resolve(packages, id);
    cpSync(source, destination, { recursive: true, errorOnExist: true, force: false });
    const manifest = JSON.parse(readFileSync(path.join(destination, 'plugin.json')));
    if (manifest.id !== id) throw new Error(`Unexpected identity: ${id}`);
    const checked = run(sdk, ['check', destination]).trim();
    run(trust, ['sign-package', root, path.resolve(signing, 'publisher.pk8'), path.resolve(signing, 'feed.pk8'), destination, sequence, expires]);
    const verified = JSON.parse(run(trust, ['verify-package', root, destination]));
    const filename = `${id}-${manifest.version}.zip`;
    const archivePath = path.resolve(assets, filename);
    // zipfile retains dot directories (the signature proof) and uses package-root paths.
    run('python', ['-c', 'import pathlib,sys,zipfile\np=pathlib.Path(sys.argv[1])\nwith zipfile.ZipFile(sys.argv[2],"w",zipfile.ZIP_DEFLATED) as z:\n for f in sorted(p.rglob("*")):\n  if f.is_symlink(): raise ValueError("Package links are forbidden")\n  if f.is_file(): z.write(f,f.relative_to(p).as_posix())', destination, archivePath]);
    plugins.push(catalogEntry(manifest, readFileSync(archivePath), `https://github.com/CobbleLauncher/CobbleLauncher/releases/download/${tag}`, filename));
    evidence.push({ id, checked, verified });
  }
  writeFileSync(path.join(output, 'index.json'), JSON.stringify({ schema_version: 1, name: 'CobbleLauncher Official', plugins }, null, 2) + '\n');
  const catalogCheck = JSON.parse(run(sdk, ['catalog-check', path.resolve(output, 'index.json'), '--json']));
  cpSync(root, path.join(assets, '1.root.json'));
  writeFileSync(path.join(output, 'verification.json'), JSON.stringify({ tag, sequence, expires, root_sha256: digest(readFileSync(root)), catalogCheck, packages: evidence }, null, 2) + '\n');
  console.log(JSON.stringify({ output, packages: plugins.length, catalogCheck }));
}
if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) main();
