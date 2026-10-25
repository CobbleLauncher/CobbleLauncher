// SPDX-License-Identifier: MIT
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { readFileSync, writeFileSync, readdirSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
import { updateManifest } from './prepare-updater-release.mjs';

export function releasePlan({ repository, version, channel, installerName }) {
  if (!/^[\w.-]+\/[\w.-]+$/.test(repository ?? '')) throw new Error('A GitHub owner/repository is required.');
  if (!['preview', 'stable'].includes(channel)) throw new Error('Choose stable or preview.');
  if (!/^[\w .+-]+\.exe$/.test(installerName ?? '')) throw new Error('A Windows installer filename is required.');
  if (channel === 'stable' && version.includes('-')) throw new Error('Stable releases require a stable version.');
  const tag = `v${version}`;
  const url = `https://github.com/${repository}/releases/download/${tag}/${encodeURIComponent(installerName)}`;
  updateManifest(version, url, 'validate-only');
  return { tag, channelTag: `update-${channel}`, url, prerelease: channel === 'preview' };
}

export function publish(directory, environment = process.env, execute = spawnSync) {
  const repository = environment.GITHUB_REPOSITORY;
  const channel = environment.COBBLE_RELEASE_CHANNEL;
  const commit = environment.GITHUB_SHA;
  if (!/^[a-f0-9]{40}$/.test(commit ?? '')) throw new Error('A reviewed GitHub commit is required.');
  const version = JSON.parse(readFileSync('apps/desktop/src-tauri/tauri.conf.json', 'utf8')).version;
  const files = readdirSync(directory, { recursive: true }).filter(file => file.endsWith('.exe'));
  if (files.length !== 1) throw new Error('Expected exactly one signed NSIS installer.');
  const installer = path.resolve(directory, files[0]);
  const signature = `${installer}.sig`;
  const plan = releasePlan({ repository, version, channel, installerName: path.basename(installer) });
  const manifest = path.resolve(directory, 'latest.json');
  writeFileSync(manifest, JSON.stringify(updateManifest(version, plan.url, readFileSync(signature, 'utf8')), null, 2) + '\n');
  const checksum = `${installer}.sha256`;
  writeFileSync(checksum, `${createHash('sha256').update(readFileSync(installer)).digest('hex')}  ${path.basename(installer)}\n`);
  function gh(args, allowMissing = false) {
    const result = execute('gh', [...args, '--repo', repository], { env: environment, encoding: 'utf8', windowsHide: true });
    if (result.status !== 0) {
      if (allowMissing && /(?:HTTP 404|release not found)/i.test(result.stderr)) return null;
      throw new Error(`GitHub release operation failed: ${result.stderr?.trim() || result.error?.code || 'unknown error'}`);
    }
    return result.stdout;
  }
  const prior = gh(['release', 'view', plan.tag, '--json', 'isDraft'], true);
  if (prior && !JSON.parse(prior).isDraft) throw new Error('Version already published. Bump the version instead of replacing released files.');
  if (!prior) gh(['release', 'create', plan.tag, '--draft', '--target', commit, '--title', `Cobble Launcher ${version}`, '--notes', 'Windows x64 signed update.']);
  gh(['release', 'upload', plan.tag, installer, signature, checksum, manifest, '--clobber']);
  gh(['release', 'edit', plan.tag, '--draft=false', `--prerelease=${plan.prerelease}`, `--latest=${!plan.prerelease}`]);
  // A failed version upload must never advance the channel.
  const feed = gh(['release', 'view', plan.channelTag, '--json', 'isDraft'], true);
  if (!feed) gh(['release', 'create', plan.channelTag, '--target', commit, '--prerelease', '--latest=false', '--title', `${channel} update feed`, '--notes', 'Machine-readable launcher update channel. Download the versioned installer release to install Cobble.']);
  else if (JSON.parse(feed).isDraft) throw new Error('Channel release is a draft; resolve it before updating the feed.');
  gh(['release', 'upload', plan.channelTag, manifest, '--clobber']);
  console.log(`Published ${plan.tag}; advanced ${plan.channelTag}.`);
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  try { publish(process.argv[2] ?? 'target/signed-updater'); }
  catch (error) { console.error(error.message); process.exitCode = 1; }
}
