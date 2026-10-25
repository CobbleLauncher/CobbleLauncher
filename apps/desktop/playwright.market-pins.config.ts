// SPDX-License-Identifier: MIT
import { defineConfig } from '@playwright/test';
export default defineConfig({
  testDir: './e2e', testMatch: ['market-pins-review.spec.ts', 'market-back-review.spec.ts', 'market-all-sources-review.spec.ts'], fullyParallel: false, workers: 1,
  reporter: [['list']],
  use: { baseURL: 'http://127.0.0.1:1426', channel: process.env.COBBLE_TEST_BROWSER_CHANNEL },
  webServer: { command: 'pnpm exec vite --host 127.0.0.1 --port 1426 --strictPort', url: 'http://127.0.0.1:1426/e2e/fixtures/market-pins.html', reuseExistingServer: false },
});
