// SPDX-License-Identifier: MIT
//! Actual isolated plugin UI/private-storage workflow; no browser mock or user data.
use serde_json::{Value, json};
use std::path::Path;

pub fn install_sender(source: &Path, data: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let source = source
        .parent()
        .ok_or("canonical packages directory")?
        .join("seed-compass-package");
    let mut manifest: Value = serde_json::from_slice(&std::fs::read(source.join("plugin.json"))?)?;
    if manifest["id"] != "seed-compass" {
        return Err("canonical Seed Compass package required".into());
    }
    let package = data.join("sender-package");
    let index: Value =
        serde_json::from_slice(&std::fs::read(source.join("frontend-assets.json"))?)?;
    let mut entries = index["files"]
        .as_array()
        .ok_or("sender asset index")?
        .clone();
    for entry in &entries {
        let relative = entry["path"].as_str().ok_or("sender asset path")?;
        let target = package.join(relative);
        std::fs::create_dir_all(target.parent().ok_or("sender asset parent")?)?;
        std::fs::copy(source.join(relative), target)?;
    }
    std::fs::copy(
        source.join("component.wasm"),
        package.join("component.wasm"),
    )?;
    if let Some(icon) = manifest["icon"]["file"].as_str() {
        std::fs::copy(source.join(icon), package.join(icon))?;
    }
    let html = std::fs::read_to_string(package.join("ui/index.html"))?.replace(
        "</body>",
        "<script type=\"module\" src=\"./joined-driver.js\"></script></body>",
    );
    std::fs::write(package.join("ui/index.html"), html)?;
    std::fs::write(package.join("ui/joined-driver.js"), SENDER)?;
    entries.push(json!({"path":"ui/joined-driver.js"}));
    for entry in &mut entries {
        let bytes =
            std::fs::read(package.join(entry["path"].as_str().ok_or("sender indexed path")?))?;
        entry["bytes"] = json!(bytes.len());
        entry["sha256"] = json!(cobble_plugin_host::digest(&bytes));
    }
    let index = serde_json::to_vec(&json!({"schema_version":1,"files":entries}))?;
    manifest["frontend"]["assets_sha256"] = json!(cobble_plugin_host::digest(&index));
    std::fs::write(package.join("frontend-assets.json"), index)?;
    std::fs::write(package.join("plugin.json"), serde_json::to_vec(&manifest)?)?;
    cobble_plugin_host::PluginRegistry::new(data.join("CobbleLauncher/plugins"))
        .install_directory(&package)?;
    super::seed_compass_driver::prepare(&data.join("managed-game"))?;
    Ok(())
}

pub fn summarized_result(method: &str, result: &Result<Value, String>) -> Value {
    if method == "handoffs.take" {
        return match result {
            Err(_) => json!(result),
            Ok(value) => {
                json!({"Ok":{"handoffPresent":!value.is_null(),"senderPluginId":value["sender"]["pluginId"],"worldScopePresent":value["worldId"].is_string(),"gameId":value["game"]["id"]}})
            }
        };
    }
    if !matches!(method, "data.read" | "data.write" | "data.remove") {
        return super::seed_compass_driver::summarized_result(method, result);
    }
    let Ok(value) = result else {
        return json!(result);
    };
    let document = &value["value"];
    let phase = document
        .as_str()
        .filter(|value| matches!(*value, "reopen" | "done" | "handoff-saved" | "failed"));
    json!({"Ok":{
        "revision":value["revision"],
        "valuePresent":!document.is_null(),
        "valueDigest":if document.is_null(){None}else{Some(cobble_plugin_host::digest(document.to_string().as_bytes()))},
        "locationCount":document["locations"].as_array().map(Vec::len),
        "fixturePhase":phase
    }})
}

pub const DRIVER: &str = r#"
const bridge=window.__COBBLE_PLUGIN_BRIDGE__,pause=ms=>new Promise(resolve=>setTimeout(resolve,ms));
async function wait(test){for(let i=0;i<800;i++){if(test())return;await pause(25);}throw Error('coordinate native driver timed out');}
const field=name=>document.querySelector(`[data-field=${name}]`),action=name=>document.querySelector(`[data-action=${name}]`),set=(name,value)=>{const input=field(name);input.value=value;input.dispatchEvent(new Event('input',{bubbles:true}));input.dispatchEvent(new Event('change',{bubbles:true}));};
let scope,phase;
try{
 await wait(()=>field('world')?.querySelector('option[value=FixtureWorld]'));
 await wait(()=>action('new')&&!action('new').disabled&&document.querySelector('.coordinate-book').getAttribute('aria-busy')==='false');
 const context=await bridge.request('context',{});if(context.instanceId!=='012345abcdef')throw Error('bound context invalid');
 scope={gameId:context.instanceId,worldId:'FixtureWorld'};
 let denied=false;try{await bridge.request('data.read',{scope:{gameId:'fedcba543210',worldId:'FixtureWorld'},key:'locations-v1'});}catch(error){denied=String(error).includes('PLUGIN_GAME_SCOPE');}
 if(!denied)throw Error('cross-game private scope was not denied');
 if(field('world').value!=='FixtureWorld'){field('world').value='FixtureWorld';field('world').dispatchEvent(new Event('change',{bubbles:true}));}
 await wait(()=>field('world').value==='FixtureWorld'&&action('new')&&!action('new').disabled&&document.querySelector('.coordinate-book').getAttribute('aria-busy')==='false');
 phase=await bridge.request('data.read',{scope,key:'native-probe-phase'});
 if(phase.value===null){
  set('name','Native saved point');set('dimension','overworld');set('x','-8');set('y','72');set('z','136');set('note','Original synthetic fixture');
  action('save').click();await wait(()=>document.querySelectorAll('.location-row').length===1&&document.querySelector('[role=status]').textContent==='Location saved.'&&document.querySelector('.coordinate-book').getAttribute('aria-busy')==='false');
  let saved=await bridge.request('data.read',{scope,key:'locations-v1'});
  if(saved.value.locations[0].name!=='Native saved point'||saved.value.locations[0].x!==-8||!saved.revision)throw Error('UI save did not persist');
  set('x','-16');action('save').click();
  await wait(()=>document.querySelector('[role=status]').textContent==='Location saved.'&&document.querySelector('.coordinate-book').getAttribute('aria-busy')==='false');
  saved=await bridge.request('data.read',{scope,key:'locations-v1'});
  if(saved.value.locations[0].x!==-16)throw Error('UI edit did not persist');
  const peer=structuredClone(saved.value);peer.locations[0].note='Second native writer';
  await bridge.request('data.write',{scope,key:'locations-v1',value:peer,expectedRevision:saved.revision});
  set('name','Native retained draft');action('save').click();
  await wait(()=>document.querySelector('[role=alert]:not([hidden])')?.textContent.includes('changed')&&document.querySelector('.coordinate-book').getAttribute('aria-busy')==='false');
  if(field('name').value!=='Native retained draft')throw Error('conflict discarded draft');
  action('reload').click();await wait(()=>document.querySelector('[role=status]').textContent.includes('Saved locations reloaded.')&&document.querySelector('.coordinate-book').getAttribute('aria-busy')==='false');
  if(field('name').value!=='Native retained draft')throw Error('reload discarded draft');
  action('save').click();
  await wait(()=>document.querySelector('[role=status]').textContent==='Location saved.'&&document.querySelector('.coordinate-book').getAttribute('aria-busy')==='false');
  saved=await bridge.request('data.read',{scope,key:'locations-v1'});
  if(saved.value.locations[0].name!=='Native retained draft'||saved.value.locations[0].x!==-16)throw Error('conflict recovery did not persist');
  await bridge.request('data.write',{scope,key:'native-probe-phase',value:'reopen',expectedRevision:null});
 }else if(phase.value==='reopen'){
  await wait(()=>document.querySelectorAll('.location-row').length===1);
  if(field('name').value!=='Native retained draft'||field('x').value!=='-16')throw Error('reopened UI did not load saved data');
  const other=await bridge.request('data.read',{scope:{gameId:context.instanceId,worldId:'Other fixture'},key:'locations-v1'});
  if(other.value!==null||other.revision!==null)throw Error('world namespaces were mixed');
  action('delete').click();await wait(()=>!document.querySelector('.confirm-bar').hidden);action('confirm').click();await wait(()=>document.querySelectorAll('.location-row').length===0&&!action('new').disabled);
  const deleted=await bridge.request('data.read',{scope,key:'locations-v1'});
  if(deleted.value!==null||deleted.revision!==null)throw Error('last deletion did not remove document');
  await bridge.request('data.write',{scope,key:'native-probe-phase',value:'done',expectedRevision:phase.revision});
 }else if(phase.value==='done'){
  if(field('name').value!=='Map location 96, -128'||field('x').value!=='96'||field('z').value!=='-128'||field('y').value!==''||document.querySelector('.dirty-label').hidden)throw Error('real handoff did not prefill unsaved draft');
  const before=await bridge.request('data.read',{scope,key:'locations-v1'});if(before.value!==null||before.revision!==null)throw Error('handoff saved without confirmation');
  action('save').click();await wait(()=>document.querySelector('[role=status]').textContent==='Location saved.'&&document.querySelector('.coordinate-book').getAttribute('aria-busy')==='false');
  const saved=await bridge.request('data.read',{scope,key:'locations-v1'}),location=saved.value.locations[0];
  if(location.x!==96||location.z!==-128||location.y!==null||location.note!=='Seed Compass')throw Error('handoff draft Save did not persist');
  await bridge.request('data.write',{scope,key:'native-probe-phase',value:'handoff-saved',expectedRevision:phase.revision});
 }else throw Error('unexpected isolated fixture phase');
}catch(error){console.error('Coordinate probe:',String(error));try{phase=await bridge.request('data.read',{scope:scope??{},key:'native-probe-phase'});await bridge.request('data.write',{scope:scope??{},key:'native-probe-phase',value:'failed',expectedRevision:phase.revision});}catch{}}
"#;

pub const PARENT: &str = r#"
window.addEventListener('DOMContentLoaded',async()=>{
 const invoke=(command,args)=>window.__TAURI_INTERNALS__.invoke(command,args),pause=ms=>new Promise(resolve=>setTimeout(resolve,ms));
 const proof={passed:false,workflow:'coordinate-book',pluginId:'coordinate-book',scope:'test-only joined native adapter; actual packaged plugin frontend/WebView2/private sidecar; not packaged-app acceptance'};let view;
 const open=async()=>{view=(await invoke('open_plugin_view',{pluginId:'coordinate-book',instanceId:'012345abcdef',bounds:{x:0,y:35,width:1000,height:720},locale:'en-US',theme:'light'})).view_id;};
 const waitPhase=async phase=>{for(let i=0;i<1200;i++){const state=await invoke('get_app_info');if(state.events.some(event=>event.method==='data.write'&&event.result.Ok?.fixturePhase==='failed'))throw Error('child UI workflow failed');if(state.events.some(event=>event.method==='data.write'&&event.result.Ok?.fixturePhase===phase))return state.events;await pause(25);}throw Error('coordinate parent timed out');};
 try{
  await open();await waitPhase('reopen');proof.cleanBeforeClose=await invoke('plugin_view_is_dirty',{viewId:view})===false;await invoke('close_plugin_view',{viewId:view});proof.firstClosed=true;
  await open();const events=await waitPhase('done');
  proof.closedAndReopened=true;
  proof.conflictDenied=events.some(event=>event.method==='data.write'&&event.result.Err==='PLUGIN_DATA_CONFLICT');
  proof.crossGameDenied=events.filter(event=>event.method==='data.read'&&event.result.Err==='PLUGIN_GAME_SCOPE').length>=2;
  proof.completedWrites=events.filter(event=>event.method==='data.write'&&event.result.Ok?.locationCount===1).length;
  proof.deleted=events.some(event=>event.method==='data.remove'&&event.result.Ok!==undefined);
  proof.emptyRead=events.some(event=>event.method==='data.read'&&event.result.Ok?.valuePresent===false&&event.result.Ok?.revision===null);
  await invoke('close_plugin_view',{viewId:view});view=null;
  let handoff=null;const callback=window.__TAURI_INTERNALS__.transformCallback(event=>{handoff=event.payload;});
  await invoke('plugin:event|listen',{event:'plugin-handoff-request',target:{kind:'Any'},handler:callback});
  view=(await invoke('open_plugin_view',{pluginId:'seed-compass',instanceId:'012345abcdef',bounds:{x:0,y:35,width:1000,height:720},locale:'en-US',theme:'light'})).view_id;
  for(let index=0;index<800&&!handoff;index++){const state=await invoke('get_app_info');if(state.events.some(event=>event.requestId==='coordinate-handoff-failed'))throw Error('sender UI workflow failed');await pause(25);}
  if(!handoff||handoff.targetPluginId!=='coordinate-book'||handoff.gameId!=='012345abcdef')throw Error('native handoff event missing or out of scope');
  proof.handoffRouted=true;await invoke('close_plugin_view',{viewId:view});view=null;
  view=(await invoke('open_plugin_view',{pluginId:'coordinate-book',instanceId:'012345abcdef',handoffId:handoff.handoffId,bounds:{x:0,y:35,width:1000,height:720},locale:'en-US',theme:'light'})).view_id;
  const finalEvents=await waitPhase('handoff-saved');proof.handoffTaken=finalEvents.some(event=>event.method==='handoffs.take'&&event.result.Ok?.handoffPresent===true&&event.result.Ok?.senderPluginId==='seed-compass'&&event.result.Ok?.worldScopePresent===true);
  proof.handoffSaved=true;proof.cleanAfterHandoff=await invoke('plugin_view_is_dirty',{viewId:view})===false;
  proof.passed=proof.cleanBeforeClose&&proof.closedAndReopened&&proof.conflictDenied&&proof.crossGameDenied&&proof.completedWrites>=4&&proof.deleted&&proof.emptyRead&&proof.handoffRouted&&proof.handoffTaken&&proof.handoffSaved&&proof.cleanAfterHandoff;
  await invoke('close_plugin_view',{viewId:view});view=null;proof.closed=true;
 }catch(error){proof.error=String(error);if(view)try{await invoke('close_plugin_view',{viewId:view});}catch{}}
 await invoke('get_app_info',{proof});
});
"#;

pub const SENDER: &str = r#"
const bridge=window.__COBBLE_PLUGIN_BRIDGE__,pause=ms=>new Promise(resolve=>setTimeout(resolve,ms));
async function wait(test){for(let index=0;index<800;index++){if(test())return;await pause(25);}throw Error('sender driver timed out');}
try{
 await wait(()=>document.querySelector('[data-field=world] option[value=FixtureWorld]'));
 const world=document.querySelector('[data-field=world]');world.value='FixtureWorld';world.dispatchEvent(new Event('change',{bubbles:true}));
 await wait(()=>document.querySelector('[data-field=seed]').value==='-9223372036854775808'&&!document.querySelector('[data-action=generate]').disabled);
 document.querySelector('[data-action=generate]').click();
 await wait(()=>!document.querySelector('canvas').hidden&&!document.querySelector('.waypoint-handoff').hidden&&!document.querySelector('[data-action=bookmark]').disabled);
 const receiver=document.querySelector('[data-field=waypoint-receiver]');receiver.value='coordinate-book';document.querySelector('[data-action=bookmark]').click();
}catch(error){console.error('Sender probe:',String(error));try{await bridge.request('seedMap.cancel',{requestId:'coordinate-handoff-failed'});}catch{}}
"#;
