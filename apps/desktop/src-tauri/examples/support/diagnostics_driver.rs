// SPDX-License-Identifier: MIT
//! Separate indexed UI driver for the real Crash Doctor component/native broker.
pub fn prepare(game: &std::path::Path) -> Result<(), std::io::Error> {
    std::fs::create_dir_all(game.join("logs"))?;
    std::fs::create_dir_all(game.join("crash-reports"))?;
    std::fs::write(game.join("logs/latest.log"), b"Original synthetic fixture\njava.lang.OutOfMemoryError: Java heap space\nUnsupportedClassVersionError: class file version 65.0\n")?;
    std::fs::write(
        game.join("crash-reports/crash-fixture.txt"),
        b"Original synthetic crash report\njava.lang.OutOfMemoryError: Java heap space\n",
    )?;
    Ok(())
}
pub const DRIVER: &str = r#"
const pause=ms=>new Promise(r=>setTimeout(r,ms));
async function wait(test){for(let i=0;i<800;i++){if(test())return;await pause(25);}throw Error('diagnostic driver timed out');}
await wait(()=>document.querySelector('#files li button')&&!document.querySelector('#check').disabled);
document.querySelector('#check').click();
await wait(()=>document.querySelector('#findings article'));
"#;
pub const PARENT: &str = r#"
window.addEventListener('DOMContentLoaded',async()=>{
 const invoke=(c,a)=>window.__TAURI_INTERNALS__.invoke(c,a),pause=ms=>new Promise(r=>setTimeout(r,ms));let view;const proof={passed:false,workflow:'diagnostics'};
 try{
 view=(await invoke('open_plugin_view',{pluginId:'crash-doctor',instanceId:'012345abcdef',bounds:{x:0,y:35,width:1000,height:720},locale:'en-US',theme:'light'})).view_id;
 let s;for(let i=0;i<1200;i++){s=await invoke('get_app_info');if(s.events.some(e=>e.method==='diagnostics.check'))break;await pause(25);}
 const find=m=>s.events.find(e=>e.method===m)?.result;
 proof.context=find('context');proof.files=find('diagnostics.list');proof.document=find('diagnostics.read');proof.report=find('diagnostics.check');
 const files=proof.files?.Ok,doc=proof.document?.Ok,report=proof.report?.Ok;
 proof.passed=proof.context?.Ok?.recentGames?.length===1&&files?.length===2&&!!doc?.revision&&doc?.name?.startsWith('crash-reports/')&&report?.findings?.some(f=>f.code==='java_heap_exhausted');
 await invoke('close_plugin_view',{viewId:view});proof.closed=true;
 }catch(e){proof.error=String(e);}await invoke('get_app_info',{proof});
});
"#;
