// SPDX-License-Identifier: MIT
// Original geometric tool artwork; no game textures or third-party image assets.
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';
import { readFileSync, writeFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
import path from 'node:path';
const root = fileURLToPath(new URL('../', import.meta.url));
const require = createRequire(new URL('../apps/desktop/package.json', import.meta.url));
const { chromium } = require('@playwright/test');
const artwork = {
  'mod-conflicts': ['#496ba0','<rect x="25" y="25" width="28" height="28" rx="5"/><rect x="67" y="25" width="28" height="28" rx="5"/><rect x="25" y="67" width="28" height="28" rx="5"/><path d="M69 69l24 24m0-24L69 93"/>'],
  'resource-pack-stack': ['#967543','<path d="M22 46l38-21 38 21-38 21zM22 62l38 21 38-21M22 78l38 21 38-21"/>'],
  'world-dashboard': ['#4f8375','<circle cx="60" cy="60" r="34"/><ellipse cx="60" cy="60" rx="16" ry="34"/><path d="M28 47h64M28 73h64"/>'],
  'backup-auditor': ['#6a70a2','<path d="M28 51v43h64V51M24 34h72v17H24zM50 70h20"/><path d="M42 23v-5m36 5v-5"/>'],
  'datapack-auditor': ['#957059','<path d="M26 40l34-18 34 18v40L60 98 26 80zM26 40l34 20 34-20M60 60v38M43 31l34 20"/>'],
  'serverbook-inspector': ['#587a9d','<rect x="25" y="24" width="70" height="72" rx="8"/><path d="M25 48h70M25 72h70M36 36h3m10 0h24M36 60h3m10 0h24M36 84h3m10 0h24"/>'],
  'screenshot-timeline': ['#9a6d86','<rect x="23" y="28" width="74" height="61" rx="7"/><circle cx="76" cy="45" r="6"/><path d="M24 80l24-28 17 20 11-12 21 22"/>'],
  'controls-auditor': ['#55827e','<rect x="24" y="25" width="30" height="30" rx="5"/><rect x="66" y="25" width="30" height="30" rx="5"/><rect x="24" y="67" width="72" height="28" rx="5"/><path d="M34 82h52M39 34v12m-6-6h12M76 40h10"/>'],
  'storage-lens': ['#8c7b4a','<path d="M28 93V66h15v27M52 93V46h15v47M76 93V26h15v67M22 99h76"/>'],
  'runtime-advisor': ['#66834c','<path d="M29 88a43 43 0 1 1 62 0M60 67l22-28M30 60h8m44 0h8M60 26v8"/><circle cx="60" cy="67" r="7"/>'],
};
const browser = await chromium.launch({ channel:'msedge', headless:true });
try {
  const page = await browser.newPage({viewport:{width:120,height:120},deviceScaleFactor:1});
  for (const [id,[color,shapes]] of Object.entries(artwork)) {
    await page.setContent(`<style>body{margin:0}</style><svg xmlns="http://www.w3.org/2000/svg" width="120" height="120" viewBox="0 0 120 120"><rect width="120" height="120" rx="25" fill="${color}"/><g fill="none" stroke="#fff" stroke-width="4" stroke-linecap="round" stroke-linejoin="round">${shapes}</g></svg>`);
    const icon = path.join(root,'examples',id,'icon.png');
    await page.screenshot({path:icon,omitBackground:true});
    const manifestPath = path.join(root,'examples',id,'plugin.json');
    const manifest = JSON.parse(readFileSync(manifestPath,'utf8'));
    manifest.icon = {file:'icon.png',sha256:createHash('sha256').update(readFileSync(icon)).digest('hex')};
    writeFileSync(manifestPath,JSON.stringify(manifest,null,2)+'\n');
  }
} finally { await browser.close(); }
