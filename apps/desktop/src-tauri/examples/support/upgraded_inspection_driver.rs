// SPDX-License-Identifier: MIT
//! Native-only synthetic fixtures and UI driver for upgraded inspection tools.
use std::path::Path;
pub fn game_id(data: &Path) -> String {
    cobble_plugin_host::digest(data.to_string_lossy().as_bytes())
}
pub struct OwnedCredential(keyring::Entry);
impl Drop for OwnedCredential {
    fn drop(&mut self) {
        let _ = self.0.delete_credential();
    }
}
pub fn snapshots(data: &Path, game: &Path) -> Result<OwnedCredential, Box<dyn std::error::Error>> {
    let id = game_id(data);
    let entry = keyring::Entry::new("CobbleLauncher.WorldSnapshots", &id)?;
    match entry.get_password() {
        Err(keyring::Error::NoEntry) => {}
        _ => return Err("refusing preexisting fixture credential".into()),
    }
    entry.set_password("native-probe-synthetic-password")?;
    let owned = OwnedCredential(entry);
    std::fs::write(data.join("owned-snapshot-credential"), &id)?;
    let root = data.join("CobbleLauncher/world-snapshots").join(&id);
    std::fs::create_dir_all(root.parent().unwrap())?;
    let mut repository =
        cobble_core::WorldSnapshotRepository::create(&root, "native-probe-synthetic-password")?;
    let region = game.join("saves/FixtureWorld/region");
    std::fs::create_dir_all(&region)?;
    std::fs::write(region.join("fixture.mca"), b"original synthetic generation")?;
    cobble_core::create_world_snapshot(game, "FixtureWorld", &mut repository, "Before")
        .map_err(|error| error.to_string())?;
    std::fs::write(region.join("fixture.mca"), b"second synthetic generation")?;
    std::fs::write(region.join("added.dat"), b"new synthetic file")?;
    cobble_core::create_world_snapshot(game, "FixtureWorld", &mut repository, "After")
        .map_err(|error| error.to_string())?;
    std::fs::write(
        region.join("fixture.mca"),
        b"live synthetic generation remains untouched",
    )?;
    Ok(owned)
}
pub fn cleanup(data: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let id = game_id(data);
    if !data.join("owned-snapshot-credential").exists() {
        return Ok(());
    }
    if std::fs::read_to_string(data.join("owned-snapshot-credential"))? != id {
        return Err("fixture credential marker mismatch".into());
    }
    let entry = keyring::Entry::new("CobbleLauncher.WorldSnapshots", &id)?;
    match entry.get_password() {
        Err(keyring::Error::NoEntry) => {}
        Ok(password) if password == "native-probe-synthetic-password" => {
            entry.delete_credential()?
        }
        _ => return Err("refusing credential that is not owned by this fixture".into()),
    }
    std::fs::remove_file(data.join("owned-snapshot-credential"))?;
    Ok(())
}
pub fn parent(plugin: &str, id: &str) -> String {
    PARENT.replace("__PLUGIN__", plugin).replace("__GAME__", id)
}
pub const DRIVER: &str = r#"
const pause=ms=>new Promise(r=>setTimeout(r,ms));
async function wait(test){for(let i=0;i<1600;i++){if(test())return;await pause(25);}throw Error('native driver timed out');}
const bridge=window.__COBBLE_PLUGIN_BRIDGE__;
await wait(()=>document.querySelector('.time-machine')||document.querySelector('.dependency-workspace'));
const context=await bridge.request('context',{}),game=context.instanceId;
if(context.inspectionGames.length!==1||context.inspectionGames[0].id!==game)throw Error('bound context leaked another game');
let denied=false;try{await bridge.request('inspection.snapshot',{gameId:'fedcba543210',scope:document.querySelector('.time-machine')?'worlds':'mods'});}catch(e){denied=String(e).includes('PLUGIN_GAME_SCOPE');}
if(!denied)throw Error('other-game scope was not denied');
if(document.querySelector('.time-machine')){
 await wait(()=>document.querySelectorAll('.snapshot-pick').length===2);
 document.querySelectorAll('.snapshot-row input')[0].click();document.querySelectorAll('.snapshot-row input')[1].click();
 [...document.querySelectorAll('button')].find(b=>b.textContent.startsWith('Compare selected')).click();
 await wait(()=>document.querySelectorAll('.changed-table tbody tr').length>=2);
 [...document.querySelectorAll('button')].find(b=>b.textContent==='← Snapshots').click();
 denied=false;try{await bridge.request('worldSnapshots.list',{gameId:'fedcba543210',worldId:'FixtureWorld'});}catch(e){denied=String(e).includes('PLUGIN_GAME_SCOPE');}
 if(!denied)throw Error('other-game snapshot access was not denied');
 document.querySelector('.recovery-action').click();
 await wait(()=>document.querySelector('[role=status]')?.textContent==='Recovery request sent.');
}else{
 await wait(()=>document.querySelectorAll('.mod-row').length===2&&document.querySelector('.graph-node'));
 const search=document.querySelector('input[type=search]');search.value='missing_fixture';search.dispatchEvent(new Event('input'));
 await wait(()=>document.querySelector('.dependency-map').textContent.includes('missing_fixture'));
 search.value='';search.dispatchEvent(new Event('input'));
}
await bridge.request('context',{});
"#;
const PARENT: &str = r#"
window.addEventListener('DOMContentLoaded',async()=>{
 const invoke=(c,a)=>window.__TAURI_INTERNALS__.invoke(c,a),pause=ms=>new Promise(r=>setTimeout(r,ms));
 const proof={passed:false,workflow:'upgraded-inspection',pluginId:'__PLUGIN__',gameId:'__GAME__',restoreEvents:[],scope:'test-only native adapter; real Wasm/native frontend bridge/WebView2; not packaged-app evidence'};let view;
 try{
 for(let i=0;i<200;i++){const games=await invoke('list_games');if(games.every(game=>game.launch_state==='idle'))break;await pause(25);}
 const callback=window.__TAURI_INTERNALS__.transformCallback(e=>proof.restoreEvents.push(e.payload));
 await invoke('plugin:event|listen',{event:'plugin-open-snapshot-restore',target:{kind:'Any'},handler:callback});
 view=(await invoke('open_plugin_view',{pluginId:'__PLUGIN__',instanceId:'__GAME__',bounds:{x:0,y:35,width:1000,height:720},locale:'en-US',theme:'light'})).view_id;
 let s;const deadline=performance.now()+40000;while(performance.now()<deadline){s=await invoke('get_app_info');if(s.events.filter(e=>e.method==='context'&&e.result.Ok).length>=3&&s.events.some(e=>e.method==='inspection.check'&&e.result.Ok))break;await pause(75);}
 proof.events=s.events;
 const passed=s.events.some(e=>e.method==='inspection.check'&&e.result.Ok?.schema_version===1)&&s.events.some(e=>e.method==='inspection.snapshot'&&e.result.Err==='PLUGIN_GAME_SCOPE')&&s.events.filter(e=>e.method==='context'&&e.result.Ok).length>=3;
 if('__PLUGIN__'==='world-dashboard'){
   const comparison=s.events.find(e=>e.method==='worldSnapshots.compare'&&e.result.Ok)?.result.Ok;
   const listed=s.events.find(e=>e.method==='worldSnapshots.list'&&Array.isArray(e.result.Ok))?.result.Ok??[];
   proof.passed=passed&&comparison?.modified>=1&&comparison?.added>=1&&s.events.some(e=>e.method==='worldSnapshots.list'&&e.result.Err==='PLUGIN_GAME_SCOPE')&&proof.restoreEvents.length===1&&proof.restoreEvents[0].game_id==='__GAME__'&&proof.restoreEvents[0].world_id==='FixtureWorld'&&proof.restoreEvents[0].plugin_id==='world-dashboard'&&listed.some(snapshot=>snapshot.id===proof.restoreEvents[0].snapshot_id)&&s.events.some(e=>e.method==='worldSnapshots.openRestore'&&e.result.Ok?.requested===true);
 }else proof.passed=passed;
 await invoke('close_plugin_view',{viewId:view});proof.closed=true;
 }catch(e){proof.error=String(e);}await invoke('get_app_info',{proof});
});
"#;
