// SPDX-License-Identifier: MIT
//! Synthetic saved world and joined WebView2 proof, never a browser mock.
use serde_json::{Value, json};
use std::path::Path;

pub const VERSION: &str = "1.20.1";

pub fn prepare(game: &Path) -> Result<(), Box<dyn std::error::Error>> {
    fn text(bytes: &mut Vec<u8>, value: &str) {
        bytes.extend_from_slice(&(value.len() as u16).to_be_bytes());
        bytes.extend_from_slice(value.as_bytes());
    }
    fn tag(bytes: &mut Vec<u8>, kind: u8, name: &str) {
        bytes.push(kind);
        text(bytes, name);
    }
    fn string(bytes: &mut Vec<u8>, name: &str, value: &str) {
        tag(bytes, 8, name);
        text(bytes, value);
    }
    let mut nbt = Vec::new();
    tag(&mut nbt, 10, "");
    tag(&mut nbt, 10, "Data");
    string(&mut nbt, "LevelName", "Seed Compass synthetic fixture");
    tag(&mut nbt, 10, "Version");
    string(&mut nbt, "Name", VERSION);
    nbt.push(0);
    for (name, value) in [
        ("DataVersion", 3465_i32),
        ("SpawnX", 96),
        ("SpawnY", 72),
        ("SpawnZ", -128),
    ] {
        tag(&mut nbt, 3, name);
        nbt.extend_from_slice(&value.to_be_bytes());
    }
    tag(&mut nbt, 10, "WorldGenSettings");
    tag(&mut nbt, 4, "seed");
    nbt.extend_from_slice(&i64::MIN.to_be_bytes());
    tag(&mut nbt, 10, "dimensions");
    tag(&mut nbt, 10, "minecraft:overworld");
    tag(&mut nbt, 10, "generator");
    string(&mut nbt, "type", "minecraft:noise");
    string(&mut nbt, "settings", "minecraft:overworld");
    tag(&mut nbt, 10, "biome_source");
    string(&mut nbt, "type", "minecraft:multi_noise");
    string(&mut nbt, "preset", "minecraft:overworld");
    nbt.extend_from_slice(&[0; 7]);
    // Original small gzip fixture: one stored DEFLATE block, IEEE CRC-32.
    let size = u16::try_from(nbt.len())?;
    let mut gzip = vec![0x1f, 0x8b, 8, 0, 0, 0, 0, 0, 0, 255, 1];
    gzip.extend_from_slice(&size.to_le_bytes());
    gzip.extend_from_slice(&(!size).to_le_bytes());
    gzip.extend_from_slice(&nbt);
    let mut crc = u32::MAX;
    for byte in &nbt {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xedb8_8320 & 0_u32.wrapping_sub(crc & 1));
        }
    }
    gzip.extend_from_slice(&(!crc).to_le_bytes());
    gzip.extend_from_slice(&(nbt.len() as u32).to_le_bytes());
    let metadata = cobble_content::read_saved_world_metadata(gzip.as_slice(), "FixtureWorld");
    if metadata.seed.as_deref() != Some("-9223372036854775808")
        || metadata.version.as_deref() != Some(VERSION)
        || metadata.generator.preset.as_deref() != Some("default")
        || metadata.generator.custom != Some(false)
    {
        return Err("synthetic seed fixture failed native metadata validation".into());
    }
    let world = game.join("saves/FixtureWorld");
    std::fs::create_dir_all(&world)?;
    std::fs::write(world.join("level.dat"), gzip)?;
    Ok(())
}

/// Proof logs contain validation facts and map digests, never seed values.
pub fn summarized_result(method: &str, result: &Result<Value, String>) -> Value {
    let Ok(value) = result else {
        return json!(result);
    };
    let summary = match method {
        "worlds.metadata" => json!({
            "schemaVersion":value["schemaVersion"],"worldId":value["worldId"],
            "source":value["source"],"seedPresent":value["seed"].is_string(),
            "fixtureSeedMatches":value["seed"]=="-9223372036854775808",
            "version":value["version"],"generator":value["generator"],
            "spawn":value["spawn"],"unavailable":value["unavailable"]
        }),
        "seedMap.generate" => json!({
            "width":value["width"],"height":value["height"],
            "biomeCount":value["biomes"].as_array().map(Vec::len),
            "paletteCount":value["palette"].as_array().map(Vec::len),
            "biomesSha256":cobble_plugin_host::digest(value["biomes"].to_string().as_bytes()),
            "engineRevision":value["engineRevision"]
        }),
        "seedMap.search" => json!({
            "matchCount":value["matches"].as_array().map(Vec::len),
            "sampledStep":value["sampledStep"],"engineRevision":value["engineRevision"]
        }),
        _ => value.clone(),
    };
    json!({"Ok":summary})
}

pub const DRIVER: &str = r#"
const pause=ms=>new Promise(resolve=>setTimeout(resolve,ms));
async function wait(test){for(let i=0;i<1200;i++){if(test())return;await pause(25);}throw Error('seed native driver timed out');}
const bridge=window.__COBBLE_PLUGIN_BRIDGE__;
let stage='world-ready';
try{
 await wait(()=>document.querySelector('select[data-field=world] option[value=FixtureWorld]'));
 const context=await bridge.request('context',{}),game=context.instanceId;
 if(game!=='012345abcdef'||context.inspectionGames.length!==1)throw Error('bound game context invalid');
 let denied=false;try{await bridge.request('worlds.metadata',{gameId:'fedcba543210',worldId:'FixtureWorld'});}catch(error){denied=String(error).includes('PLUGIN_GAME_SCOPE');}
 if(!denied)throw Error('cross-game seed access was not denied');
 const world=document.querySelector('select[data-field=world]');world.value='FixtureWorld';world.dispatchEvent(new Event('change'));
 stage='metadata-ready';
 await wait(()=>document.querySelector('input[data-field=seed]').value==='-9223372036854775808'&&!document.querySelector('button[data-action=generate]').disabled);
 if(document.querySelector('select[data-field=version]').value!=='1.20.1')throw Error('saved version not selected');
 if(document.querySelector('input[data-field=x]').value!=='96'||document.querySelector('input[data-field=z]').value!=='-128')throw Error('saved spawn not selected');
 stage='map-submit';document.querySelector('button[data-action=generate]').click();
 await wait(()=>!document.querySelector('canvas').hidden&&document.querySelectorAll('.legend span').length>0&&document.querySelector('.seed-compass').getAttribute('aria-busy')==='false');
 const canvas=document.querySelector('canvas'),pixels=canvas.getContext('2d').getImageData(0,0,canvas.width,canvas.height).data;
 if(canvas.width!==128||pixels.every(value=>value===0))throw Error('real raster not drawn');
 stage='structure-search';const target=document.querySelector('select[data-field=target]');target.value='structure:village';
 document.querySelector('select[data-field=radius]').value='2048';document.querySelector('button[data-action=search]').click();
 await wait(()=>document.querySelector('.search-panel>.muted').textContent.includes('terrain')&&document.querySelector('.seed-compass').getAttribute('aria-busy')==='false');
 stage='cancel-search';const common={seed:document.querySelector('input[data-field=seed]').value,version:'1.20.1',dimension:'overworld',preset:'default',y:64,x:96,z:-128};
 const work=bridge.request('seedMap.search',{requestId:'probe-cancel-search',search:{...common,radius:8192,target:{kind:'biome',id:'1'},limit:16}}).then(()=>({completed:true}),error=>({cancelled:String(error).includes('SEED_MAP_CANCELLED')}));
 await pause(25);const cancelled=await bridge.request('seedMap.cancel',{requestId:'probe-cancel-search'}),outcome=await work;
 if(cancelled!==true||outcome.cancelled!==true)throw Error('running native search did not cancel');
 // Cancellation resolution must release the worker's slot before this request.
 const next=await bridge.request('seedMap.generate',{requestId:'probe-after-cancel',map:{...common,width:8,height:8,step:4}});
 if(next.biomes.length!==64||!next.engineRevision)throw Error('cancelled worker retained its slot');
 await bridge.request('seedMap.cancel',{requestId:'probe-finished'});
}catch(error){console.error('Seed probe:',String(error));try{const field=name=>document.querySelector(`[data-field=${name}]`),flag=value=>value?'1':'0';await bridge.request('seedMap.cancel',{requestId:`probe-state-s${flag(field('seed')?.value==='-9223372036854775808')}-g${flag(document.querySelector('[data-action=generate]')?.disabled)}-v${flag(field('version')?.value==='1.20.1')}-d${flag(field('dimension')?.value==='overworld')}-p${flag(field('preset')?.value==='default')}-b${flag(document.querySelector('.seed-compass')?.getAttribute('aria-busy')==='true')}`});await bridge.request('seedMap.cancel',{requestId:`probe-failed-${stage}`});}catch{}}
"#;

pub const PARENT: &str = r#"
window.addEventListener('DOMContentLoaded',async()=>{
 const invoke=(command,args)=>window.__TAURI_INTERNALS__.invoke(command,args),pause=ms=>new Promise(resolve=>setTimeout(resolve,ms));let view;
 const proof={passed:false,workflow:'seed-compass',pluginId:'seed-compass',version:'1.20.1',scope:'test-only joined native adapter; actual packaged plugin frontend/WebView2/native metadata/Cubiomes; not packaged-app acceptance'};
 try{
 view=(await invoke('open_plugin_view',{pluginId:'seed-compass',instanceId:'012345abcdef',bounds:{x:0,y:35,width:1000,height:720},locale:'en-US',theme:'light'})).view_id;
 let state;for(let i=0;i<1600;i++){state=await invoke('get_app_info');if(state.events.some(event=>event.requestId==='probe-finished'||event.requestId?.startsWith('probe-failed-')))break;await pause(25);}
 const events=state.events,ok=method=>events.filter(event=>event.method===method&&event.result.Ok!==undefined).map(event=>event.result.Ok);
 const metadata=ok('worlds.metadata').find(value=>value.fixtureSeedMatches),maps=ok('seedMap.generate'),searches=ok('seedMap.search');
 proof.savedMetadata=metadata;proof.maps=maps;proof.searches=searches;
 proof.requestedVersions=events.filter(event=>['seedMap.generate','seedMap.search'].includes(event.method)).map(event=>event.requestedVersion);
 const catalog=ok('seedMap.capabilities')[0]?.versionCatalogs.find(value=>value.version==='1.20.1');
 proof.capabilityMapping=catalog?{version:catalog.version,engineVersion:catalog.engineVersion,mapping:catalog.mapping}:null;
 proof.crossGameDenied=events.some(event=>event.method==='worlds.metadata'&&event.result.Err==='PLUGIN_GAME_SCOPE');
 proof.cancelled=events.some(event=>event.method==='seedMap.cancel'&&event.requestId==='probe-cancel-search'&&event.result.Ok===true)&&events.some(event=>event.method==='seedMap.search'&&event.result.Err==='SEED_MAP_CANCELLED');
 proof.passed=!!catalog&&!!metadata&&metadata.source==='saved_metadata'&&metadata.version==='1.20.1'&&metadata.generator.preset==='default'&&metadata.generator.custom===false&&metadata.spawn.x===96&&metadata.spawn.z===-128&&maps.some(value=>value.biomeCount===16384&&value.paletteCount>0&&value.engineRevision)&&maps.some(value=>value.biomeCount===64)&&searches.some(value=>value.engineRevision)&&proof.requestedVersions.length===4&&proof.requestedVersions.every(value=>value==='1.20.1')&&proof.crossGameDenied&&proof.cancelled&&events.some(event=>event.requestId==='probe-finished');
 await invoke('close_plugin_view',{viewId:view});proof.closed=true;
 }catch(error){proof.error=String(error);if(view)try{await invoke('close_plugin_view',{viewId:view});}catch{}}
 await invoke('get_app_info',{proof});
});
"#;
