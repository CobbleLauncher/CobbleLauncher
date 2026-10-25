// SPDX-License-Identifier: MIT
// Approved local experience package: read only the CurseForge credential, never print it.
import { readFileSync } from 'node:fs';
import { spawn } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { parseLocalCredential, runtimeEnvironment } from './launch-desktop-local.mjs';
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');

export function localPackageArguments(args) {
  if (args.some(arg => ['--skip-build', '--check', '--embedded-curseforge-key'].includes(arg))) throw new Error('Local experience packages always use a fresh credential-enabled build.');
  return ['package:windows', '--embedded-curseforge-key', ...args];
}

export function main(args = process.argv.slice(2)) {
  if (args.length === 1 && args[0] === '--help') {
    console.log('pnpm package:local [--mesh-project <absolute-path>] [--label <name>]\nBuild the local Windows experience package using COURSE_FORGE_API from ../.secret.env. The key is embedded only in the desktop binary, never copied as an environment file.');
    return;
  }
  const commandArgs = localPackageArguments(args);
  if (!process.env.npm_execpath) throw new Error('Run through pnpm package:local.');
  let key;
  try { key = parseLocalCredential(readFileSync(path.resolve(root, '..', '.secret.env'), 'utf8')); }
  catch { throw new Error('Could not load COURSE_FORGE_API from the local .secret.env file.'); }
  const child = spawn(process.execPath, [process.env.npm_execpath, ...commandArgs], {
    cwd: root, env: runtimeEnvironment(process.env, key), stdio: 'inherit', shell: false, windowsHide: true,
  });
  child.once('error', () => { console.error('Could not start the local package build.'); process.exitCode = 1; });
  child.once('exit', code => { process.exitCode = code ?? 1; });
}
if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  try { main(); } catch (error) { console.error(error.message); process.exitCode = 1; }
}
