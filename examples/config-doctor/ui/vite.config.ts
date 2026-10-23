// SPDX-License-Identifier: MIT
import { fileURLToPath } from 'node:url';
export default {
  root: fileURLToPath(new URL('.', import.meta.url)),
  base: './',
  build: { outDir: 'dist', emptyOutDir: true, sourcemap: false, target: ['es2022','chrome105','safari15'] },
};
