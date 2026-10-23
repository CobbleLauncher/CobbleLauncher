// SPDX-License-Identifier: MIT
// Synthetic native boundary: tests the real Create Game form and shared selector layout.
import {expect,test} from '@playwright/test';
test('version popup stays anchored without moving the create form at narrow and wide widths',async({page})=>{
 await page.addInitScript(()=>{
  localStorage.setItem('cobble-language','en-US');let callback=0;
  const target=window as unknown as {isTauri:boolean;__TAURI_INTERNALS__:unknown;__TAURI_EVENT_PLUGIN_INTERNALS__:unknown};target.isTauri=true;target.__TAURI_EVENT_PLUGIN_INTERNALS__={unregisterListener:()=>{}};
  target.__TAURI_INTERNALS__={metadata:{currentWindow:{label:'main'},currentWebview:{label:'main'}},transformCallback:()=>++callback,invoke:async(command:string)=>{
   if(command==='plugin:event|listen')return callback;if(command==='plugin:window|is_maximized')return false;
   if(['list_games','list_tasks','list_plugins','list_extension_contributions','list_plugin_catalogs','list_bundled_plugins','list_blueprints'].includes(command))return[];
   if(command==='list_player_profiles')return{profiles:[],active_id:null};
   if(command==='get_launcher_preferences')return{memory_mb:4096,game_directory:'C:/Games',custom_game_directory:false};
   if(command==='get_download_settings')return{source:'automatic',mirror:''};if(command==='default_game_directory')return'C:/Games';
   if(command==='list_game_versions')return Array.from({length:40},(_,index)=>({id:`1.21.${index}`,kind:'release'}));
   return null;
  }};
 });
 for(const [width,height] of [[560,560],[1100,850]]){
  await page.setViewportSize({width:width!,height:height!});await page.goto('/');await page.getByRole('button',{name:'Create game',exact:true}).click();
  const input=page.locator('#game-version');await expect(input).toHaveValue('1.21.0');await input.scrollIntoViewIfNeeded();
  const form=page.locator('.create-form');const before=await form.boundingBox();const anchor=await input.boundingBox();
  await input.click();await expect(page.getByRole('option',{name:'1.21.1',exact:true})).toBeVisible();
  const popup=page.locator('.version-selector-content'); await expect.poll(async()=>Math.round((await popup.boundingBox())!.width)).toBe(Math.round(anchor!.width));const box=await popup.boundingBox();const after=await form.boundingBox();
  expect(box!.width).toBeGreaterThan(150);expect(Math.abs(box!.width-anchor!.width)).toBeLessThan(3);expect(box!.height).toBeGreaterThan(90);
  expect(box!.x).toBeGreaterThanOrEqual(0);expect(box!.x+box!.width).toBeLessThanOrEqual(width!);expect(box!.y).toBeGreaterThanOrEqual(0);expect(box!.y+box!.height).toBeLessThanOrEqual(height!);
  expect(Math.abs(after!.height-before!.height)).toBeLessThan(1);expect(Math.abs(after!.y-before!.y)).toBeLessThan(1);
  await page.getByRole('button',{name:'Load more',exact:true}).click();await expect(page.getByRole('option',{name:'1.21.20',exact:true})).toBeVisible();
  await input.fill('1.21.39');await expect(page.getByRole('option',{name:'1.21.39',exact:true})).toBeVisible();await input.press('ArrowDown');await input.press('Enter');await expect(input).toHaveValue('1.21.39');await page.locator('.version-selector-trigger').first().click();await expect(page.getByRole('option',{name:'1.21.39',exact:true})).toBeVisible();await input.press('Escape');
  expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth)).toBe(true);
 }
});

