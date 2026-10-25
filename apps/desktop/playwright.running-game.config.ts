// SPDX-License-Identifier: MIT
import {defineConfig} from '@playwright/test';
export default defineConfig({testDir:'./e2e',testMatch:'running-game.spec.ts',workers:1,reporter:[['list']],use:{baseURL:'http://127.0.0.1:1423',channel:process.env.COBBLE_TEST_BROWSER_CHANNEL},webServer:{command:'pnpm exec vite --host 127.0.0.1 --port 1423 --strictPort',url:'http://127.0.0.1:1423/e2e/fixtures/running-game.html',reuseExistingServer:false}});
