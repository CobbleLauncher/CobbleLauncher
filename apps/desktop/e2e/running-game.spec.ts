// SPDX-License-Identifier: MIT
// Component fixture with synthetic native reads; no native process or Wasm acceptance claim.
import {expect,test} from '@playwright/test';
for(const sample of [{locale:'en-US',theme:'light',width:1280},{locale:'zh-CN',theme:'dark',width:560}])test(`running workspace ${sample.locale}`,async({page})=>{
 await page.setViewportSize({width:sample.width,height:820});await page.route('https://**/*',route=>route.abort());
 await page.addInitScript(()=>{(window as unknown as {__TAURI_INTERNALS__:unknown}).__TAURI_INTERNALS__={invoke:async()=>({preference:'enabled',last_backup:'2026-10-23T10:00:00Z',last_error:null})};});
 await page.goto(`/e2e/fixtures/running-game.html?locale=${sample.locale}&theme=${sample.theme}`);await expect(page.locator('.tool-action')).toHaveCount(2);await expect(page.locator('.backup-status')).not.toContainText('Loading');
 expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth)).toBe(true);
 const box=await page.locator('.running-workspace').boundingBox();expect(box!.width).toBeGreaterThan(sample.width-70);
 await page.evaluate(() => document.fonts.ready);
 await page.screenshot({path:`../../target/ui-evidence/running-game-${sample.locale}.png`,fullPage:true});
});
