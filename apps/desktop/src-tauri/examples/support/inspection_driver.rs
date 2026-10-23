// SPDX-License-Identifier: MIT
//! Isolated joined proof for any real typed inspection workbench and Wasm guest.
pub fn prepare(game: &std::path::Path) -> Result<(), std::io::Error> {
    use std::io::Write;
    fn archive(path: &std::path::Path, name: &str, bytes: &[u8]) -> std::io::Result<()> {
        let mut writer = zip::ZipWriter::new(std::fs::File::create(path)?);
        writer
            .start_file(
                name,
                zip::write::SimpleFileOptions::default()
                    .compression_method(zip::CompressionMethod::Stored),
            )
            .map_err(std::io::Error::other)?;
        writer.write_all(bytes)?;
        writer.finish().map_err(std::io::Error::other)?;
        Ok(())
    }
    std::fs::create_dir_all(game.join("resourcepacks"))?;
    std::fs::create_dir_all(game.join("screenshots"))?;
    std::fs::write(
        game.join("resourcepacks/fixture.txt"),
        b"Original bounded storage fixture",
    )?;
    std::fs::create_dir_all(game.join("mods"))?;
    std::fs::write(game.join("options.txt"),b"key_key.forward:key.keyboard.w\nkey_key.jump:key.keyboard.w\nfov:0.75\nrenderDistance:12\nresourcePacks:[\"vanilla\",\"file/FixturePack\"]\n")?;
    for name in ["first.jar", "duplicate.jar"] {
        archive(&game.join("mods").join(name),"fabric.mod.json",br#"{"schemaVersion":1,"id":"inspection_fixture","version":"1.0.0","name":"Inspection fixture","depends":{"missing_fixture":"*"}}"#)?;
    }
    std::fs::create_dir_all(game.join("resourcepacks/FixturePack"))?;
    std::fs::write(
        game.join("resourcepacks/FixturePack/pack.mcmeta"),
        br#"{"pack":{"pack_format":34,"description":"Original inspection fixture"}}"#,
    )?;
    std::fs::create_dir_all(game.join("saves/FixtureWorld/datapacks/FixtureData"))?;
    std::fs::write(
        game.join("saves/FixtureWorld/level.dat"),
        include_bytes!("inspection-fixtures/level.dat"),
    )?;
    std::fs::write(
        game.join("saves/FixtureWorld/datapacks/FixtureData/pack.mcmeta"),
        br#"{"pack":{"pack_format":48,"description":"Original inspection datapack fixture"}}"#,
    )?;
    std::fs::write(
        game.join("screenshots/fixture.png"),
        include_bytes!("inspection-fixtures/screenshot.png"),
    )?;
    std::fs::write(game.join("servers.dat"),b"\x0a\x00\x00\x09\x00\x07servers\x0a\x00\x00\x00\x01\x08\x00\x04name\x00\x0cFixture home\x08\x00\x02ip\x00\x09localhost\x00\x00")?;
    std::fs::create_dir_all(game.join(".cobble-world-backups/fixture-backup/world"))?;
    std::fs::write(
        game.join(".cobble-world-backups/fixture-backup/world/level.dat"),
        include_bytes!("inspection-fixtures/level.dat"),
    )?;
    std::fs::write(game.join(".cobble-world-backups/fixture-backup/backup.json"),br#"{"folder_name":"FixtureWorld","world_name":"Inspection fixture","created_at":1700000000}"#)?;
    std::fs::create_dir_all(game.join("versions/1.21.1"))?;
    std::fs::write(
        game.join("versions/1.21.1/1.21.1.json"),
        br#"{"javaVersion":{"majorVersion":21}}"#,
    )?;
    Ok(())
}
// The production workbench starts the native snapshot/report workflow itself.
pub const DRIVER: &str = r#"document.documentElement.dataset.nativeProof='inspection';"#;
pub fn parent(plugin_id: &str) -> String {
    PARENT.replace("__PLUGIN_ID__", plugin_id)
}
const PARENT: &str = r#"
window.addEventListener('DOMContentLoaded',async()=>{
 const invoke=(c,a)=>window.__TAURI_INTERNALS__.invoke(c,a),pause=ms=>new Promise(r=>setTimeout(r,ms));let view;const proof={passed:false,workflow:'inspection',pluginId:'__PLUGIN_ID__'};
 try{
 view=(await invoke('open_plugin_view',{pluginId:'__PLUGIN_ID__',instanceId:'012345abcdef',bounds:{x:0,y:35,width:1000,height:720},locale:'en-US',theme:'light'})).view_id;
 let s;for(let i=0;i<1600;i++){s=await invoke('get_app_info');if(s.events.some(e=>e.method==='inspection.check'))break;await pause(25);}
 const find=m=>s.events.find(e=>e.method===m)?.result;
 proof.context=find('context');proof.snapshot=find('inspection.snapshot');proof.report=find('inspection.check');
 const snapshot=proof.snapshot?.Ok,report=proof.report?.Ok;
 proof.passed=proof.context?.Ok?.inspectionGames?.length===1&&snapshot?.schemaVersion===1&&snapshot?.game?.id==='012345abcdef'&&typeof snapshot?.revision==='string'&&Array.isArray(snapshot?.data?.items)&&report?.schema_version===1&&Array.isArray(report?.findings);
 await invoke('close_plugin_view',{viewId:view});proof.closed=true;
 }catch(e){proof.error=String(e);}await invoke('get_app_info',{proof});
});
"#;
