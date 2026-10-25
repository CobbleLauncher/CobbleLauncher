// SPDX-License-Identifier: MIT
import { fileURLToPath } from 'node:url';
import { frontendNotices } from '../../../packages/plugin-workbench/notices.mjs';
export default {
  plugins: [frontendNotices()],
  root: fileURLToPath(new URL('.', import.meta.url)),
  base: './',
  build: { outDir: 'dist', emptyOutDir: true, sourcemap: false, target: ['es2022','chrome105','safari15'] },
};
