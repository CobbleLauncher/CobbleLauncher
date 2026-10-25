// SPDX-License-Identifier: MIT
import { defineConfig } from '@playwright/test';
export default defineConfig({
  testDir:'./e2e',testMatch:'tool-forms.spec.ts',fullyParallel:false,workers:1,reporter:[['list']],
  use:{baseURL:'http://127.0.0.1:1421',channel:process.env.COBBLE_TEST_BROWSER_CHANNEL},
  webServer:{command:'pnpm exec vite --host 127.0.0.1 --port 1421 --strictPort',url:'http://127.0.0.1:1421/e2e/fixtures/tool-form.html',reuseExistingServer:false},
});
