// SPDX-License-Identifier: MIT
import {expect,test} from '@playwright/test';
import path from 'node:path';
type FixtureWindow=Window & {isTauri:boolean;__TAURI_INTERNALS__:{invoke:(command:string)=>Promise<unknown>};packJsonPanel?:{value?:{supported:boolean;choose:()=>Promise<void>}}};

test('JSON field choices render in a copy preview in wide and narrow layouts',async({page})=>{
 await page.addInitScript(()=>{
  localStorage.setItem('cobble-language','en-US');
  const w=window as unknown as FixtureWindow;w.isTauri=true;
  w.__TAURI_INTERNALS__={invoke:async(command:string)=>{
   if(command==='get_game_pack_update_context')return{supported:true,provider:'mrpack',name:'Synthetic pack',installed_version:'v1',online_versions:false,identity_relation:'unknown',reason:null};
   if(command==='preview_game_pack_update'){
    const f={identity:'config/settings.json',path:'config/settings.json',version_id:null,size_bytes:42,sha1:null,sha512:null};
    const value=(value:unknown,exists=true)=>({value,exists});
    return{plan_id:'fixture',player_added:[],migratable_paths:['config/settings.json'],baseline_available:true,baseline_status:'verified',target_name:'Synthetic pack',excluded_optional_files:0,observed_file_count:1,unobserved_file_count:0,current_version:'v1',target_version:'v2',identity_relation:'confirmed',runtime_changed:false,added:[],removed:[],unchanged:[],version_changed:[{before:f,after:f,local_state:'modified',preserve:true}],json_configs:[{path:f.path,fields:[{pointer:'/difficulty',base:value('normal'),local:value('hard'),upstream:value('peaceful'),conflict:true,suggested:'local'},{pointer:'/hud/scale',base:value(1),local:value(1.5),upstream:value(1),conflict:false,suggested:'local'},{pointer:'/newFeature',base:value(null,false),local:value(null,false),upstream:value(true),conflict:false,suggested:'upstream'}]}]};
   }
   return null;
  }};
 });
 await page.setViewportSize({width:1100,height:850});
 await page.goto('/e2e/pack-json-harness.html');
 await page.waitForFunction(()=>(window as unknown as FixtureWindow).packJsonPanel?.value?.supported);
 await page.evaluate(()=>(window as unknown as FixtureWindow).packJsonPanel!.value!.choose());
 await page.getByRole('radio',{name:'Merge fields',exact:true}).click();
 await expect(page.getByRole('columnheader',{name:'Original pack',exact:true})).toBeVisible();
 const create=page.getByRole('button',{name:'Create update copy',exact:true});
 await expect(create).toBeDisabled();
 await page.locator('.pack-update-json tbody tr').first().getByRole('radio',{name:'Keep local',exact:true}).click();
 await expect(create).toBeEnabled();
 await page.screenshot({path:path.resolve(process.cwd(),'../../validation/T077-pack-json-wide.png'),fullPage:true});
 await page.setViewportSize({width:420,height:850});
 await expect(create).toBeVisible();
 expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth)).toBe(true);
 await page.screenshot({path:path.resolve(process.cwd(),'../../validation/T077-pack-json-narrow.png'),fullPage:true});
});
