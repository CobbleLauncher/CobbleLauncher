// SPDX-License-Identifier: MIT
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
const root = fileURLToPath(new URL('../../../', import.meta.url));
execFileSync(process.execPath, [fileURLToPath(new URL('../../../apps/desktop/node_modules/vite/bin/vite.js', import.meta.url)), 'build', '--config', fileURLToPath(new URL('./vite.config.ts', import.meta.url))], { cwd: root, stdio: 'inherit' });
