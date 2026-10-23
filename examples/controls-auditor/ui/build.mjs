// SPDX-License-Identifier: MIT
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

execFileSync(process.execPath, [
  fileURLToPath(new URL('../../../packages/plugin-workbench/build.mjs', import.meta.url)),
  'controls-auditor',
], { stdio: 'inherit' });
