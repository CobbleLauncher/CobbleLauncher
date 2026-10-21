// SPDX-License-Identifier: MIT
import { defineConfig } from '@playwright/test';

export default defineConfig({
  testDir: './e2e',
  fullyParallel: true,
  reporter: [['list']],
  use: {
    baseURL: 'http://127.0.0.1:1420',
    viewport: { width: 680, height: 560 },
    channel: process.env.COBBLE_TEST_BROWSER_CHANNEL,
  },
  webServer: {
    command: 'pnpm preview',
    url: 'http://127.0.0.1:1420',
    reuseExistingServer: false,
  },
});
