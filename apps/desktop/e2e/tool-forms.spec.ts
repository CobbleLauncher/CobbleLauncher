// SPDX-License-Identifier: MIT
// Actual Command Pocket manifest + host renderer. Synthetic native invocation and clipboard only.
// These checks cover rendering/interaction, not Wasm execution or Minecraft acceptance.
import { expect,test,type Page } from '@playwright/test';
type Call={command:string;args:Record<string,unknown>};
type FixtureWindow=Window & {__toolCalls:Call[];__toolPending:boolean;__toolFinish?:()=>void;__toolConfirmLeave:()=>Promise<boolean>};
async function fixture(page:Page,locale:string,theme:string,width:number){
  await page.setViewportSize({width,height:900});
  await page.route('https://**/*',route=>route.abort());
  await page.addInitScript(()=>{
    const target=window as unknown as FixtureWindow & {__TAURI_INTERNALS__:unknown};
    target.__toolCalls=[];target.__toolPending=false;
    const output={title:'Command',blocks:[{label:null,text:'/tp @s ~ ~ ~'}]};
    target.__TAURI_INTERNALS__={invoke:async(command:string,args:Record<string,unknown>={})=>{
      target.__toolCalls.push({command,args});
      if(command==='run_plugin_tool_form'){
        if(target.__toolPending)return new Promise(resolve=>{target.__toolFinish=()=>resolve(output);});
        return output;
      }
      return null;
    }};
  });
  await page.goto(`/e2e/fixtures/tool-form.html?locale=${locale}&theme=${theme}`);
  await expect(page.locator('.tool-form-panel select')).toHaveCount(2);
}
async function calls(page:Page,command:string){return page.evaluate(command=>(window as unknown as FixtureWindow).__toolCalls.filter(call=>call.command===command),command);}
for(const locale of ['en-US','zh-CN'])for(const theme of ['light','dark'])for(const width of [1000,560]){
  test(`tool form ${locale} ${theme} ${width}`,async({page},info)=>{
    await fixture(page,locale,theme,width);
    expect(await calls(page,'run_plugin_tool_form')).toHaveLength(0);
    const selects=page.locator('.tool-form-panel select');
    await expect(selects.nth(0)).toHaveValue('"1.21.1"');
    await expect(page.locator('.tool-form-panel input')).toHaveCount(4);
    await selects.nth(1).selectOption('"give"');
    await expect(page.locator('.tool-form-panel input')).toHaveCount(3);
    await selects.nth(1).selectOption('"effect"');
    await expect(page.locator('.tool-form-panel input')).toHaveCount(5);
    await selects.nth(1).selectOption('"teleport"');
    const submit=page.locator('button[type=submit]');
    await submit.focus();await page.keyboard.press('Enter');
    await expect(page.locator('.tool-form-output textarea')).toHaveValue('/tp @s ~ ~ ~');
    await expect(page.locator('.tool-form-output textarea')).toHaveAttribute('readonly','');
    await page.locator('.tool-form-output button').click();
    expect(await calls(page,'plugin:clipboard-manager|write_text')).toHaveLength(1);
    expect((await calls(page,'run_plugin_tool_form'))[0]?.args).toMatchObject({pluginId:'command-pocket',locale:locale==='en-US'?'en':'zh-CN',input:{version:'1.21.1',template:'teleport'}});
    expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth)).toBe(true);
    await page.screenshot({path:info.outputPath(`tool-form-${locale}-${theme}-${width}.png`),fullPage:true,animations:'disabled'});
    await page.locator('.tool-form-panel input').first().fill('@a');
    await expect(page.locator('.tool-form-output')).toHaveCount(0);
  });
}
test('transient draft may leave; active generation cancels and late output is suppressed',async({page},info)=>{
  await fixture(page,'en-US','light',560);
  await page.locator('.tool-form-panel input').first().fill('@a');
  expect(await page.evaluate(()=>(window as unknown as FixtureWindow).__toolConfirmLeave())).toBe(true);
  expect(await calls(page,'run_plugin_tool_form')).toHaveLength(0);
  await page.evaluate(()=>{(window as unknown as FixtureWindow).__toolPending=true;});
  await page.locator('button[type=submit]').click();
  await expect(page.locator('.tool-form-panel')).toHaveAttribute('aria-busy','true');
  await page.screenshot({path:info.outputPath('tool-form-pending-560.png'),fullPage:true,animations:'disabled'});
  expect(await page.evaluate(()=>(window as unknown as FixtureWindow).__toolConfirmLeave())).toBe(true);
  expect(await calls(page,'cancel_plugin_tool_form')).toEqual([{command:'cancel_plugin_tool_form',args:{pluginId:'command-pocket'}}]);
  await page.evaluate(()=>(window as unknown as FixtureWindow).__toolFinish?.());
  await expect(page.locator('.tool-form-panel')).toHaveAttribute('aria-busy','false');
  await expect(page.locator('.tool-form-output')).toHaveCount(0);
});
