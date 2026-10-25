// SPDX-License-Identifier: MIT
import {execFileSync} from 'node:child_process';
import {fileURLToPath} from 'node:url';
const ui = fileURLToPath(new URL('./', import.meta.url));
const install = ['install', '--ignore-workspace', '--frozen-lockfile', '--ignore-scripts'];
const pnpmScript = process.env.npm_execpath;
// pnpm scripts pass their actual JS entry, avoiding Windows .cmd execution.
if (pnpmScript && /(?:^|[\\/])pnpm\.(?:cjs|mjs|js)$/.test(pnpmScript)) {
  execFileSync(process.execPath, [pnpmScript, ...install], {cwd:ui,stdio:'inherit'});
} else if (process.platform === 'win32') {
  // Only this fixed command reaches cmd.exe; no path or user input is interpolated.
  execFileSync(process.env.ComSpec || 'cmd.exe', ['/d','/s','/c','pnpm install --ignore-workspace --frozen-lockfile --ignore-scripts'], {cwd:ui,stdio:'inherit',windowsHide:true});
} else {
  execFileSync('pnpm', install, {cwd:ui,stdio:'inherit'});
}
execFileSync(process.execPath, [fileURLToPath(new URL('../../../packages/plugin-workbench/build.mjs', import.meta.url)), 'modpack-inspector'], {stdio:'inherit'});
