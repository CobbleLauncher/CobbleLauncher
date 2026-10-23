// SPDX-License-Identifier: MIT
// Launches only an already-built debug desktop. Never starts Cargo or a release build.
import { spawn } from 'node:child_process';
import { existsSync, readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const localEnvironmentFile = path.resolve(root, '..', '.secret.env');
export function parseLocalCredential(text) {
  const matches = text.split(/\r?\n/).filter(line => /^\s*(?:export\s+)?COURSE_FORGE_API\s*=/.test(line));
  if (matches.length !== 1) throw new Error('The local file must define COURSE_FORGE_API exactly once.');
  let value = matches[0].slice(matches[0].indexOf('=') + 1).trim();
  if (value.startsWith('"') || value.startsWith("'")) {
    const quote = value[0];
    if (!value.endsWith(quote) || value.length < 2) throw new Error('The local COURSE_FORGE_API value is invalid.');
    value = value.slice(1, -1);
  }
  if (!value || value.length > 4096 || !/^[\x21-\x7e]+$/.test(value)) throw new Error('The local COURSE_FORGE_API value is invalid.');
  return value;
}
export function runtimeEnvironment(environment, credential) {
  const safe = Object.fromEntries(Object.entries(environment).filter(([name]) => name.toUpperCase() !== 'COURSE_FORGE_API' && !name.toUpperCase().startsWith('MCS')));
  return { ...safe, COURSE_FORGE_API: credential };
}
export function main(args = process.argv.slice(2)) {
  if (args.length === 1 && args[0] === '--help') {
    console.log('node scripts/launch-desktop-local.mjs\nLaunch an existing target/debug/cobble-desktop executable with the local COURSE_FORGE_API at runtime only. Build debug separately without embedded-curseforge-key first. Keep pnpm dev running if that debug build uses the development server.');
    return;
  }
  if (args.length) throw new Error('This local debug launcher does not accept build flags or executable overrides.');
  const executable = path.join(root, 'target', 'debug', process.platform === 'win32' ? 'cobble-desktop.exe' : 'cobble-desktop');
  if (!existsSync(executable)) throw new Error('Debug desktop executable not found. Build it separately without embedded-curseforge-key, then run this launcher again.');
  let credential;
  try { credential = parseLocalCredential(readFileSync(localEnvironmentFile, 'utf8')); }
  catch { throw new Error('Could not load a valid COURSE_FORGE_API from the configured local .secret.env file.'); }
  const child = spawn(executable, [], { cwd: root, env: runtimeEnvironment(process.env, credential), stdio: 'inherit', shell: false, windowsHide: true });
  child.once('error', () => { console.error('Could not launch the debug desktop executable.'); process.exitCode = 1; });
  child.once('exit', code => { process.exitCode = code ?? 1; });
}
if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  try { main(); } catch (error) { console.error(error.message); process.exitCode = 1; }
}
