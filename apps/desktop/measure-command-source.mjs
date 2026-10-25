// SPDX-License-Identifier: MIT
// Source-rendered synthetic boundary. No native launch or game execution.
import { chromium, expect } from '@playwright/test';
import { mkdir, writeFile, readFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { createHash } from 'node:crypto';
const out = resolve(process.env.COBBLE_PERF_REPORT_DIR ?? '../../target/command-source-baseline');
await mkdir(out,{recursive:true});
const browser = await chromium.launch({headless:true,channel:'msedge'});
const page = await browser.newPage({viewport:{width:1100,height:800},reducedMotion:'reduce'});
const errors=[], remote=[]; page.on('pageerror',error=>errors.push(error.message));
await page.route('https://**/*',route=>{remote.push(route.request().url());return route.abort();});
await page.addInitScript(()=>{
 localStorage.setItem('cobble-language','en-US'); localStorage.setItem('cobble.appearance','light');
 window.isTauri=true; let callback=0; window.__TAURI_EVENT_PLUGIN_INTERNALS__={unregisterListener:()=>{}};
 const games=Array.from({length:1000},(_,i)=>({id:'fixture-'+i,name:'Fixture game '+String(i).padStart(4,'0'),version:'1.21.1',loader_kind:'fabric',loader_version:'0.16.14',directory:'synthetic',status:'installed',progress:null,error:null}));
 const commands=Array.from({length:100},(_,i)=>({id:'action-'+i,label:'Plugin action '+String(i).padStart(3,'0'),menus:[{position:'tools'}],searchKeywords:['synthetic'],quickEntry:true}));
 window.__TAURI_INTERNALS__={metadata:{currentWindow:{label:'main'},currentWebview:{label:'main'}},transformCallback:()=>++callback,invoke:async(command)=>{
  if(command==='plugin:event|listen')return callback;
  if(command==='plugin:window|is_maximized')return false;
  if(command==='list_games')return games;
  if(command==='list_extension_contributions')return [{pluginId:'fixture-plugin',pluginName:'Fixture tools',commands}];
  if(command==='list_player_profiles')return {profiles:[],active_id:null};
  if(command==='get_launcher_preferences')return {memory_mb:4096,game_directory:'synthetic',custom_game_directory:false};
  if(command==='get_download_settings')return {source:'automatic',mirror:''};
  if(command==='get_download_tuning')return {max_concurrent:12,per_origin:4};
  if(command==='get_public_cache_stats')return {disk_bytes:0};
  if(command==='get_java_settings')return {mode:'automatic',executable:null,runtime:null};
  if(command==='list_java_runtimes')return {runtimes:[],recommendedId:null};
  if(command.startsWith('list_')||command==='get_game_covers'||command==='get_game_icons')return [];
  return null;
 }};
});
const report={scope:'Current source UI fixture only;1000 synthetic games and100 plugin actions;real Vue filtering/rendering with synthetic IPC.No prior feature baseline or native latency claim.',viewport:{width:1100,height:800},palette:{},quickChooser:{}};
try {
 await page.goto('http://127.0.0.1:1420/');await expect(page.locator('.game-card').first()).toBeVisible();
 report.browser=await page.evaluate(()=>navigator.userAgent);
 let started=await page.evaluate(()=>performance.now());await page.keyboard.press('Control+k');
 await expect(page.getByRole('option').first()).toBeVisible();
 report.palette.openReadyMs=await page.evaluate(t=>performance.now()-t,started);
 report.palette.initialRows=await page.getByRole('option').count();
 report.palette.search=[];const input=page.getByRole('searchbox',{name:'Search actions',exact:true});
 for(const [query,label]of [['Fixture game 0999','Fixture game 0999'],['Plugin action 099','Plugin action 099'],['Fixture game 0123','Fixture game 0123']]){
  started=await page.evaluate(()=>performance.now());await input.fill(query);await expect(page.getByRole('option',{name:label,exact:true})).toBeVisible();await expect(page.getByRole('option')).toHaveCount(1);
  report.palette.search.push({query,readyMs:await page.evaluate(t=>performance.now()-t,started),rows:await page.getByRole('option').count()});
 }
 await input.fill('Fixture game 099');await expect(page.getByRole('option',{name:'Fixture game 0991',exact:true})).toBeVisible();await input.press('ArrowDown');
 await expect(page.locator('#command-1')).toHaveAttribute('aria-selected','true');
 await page.screenshot({path:resolve(out,'palette-keyboard.png'),animations:'disabled'});
 await input.press('Escape');await expect(page.locator('.command-dialog')).toHaveCount(0);
 started=await page.evaluate(()=>performance.now());await page.locator('.home-edit-toggle').click();await expect(page.locator('.home-quick-candidate')).toHaveCount(20);
 report.quickChooser.openReadyMs=await page.evaluate(t=>performance.now()-t,started);report.quickChooser.initialRows=await page.locator('.home-quick-candidate').count();report.quickChooser.search=[];
 const quick=page.getByRole('searchbox',{name:'Find quick action',exact:true});
 for(const query of ['Fixture game 0999','Plugin action 099','Fixture game 0123']){
  started=await page.evaluate(()=>performance.now());await quick.fill(query);await expect(page.locator('.home-quick-candidate')).toHaveCount(1);
  await expect(page.locator('.home-quick-candidate')).toContainText(query);report.quickChooser.search.push({query,readyMs:await page.evaluate(t=>performance.now()-t,started),rows:await page.locator('.home-quick-candidate').count()});
 }
 await quick.press('Tab');await page.keyboard.press('Space');await expect(page.locator('.home-quick-candidate [role=switch]')).toHaveAttribute('aria-checked','true');
 await page.locator('.home-quick-picker').screenshot({path:resolve(out,'quick-chooser-keyboard.png'),animations:'disabled'});
 report.keyboard={paletteArrowDown:true,paletteEscape:true,quickChooserTabSpace:true};report.errors=errors;report.remoteRequests=remote;
 const files=['src/commands/CommandPalette.vue','src/commands/registry.ts','src/commands/use-shell-commands.ts','src/LibraryHome.vue'];report.sourceHashes=Object.fromEntries(await Promise.all(files.map(async file=>[file,createHash('sha256').update(await readFile(file)).digest('hex')])));
 await writeFile(resolve(out,'report.json'),JSON.stringify(report,null,2));console.log(JSON.stringify(report,null,2));
}finally{await browser.close();}




