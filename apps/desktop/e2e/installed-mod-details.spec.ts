// SPDX-License-Identifier: MIT
// Component rendering fixture; synthetic IPC, no provider/network/game writes.
import { expect, test } from '@playwright/test';
test('installed details review a compatible downgrade and required addition at narrow and wide widths', async ({page}, testInfo) => {
  const errors: string[] = []; page.on('pageerror', error => errors.push(error.message));
  await page.addInitScript(() => {
    const target = window as unknown as {isTauri: boolean; __TAURI_INTERNALS__: unknown; __modCalls: {command:string;args:unknown}[]};
    target.isTauri = true; target.__modCalls = [];
    const mod = {file_name:'forest-map.jar.disabled',name:'Forest Map',version:'2.0',authors:['Original fixture author'],loader:'Fabric',enabled:false,description:'A map for our shared survival world.'};
    const current = {version_id:'v2',name:'Forest Map',version_number:'2.0',version_type:'release',date_published:'2026-10-20'};
    target.__TAURI_INTERNALS__ = {invoke: async (command: string, args: unknown) => {
      target.__modCalls.push({command,args});
      if(command === 'list_game_mods') return [mod];
      if(command === 'list_managed_game_mods') return [{project_id:'forest',version_id:'v2',filename:'forest-map.jar',enabled:false,name:mod.name}];
      if(command === 'list_game_mod_versions') return [current,{...current,version_id:'v1',version_number:'1.0',date_published:'2026-09-01'}];
      if(command === 'preview_game_mod_version_changes') return {plan_id:'review-plan',changes:[
        {project_id:'forest',name:mod.name,from_version:'2.0',to_version:'1.0',filename:'forest-map.jar',enabled:false},
        {project_id:'api',name:'Required API',from_version:null,to_version:'3.0',filename:'api.jar',enabled:true}
      ]};
      if(command === 'apply_game_mod_updates') return {updated:1,added:1};
      return null;
    }};
  });
  await page.emulateMedia({ reducedMotion:'reduce' });
  await page.setViewportSize({width:560,height:800});
  await page.goto('/e2e/fixtures/installed-mod-details.html');
  await page.getByRole('button',{name:'Details for Forest Map',exact:true}).click();
  await expect(page.locator('.installed-mod-details').getByText('forest-map.jar.disabled',{exact:true})).toBeVisible();
  await page.screenshot({path:testInfo.outputPath('installed-mod-details-560.png'),fullPage:true});
  await page.getByRole('button',{name:'Versions and dependencies',exact:true}).click();
  await page.getByLabel('Mod version').fill('1.0');
  await page.getByRole('option').filter({hasText:'1.0'}).click();
  await page.getByRole('button',{name:'Review version changes',exact:true}).click();
  await expect(page.getByRole('list',{name:'Version changes'})).toContainText('Required API');
  await expect(page.getByRole('list',{name:'Version changes'})).toContainText('Keep disabled');
  const applied = () => page.evaluate(() => (window as unknown as {__modCalls:{command:string}[]}).__modCalls.some(call=>call.command==='apply_game_mod_updates'));
  expect(await applied()).toBe(false);
  for(const width of [560,1100]) {
    await page.setViewportSize({width,height:800});
    expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth)).toBe(true);
    await page.screenshot({path:testInfo.outputPath(`installed-mod-plan-${width}.png`),fullPage:true});
  }
  await page.getByRole('button',{name:'Confirm changes',exact:true}).click();
  await expect(page.getByText('Version changed.',{exact:true})).toBeVisible();
  expect(await applied()).toBe(true);
  expect(errors).toEqual([]);
});
