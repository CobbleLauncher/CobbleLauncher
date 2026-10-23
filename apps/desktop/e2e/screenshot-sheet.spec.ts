// SPDX-License-Identifier: MIT
// Synthetic native boundary only: exercises the real App/gallery UI, not the guest or filesystem export.
import {expect,test} from '@playwright/test';
test('selected screenshots produce a bounded preview and explicit Save As at narrow and wide widths',async({page},info)=>{
 await page.addInitScript(()=>{
  localStorage.setItem('cobble-language','en-US');let callback=0;let saves=0;
  const canvas=document.createElement('canvas');canvas.width=512;canvas.height=256;const context=canvas.getContext('2d')!;context.fillStyle='#78946b';context.fillRect(0,0,256,256);context.fillStyle='#9baec4';context.fillRect(264,0,248,256);const png=canvas.toDataURL('image/png');
  const game={id:'gallery-game',name:'Gallery game',version:'1.21.1',directory:'synthetic',status:'installed',progress:null,error:null,launch_state:'idle'};
  const target=window as unknown as {isTauri:boolean;__TAURI_INTERNALS__:unknown;__TAURI_EVENT_PLUGIN_INTERNALS__:unknown;__sheetCalls:{command:string;args:unknown}[]};target.isTauri=true;target.__sheetCalls=[];target.__TAURI_EVENT_PLUGIN_INTERNALS__={unregisterListener:()=>{}};
  target.__TAURI_INTERNALS__={metadata:{currentWindow:{label:'main'},currentWebview:{label:'main'}},transformCallback:()=>++callback,invoke:async(command:string,args:unknown)=>{
   target.__sheetCalls.push({command,args});if(command==='plugin:event|listen')return callback;if(command==='plugin:window|is_maximized')return false;if(command==='list_games')return[game];
   if(['list_tasks','list_plugins','list_extension_contributions','list_plugin_catalogs','list_bundled_plugins','list_blueprints','list_game_mods','list_managed_game_mods','list_game_packs','list_game_worlds'].includes(command))return[];
   if(command==='list_player_profiles')return{profiles:[],active_id:null};if(command==='get_launcher_preferences')return{memory_mb:4096,game_directory:'C:/Synthetic',custom_game_directory:false};
   if(command==='get_download_settings')return{source:'automatic',mirror:''};if(command==='get_java_settings')return{mode:'automatic',executable:null,runtime:null};if(command==='list_java_runtimes')return{runtimes:[],recommendedId:null};
   if(command==='list_game_screenshots')return{items:Array.from({length:4},(_,index)=>({id:`shot-${index}`,name:`Capture ${index+1}.png`,modified_at:index,size_bytes:100})),total:4,next_offset:null};
   if(command==='get_game_screenshot')return{data_url:png,width:512,height:256,original_width:512,original_height:256};
   if(command==='list_screenshot_sheet_plugins')return[{plugin_id:'synthetic-layout',name:'Synthetic layout'}];if(command==='preview_screenshot_sheet')return{preview_id:'synthetic-preview',data_url:png,width:512,height:256};if(command==='save_screenshot_sheet')return++saves>1;return null;
  }};
 });
 for(const width of [560,1100]){
  await page.setViewportSize({width,height:850});await page.emulateMedia({reducedMotion:'reduce'});await page.goto('/');await page.getByRole('button',{name:'Manage Gallery game',exact:true}).click();await page.getByRole('tab',{name:'Screenshots',exact:true}).click();
  const initialGridTop=await page.locator('.screenshot-grid').evaluate(element=>(element as HTMLElement).offsetTop);await page.getByRole('checkbox',{name:'Select Capture 2.png',exact:true}).check();await page.getByRole('checkbox',{name:'Select Capture 1.png',exact:true}).check();await expect(page.getByText('2 / 12 selected',{exact:true})).toBeVisible();
  await expect(page.getByRole('button',{name:'Save As…',exact:true})).toHaveCount(0);await expect(page.getByRole('combobox',{name:'Columns',exact:true})).toHaveCount(0);expect((await page.locator('.screenshot-grid').evaluate(element=>(element as HTMLElement).offsetTop))-initialGridTop).toBeLessThan(200);await page.screenshot({path:info.outputPath(`selection-${width}.png`),animations:'disabled'});await page.getByRole('button',{name:'Preview contact sheet',exact:true}).click();await page.getByRole('combobox',{name:'Columns',exact:true}).selectOption('2');await page.getByRole('button',{name:'Preview contact sheet',exact:true}).click();await expect(page.getByAltText('Screenshot contact sheet preview')).toBeVisible();
  expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth)).toBe(true);await expect(page.locator('.app-workspace h1:visible')).toHaveCount(1);
  const calls=await page.evaluate(()=>(window as unknown as {__sheetCalls:{command:string;args:unknown}[]}).__sheetCalls);expect(calls.filter(call=>call.command==='save_screenshot_sheet')).toHaveLength(0);expect(calls.find(call=>call.command==='preview_screenshot_sheet')!.args).toMatchObject({gameId:'gallery-game',screenshotIds:['shot-1','shot-0'],pluginId:'synthetic-layout',columns:2});
  await page.getByRole('button',{name:'Save As…',exact:true}).click();await expect(page.getByText('Save canceled.',{exact:true})).toBeVisible();await expect(page.getByText('Contact sheet saved.',{exact:true})).toHaveCount(0);await page.screenshot({path:info.outputPath(`sheet-${width}.png`),animations:'disabled'});
 }
});

