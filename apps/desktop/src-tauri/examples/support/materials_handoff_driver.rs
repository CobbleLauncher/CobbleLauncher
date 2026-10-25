// SPDX-License-Identifier: MIT
use serde_json::{Value, json};
use std::path::Path;
pub fn install(source: &Path, data: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let mut manifest: Value = serde_json::from_slice(&std::fs::read(source.join("plugin.json"))?)?;
    if manifest["id"] != "material-planner" {
        return Err("canonical material planner required".into());
    }
    let package = data.join("consumer-package");
    let index: Value =
        serde_json::from_slice(&std::fs::read(source.join("frontend-assets.json"))?)?;
    let mut entries = index["files"].as_array().ok_or("asset index")?.clone();
    for entry in &entries {
        let relative = entry["path"].as_str().ok_or("asset path")?;
        let target = package.join(relative);
        std::fs::create_dir_all(target.parent().ok_or("parent")?)?;
        std::fs::copy(source.join(relative), target)?;
    }
    for relative in [Some("component.wasm"), manifest["icon"]["file"].as_str()]
        .into_iter()
        .flatten()
    {
        std::fs::copy(source.join(relative), package.join(relative))?;
    }
    let html = std::fs::read_to_string(package.join("ui/index.html"))?.replace(
        "</body>",
        "<script type=\"module\" src=\"./joined-driver.js\"></script></body>",
    );
    std::fs::write(package.join("ui/index.html"), html)?;
    std::fs::write(package.join("ui/joined-driver.js"), CHILD)?;
    entries.push(json!({"path":"ui/joined-driver.js"}));
    for entry in &mut entries {
        let bytes = std::fs::read(package.join(entry["path"].as_str().ok_or("indexed path")?))?;
        entry["bytes"] = json!(bytes.len());
        entry["sha256"] = json!(cobble_plugin_host::digest(&bytes));
    }
    let index = serde_json::to_vec(&json!({"schema_version":1,"files":entries}))?;
    manifest["frontend"]["assets_sha256"] = json!(cobble_plugin_host::digest(&index));
    std::fs::write(package.join("frontend-assets.json"), index)?;
    std::fs::write(package.join("plugin.json"), serde_json::to_vec(&manifest)?)?;
    cobble_plugin_host::PluginRegistry::new(data.join("CobbleLauncher/plugins"))
        .install_directory(&package)?;
    Ok(())
}
pub fn summary(method: &str, result: &Result<Value, String>) -> Value {
    match result {
        Err(code) => json!({"Err":code}),
        Ok(value) => {
            if method == "handoffs.take" {
                return json!({"Ok":{"present":!value.is_null(),"materials":value["payload"]["materials"].as_array().map(Vec::len),"general":value["game"].is_null(),"unknownStacks":value["payload"]["materials"].as_array().is_some_and(|rows|rows.iter().all(|r|r["stackSize"].is_null()))}});
            }
            if method.starts_with("data.") {
                let v = &value["value"];
                return json!({"Ok":{"revisionPresent":value["revision"].is_string(),"valuePresent":!v.is_null(),"itemCount":v["itemCount"],"chunkItems":v["items"].as_array().map(Vec::len),"phase":v.as_str().filter(|s|matches!(*s,"saved"|"reopened"|"failed")),"digest":if v.is_null(){None}else{Some(cobble_plugin_host::digest(v.to_string().as_bytes()))}}});
            }
            json!({"Ok":true})
        }
    }
}
pub const CHILD: &str = r#"
const bridge=window.__COBBLE_PLUGIN_BRIDGE__,pause=ms=>new Promise(r=>setTimeout(r,ms));
async function wait(test){for(let i=0;i<1600;i++){if(await test())return;await pause(25)}throw Error('material consumer driver timeout')}
const field=name=>document.querySelector(`[data-field=${name}]`),action=name=>document.querySelector(`[data-action=${name}]`);
const ready=()=>document.querySelector('.material-planner')?.getAttribute('aria-busy')==='false';
async function mark(value){const old=await bridge.request('data.read',{scope:{},key:'native-probe-phase'});await bridge.request('data.write',{scope:{},key:'native-probe-phase',value,expectedRevision:old.revision})}
try{
 await wait(()=>ready()&&field('game'));
 const context=await bridge.request('context',{});if(context.instanceId!==null||field('game').value!=='')throw Error('consumer was silently game-bound');
 const phase=await bridge.request('data.read',{scope:{},key:'native-probe-phase'});
 await wait(()=>ready()&&document.querySelectorAll('tbody tr[data-material]').length===1000);
 if(phase.value===null){
  const empty=await bridge.request('data.read',{scope:{},key:'plans-v1'});if(empty.value!==null)throw Error('handoff wrote a saved plan before Save');
  if(action('save').disabled)throw Error('handoff did not leave an unsaved draft');
  const row=document.querySelector('tbody tr[data-material]');for(const [name,value] of [['collected','1'],['stack-size','64']]){const input=row.querySelector(`[data-field=${name}]`);input.value=value;input.dispatchEvent(new Event('input',{bubbles:true}));input.dispatchEvent(new Event('change',{bubbles:true}));}
  action('save').click();await wait(()=>ready()&&document.querySelector('.status').textContent==='Progress saved.');
  const index=await bridge.request('data.read',{scope:{},key:'plans-v1'});if(index.value.plans.length!==1)throw Error('saved index missing');
  const plan=await bridge.request('data.read',{scope:{},key:'p-'+index.value.plans[0].id});
  const rows=(await Promise.all(plan.value.chunks.map(async ref=>(await bridge.request('data.read',{scope:{},key:ref.key})).value.items))).flat();
  if(rows.length!==1000||new Set(rows.map(r=>r.id)).size!==1000||!rows.every(r=>r.required===1&&/^fixture:block_[0-9]+$/.test(r.id))||rows.filter(r=>r.stackSize===64&&r.collected===1).length!==1||rows.filter(r=>r.stackSize===null).length!==999)throw Error('saved full plan or progress mismatch');
  await mark('saved');
 }else if(phase.value==='saved'){
  const rows=[...document.querySelectorAll('tbody tr[data-material]')];
  if(rows.filter(row=>row.querySelector('[data-field=collected]').value==='1'&&row.querySelector('[data-field=stack-size]').value==='64').length!==1)throw Error('reopened progress was lost');
  await mark('reopened');
 }else throw Error('unexpected native phase');
}catch(error){console.error('Material probe',String(error));try{await mark('failed')}catch{}}

"#;
pub const PARENT: &str = r#"
localStorage.setItem('cobble-language','en-US');
const bootErrors=[];window.addEventListener('error',event=>bootErrors.push({message:event.message??'resource error',source:event.target?.src??event.filename??''}),true);window.addEventListener('unhandledrejection',event=>bootErrors.push({rejection:String(event.reason)}));
window.addEventListener('DOMContentLoaded',async()=>{
const invoke=(c,a)=>window.__TAURI_INTERNALS__.invoke(c,a),pause=ms=>new Promise(r=>setTimeout(r,ms));
async function wait(test){for(let i=0;i<1200;i++){const result=await test();if(result)return result;await pause(25);}throw Error('materials handoff driver timeout')}
const button=text=>[...document.querySelectorAll('button')].find(n=>n.textContent.trim()===text);const proof={passed:false,workflow:'1000-materials-general-plan'};
try{
 await wait(()=>button('Tools'));button('Tools').click();(await wait(()=>[...document.querySelectorAll('button')].find(n=>n.textContent.includes('Blueprint library')))).click();
 (await wait(()=>document.querySelector('.blueprint-open'))).click();
 await wait(()=>document.querySelectorAll('.materials tbody tr').length===1000&&document.querySelector('[data-action=send-materials]'));
 proof.renderedRows=document.querySelectorAll('.materials tbody tr').length;const scope=document.querySelector('[data-field=material-game]');proof.defaultGeneral=scope.value==='';if(!proof.defaultGeneral)throw Error('material entry silently chose a game');
 document.querySelector('[data-action=send-materials]').click();
 await wait(async()=>{const state=await invoke('get_app_info');if(state.events.some(e=>e.result?.Ok?.phase==='failed'))throw Error('consumer workflow failed');return state.events.some(e=>e.result?.Ok?.phase==='saved')});
 proof.saved=true;
 // Real Tools navigation closes and recreates the plugin WebView. Its token is not replayed.
 const back=await wait(()=>document.querySelector('.back-button'));back.click();
 await wait(()=>document.querySelector('.blueprint-open'));
 // Reopen the installed consumer through the real native view command after the main UI closed its prior view.
 const plugins=await invoke('list_plugins');proof.consumerInstalled=plugins.some(p=>p.manifest.id==='material-planner');
 const view=await invoke('open_plugin_view',{pluginId:'material-planner',instanceId:null,bounds:{x:0,y:35,width:1100,height:740},locale:'en-US',theme:'light'});
 await wait(async()=>{const state=await invoke('get_app_info');if(state.events.some(e=>e.result?.Ok?.phase==='failed'))throw Error('consumer reopen failed');return state.events.some(e=>e.result?.Ok?.phase==='reopened')});
 proof.reopened=true;await invoke('close_plugin_view',{viewId:view.view_id});proof.closed=true;proof.passed=proof.defaultGeneral&&proof.renderedRows===1000&&proof.saved&&proof.reopened;
}catch(error){proof.error=String(error);proof.body=document.body.innerText.slice(-2000);proof.html=document.documentElement.outerHTML.slice(0,2000);proof.bootErrors=bootErrors;proof.readyState=document.readyState}
await invoke('get_app_info',{proof});
});
"#;
