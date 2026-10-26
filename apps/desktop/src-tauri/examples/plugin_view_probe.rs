// SPDX-License-Identifier: MIT
//! Standalone native browser boundary probe. Never loads account/game libraries.
#![allow(dead_code)]
#[path = "../src/plugin_views.rs"]
mod plugin_views;
use serde_json::{Value, json};
use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::Duration,
};
use tauri::Manager;
static ROOT: OnceLock<tempfile::TempDir> = OnceLock::new();
static PROOF: Mutex<Option<Value>> = Mutex::new(None);
static ACTUAL: AtomicBool = AtomicBool::new(false);
static MAIN_CONTROL: AtomicBool = AtomicBool::new(false);
static MAIN_NETWORK: AtomicBool = AtomicBool::new(false);
static CHILD_COMMAND_HITS: AtomicUsize = AtomicUsize::new(0);
static ACL_DENIED: AtomicBool = AtomicBool::new(false);
static TCP_HITS: AtomicUsize = AtomicUsize::new(0);
static UDP_HITS: AtomicUsize = AtomicUsize::new(0);
fn valid_id(id: &str) -> Result<(), String> {
    if id
        .bytes()
        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
    {
        Ok(())
    } else {
        Err("invalid".into())
    }
}
fn root(name: &str) -> Result<PathBuf, String> {
    Ok(ROOT.get().ok_or("probe root")?.path().join(name))
}
fn registry() -> Result<cobble_plugin_host::PluginRegistry, String> {
    Ok(cobble_plugin_host::PluginRegistry::new(root("plugins")?))
}
mod games {
    pub(crate) fn ensure_registered_game(_: &tauri::AppHandle, _: &str) -> Result<(), String> {
        Err("No game context in security probe".into())
    }
}
mod plugin_lang_package {
    pub fn revoke_session(_: &str) {}
}
mod plugin_mesh_events {
    pub fn revoke_session(_: &str) {}
}
mod plugin_handoffs {
    pub fn authorize_view(_: &str, _: Option<&str>, _: Option<&str>) -> Result<(), String> {
        Err("No handoff in security probe".into())
    }
}
#[tauri::command]
fn get_app_info(webview: tauri::Webview) -> Value {
    eprintln!("probe main-ACL command reached by {}", webview.label());
    if webview.label() == "main" {
        MAIN_CONTROL.store(true, Ordering::Release);
    } else {
        CHILD_COMMAND_HITS.fetch_add(1, Ordering::Relaxed);
    }
    json!({"probe":true})
}
#[tauri::command]
async fn frontend_call(
    app: tauri::AppHandle,
    webview: tauri::Webview,
    request: Value,
) -> Result<Value, String> {
    let auth = plugin_views::authorize_view(&webview)?;
    eprintln!("probe frontend broker reached: {}", request["method"]);
    match request["method"].as_str() {
        Some("report.cancel") => {
            ACL_DENIED.store(request["params"]["aclDenied"] == true, Ordering::Release);
            Ok(json!({}))
        }
        Some("context") => {
            if auth.manifest.id == "config-doctor" && !ACTUAL.swap(true, Ordering::AcqRel) {
                if let Some(child) = app.get_webview(&auth.session_id) {
                    let _ = child.eval("window.__TAURI_INTERNALS__.invoke('get_app_info').then(()=>window.__TAURI_INTERNALS__.invoke('frontend_call',{request:{method:'report.cancel',params:{aclDenied:false}}}),()=>window.__TAURI_INTERNALS__.invoke('frontend_call',{request:{method:'report.cancel',params:{aclDenied:true}}}));");
                }
                let handle = app.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(Duration::from_millis(700));
                    if let Some(main) = handle.get_webview("main") {
                        let _ = main.eval("window.startProbe()");
                    }
                });
            }
            Ok(json!({"locale":"en","theme":"light","instanceId":null,"games":[]}))
        }
        Some("report.check") => {
            *PROOF.lock().unwrap() = Some(request["params"].clone());
            let handle = app.clone();
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(500));
                eprintln!(
                    "PROBE_RECEIPT {}",
                    json!({"actual":ACTUAL.load(Ordering::Acquire),"main":MAIN_CONTROL.load(Ordering::Acquire),"mainNetwork":MAIN_NETWORK.load(Ordering::Acquire),"aclDenied":ACL_DENIED.load(Ordering::Acquire),"childExecutions":CHILD_COMMAND_HITS.load(Ordering::Acquire),"tcp":TCP_HITS.load(Ordering::Acquire),"udp":UDP_HITS.load(Ordering::Acquire),"frame":*PROOF.lock().unwrap()})
                );
                let frame = PROOF.lock().unwrap().clone().unwrap_or(Value::Null);
                let safe = ACTUAL.load(Ordering::Acquire)
                    && MAIN_CONTROL.load(Ordering::Acquire)
                    && MAIN_NETWORK.load(Ordering::Acquire)
                    && ACL_DENIED.load(Ordering::Acquire)
                    && CHILD_COMMAND_HITS.load(Ordering::Acquire) == 0
                    && TCP_HITS.load(Ordering::Acquire) == 0
                    && UDP_HITS.load(Ordering::Acquire) == 0
                    && frame["parentDOM"] == false
                    && frame["storage"] == false
                    && frame["bridge"] == true
                    && frame["origin"] == "null"
                    && (frame["rawPrivilegedOutcome"] == "transport-blocked"
                        || frame["rawPrivilegedOutcome"] == "unavailable")
                    && frame["unknownMethodDenied"] == true
                    && frame["rtc"] == cfg!(windows);
                handle.exit(if safe { 0 } else { 3 });
            });
            Ok(json!({"accepted":true}))
        }
        _ => Err("PROBE_METHOD_DENIED".into()),
    }
}
fn package_probe(
    source: &std::path::Path,
    tcp: u16,
    udp: u16,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let path = ROOT.get().unwrap().path().join("fixture");
    std::fs::create_dir_all(path.join("ui"))?;
    let mut manifest: Value = serde_json::from_slice(&std::fs::read(source.join("plugin.json"))?)?;
    manifest["id"] = json!("frontend-security-probe");
    manifest["name"] = json!("Frontend security probe");
    manifest.as_object_mut().unwrap().remove("contributions");
    manifest["permissions"]
        .as_array_mut()
        .unwrap()
        .retain(|node| node["permission"] != "launcher:settings/read");
    std::fs::copy(source.join("component.wasm"), path.join("component.wasm"))?;
    let html=b"<!doctype html><html><head><meta charset=\"utf-8\"><script type=\"module\" src=\"./probe.js\"></script></head><body>Security probe</body></html>";
    let script = PROBE_SCRIPT
        .replace("__TCP__", &tcp.to_string())
        .replace("__UDP__", &udp.to_string());
    std::fs::write(path.join("ui/index.html"), html)?;
    std::fs::write(path.join("ui/probe.js"), script.as_bytes())?;
    let index = serde_json::to_vec(
        &json!({"schema_version":1,"files":[{"path":"ui/index.html","sha256":cobble_plugin_host::digest(html),"bytes":html.len()},{"path":"ui/probe.js","sha256":cobble_plugin_host::digest(script.as_bytes()),"bytes":script.len()}]}),
    )?;
    manifest["frontend"]["assets_sha256"] = json!(cobble_plugin_host::digest(&index));
    std::fs::write(path.join("frontend-assets.json"), index)?;
    std::fs::write(path.join("plugin.json"), serde_json::to_vec(&manifest)?)?;
    Ok(path)
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    ROOT.set(
        tempfile::Builder::new()
            .prefix("cobble-plugin-webview-probe-")
            .tempdir()?,
    )
    .map_err(|_| "root")?;
    let source = PathBuf::from(
        std::env::args()
            .nth(1)
            .ok_or("Pass actual Config Doctor frontend package path")?,
    );
    let tcp = std::net::TcpListener::bind("127.0.0.1:0")?;
    tcp.set_nonblocking(true)?;
    let tcp_port = tcp.local_addr()?.port();
    let udp = std::net::UdpSocket::bind("127.0.0.1:0")?;
    udp.set_nonblocking(true)?;
    let udp_port = udp.local_addr()?.port();
    let stop = Arc::new(AtomicBool::new(false));
    let stop_tcp = stop.clone();
    let stop_udp = stop.clone();
    std::thread::spawn(move || {
        while !stop_tcp.load(Ordering::Acquire) {
            if let Ok((mut stream, _)) = tcp.accept() {
                let _ = stream.set_read_timeout(Some(Duration::from_millis(100)));
                let mut bytes = [0; 2048];
                if let Ok(count) = std::io::Read::read(&mut stream, &mut bytes) {
                    let main_control = bytes[..count].starts_with(b"GET /main-control ");
                    if main_control {
                        MAIN_NETWORK.store(true, Ordering::Release);
                    } else {
                        TCP_HITS.fetch_add(1, Ordering::Relaxed);
                    }
                    eprintln!(
                        "probe observed HTTP {}",
                        String::from_utf8_lossy(&bytes[..count])
                            .lines()
                            .next()
                            .unwrap_or("")
                    );
                    let _ = std::io::Write::write_all(&mut stream, b"HTTP/1.1 200 OK\r\nAccess-Control-Allow-Origin: *\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok");
                }
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    });
    std::thread::spawn(move || {
        let mut bytes = [0; 4096];
        while !stop_udp.load(Ordering::Acquire) {
            if udp.recv_from(&mut bytes).is_ok() {
                UDP_HITS.fetch_add(1, Ordering::Relaxed);
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    });
    let fixture = package_probe(&source, tcp_port, udp_port)?;
    registry()?.install_directory(&source)?;
    registry()?.install_directory(&fixture)?;
    plugin_views::enable_isolated_probe()?;
    let mut context = tauri::generate_context!();
    context.config_mut().app.windows.clear();
    let data = ROOT.get().unwrap().path().join("webview-data");
    let app = tauri::Builder::default()
        .register_uri_scheme_protocol("cobble-probe", |_, _| tauri::http::Response::builder().header("Content-Type", "text/html; charset=utf-8").body(b"<!doctype html><html><body>Isolated frontend boundary probe</body></html>".to_vec()).unwrap())
        .register_uri_scheme_protocol("cobble-plugin", plugin_views::serve_asset)
        .invoke_handler(tauri::generate_handler![
            frontend_call,
            get_app_info,
            plugin_views::open_plugin_view,
            plugin_views::resize_plugin_view,
            plugin_views::close_plugin_view,
            plugin_views::set_plugin_view_visible
        ])
        .setup(move |app| {
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_secs(30));
                eprintln!("PROBE_TIMEOUT {}", json!({"actual":ACTUAL.load(Ordering::Acquire),"main":MAIN_CONTROL.load(Ordering::Acquire),"aclDenied":ACL_DENIED.load(Ordering::Acquire),"tcp":TCP_HITS.load(Ordering::Acquire),"udp":UDP_HITS.load(Ordering::Acquire)}));
                handle.exit(2);
            });
            let url = if cfg!(windows) {
                "http://cobble-probe.localhost/index.html"
            } else {
                "cobble-probe://localhost/index.html"
            };
            tauri::WebviewWindowBuilder::new(
                app,
                "main",
                tauri::WebviewUrl::External(url.parse()?),
            )
            .on_page_load(|_, page| eprintln!("probe main page {:?}", page.event()))
            .title("Cobble isolated frontend probe")
            .inner_size(680.0, 560.0)
            .data_directory(data)
            .on_web_resource_request(|_, response| {
                *response.status_mut() = tauri::http::StatusCode::OK;
                *response.body_mut() = std::borrow::Cow::Owned(
                    b"<!doctype html><html><body>Isolated frontend boundary probe</body></html>"
                        .to_vec(),
                );
            })
            .initialization_script(PARENT_SCRIPT.replace("__TCP__", &tcp_port.to_string()))
            .build()?;
            Ok(())
        })
        .build(context)?;
    app.run(|_, _| {});
    stop.store(true, Ordering::Release);
    let proof = PROOF.lock().unwrap().clone();
    let result = json!({"actual_config_doctor_frontend_context":ACTUAL.load(Ordering::Acquire),"main_control":MAIN_CONTROL.load(Ordering::Acquire),"main_network_unaffected":MAIN_NETWORK.load(Ordering::Acquire),"child_privileged_executions":CHILD_COMMAND_HITS.load(Ordering::Acquire),"child_acl_denied":ACL_DENIED.load(Ordering::Acquire),"tcp_hits":TCP_HITS.load(Ordering::Acquire),"udp_hits":UDP_HITS.load(Ordering::Acquire),"frame":proof});
    println!("{}", serde_json::to_string_pretty(&result)?);
    if !ACTUAL.load(Ordering::Acquire) || proof.is_none() {
        return Err("Frontend runtime probe did not finish".into());
    }
    let frame = proof.as_ref().unwrap();
    if frame["parentDOM"] != false
        || frame["storage"] != false
        || frame["bridge"] != true
        || frame["origin"] != "null"
        || frame["tauri"] != "undefined"
        || !MAIN_CONTROL.load(Ordering::Acquire)
        || !MAIN_NETWORK.load(Ordering::Acquire)
        || CHILD_COMMAND_HITS.load(Ordering::Acquire) != 0
        || !ACL_DENIED.load(Ordering::Acquire)
    {
        return Err("DOM/storage/native ACL boundary probe failed".into());
    }
    if TCP_HITS.load(Ordering::Acquire) > 0 || UDP_HITS.load(Ordering::Acquire) > 0 {
        return Err("Browser egress escaped plugin policy".into());
    }
    Ok(())
}
const PARENT_SCRIPT: &str = r#"
window.startProbe=async()=>{await fetch('http://127.0.0.1:__TCP__/main-control');const v=await window.__TAURI_INTERNALS__.invoke('open_plugin_view',{pluginId:'frontend-security-probe',instanceId:null,bounds:{x:0,y:30,width:680,height:500},locale:'en-US',theme:'light'});await new Promise(r=>setTimeout(r,300));return window.__TAURI_INTERNALS__.invoke('resize_plugin_view',{viewId:v.view_id,bounds:{x:16,y:45,width:400,height:300}}).catch(console.error)};
window.addEventListener('DOMContentLoaded',async()=>{await window.__TAURI_INTERNALS__.invoke('get_app_info');return window.__TAURI_INTERNALS__.invoke('open_plugin_view',{pluginId:'config-doctor',instanceId:null,bounds:{x:0,y:30,width:680,height:500},locale:'en-US',theme:'light'}).catch(console.error)});
"#;
const PROBE_SCRIPT: &str = r#"
const proof={loaded:true,origin:window.origin,bridge:typeof window.__COBBLE_PLUGIN_BRIDGE__?.request==='function',tauri:typeof window.__TAURI_INTERNALS__,rawInvoke:typeof window.__TAURI_INTERNALS__?.invoke};
try{void window.top.document;proof.parentDOM=true}catch{proof.parentDOM=false}
try{localStorage.getItem('x');proof.storage=true}catch{proof.storage=false}
if(proof.rawInvoke==='function'){
 proof.rawPrivilegedOutcome=await Promise.race([window.__TAURI_INTERNALS__.invoke('get_app_info').then(()=> 'executed',()=> 'rejected'),new Promise(resolve=>setTimeout(()=>resolve('transport-blocked'),300))]);
}else{proof.rawPrivilegedOutcome='unavailable'}
try{await window.__COBBLE_PLUGIN_BRIDGE__.request('unknown.method',{});proof.unknownMethodDenied=false}catch{proof.unknownMethodDenied=true}
const endpoint='http://127.0.0.1:__TCP__/proof';
fetch(endpoint).catch(()=>{});const image=new Image();image.src=endpoint+'/image';document.body.append(image);
try{new WebSocket('ws://127.0.0.1:__TCP__/ws')}catch{}
try{window.open(endpoint+'/popup')}catch{}
try{const worker=new Worker(URL.createObjectURL(new Blob(['fetch('+JSON.stringify(endpoint) +')'],{type:'text/javascript'})));proof.worker=true;worker.terminate()}catch{proof.worker=false}
try{const pc=new RTCPeerConnection({iceServers:[{urls:'stun:127.0.0.1:__UDP__'}]});proof.rtc=true;pc.createDataChannel('probe');pc.createOffer().then(offer=>pc.setLocalDescription(offer));setTimeout(()=>pc.close(),1700)}catch{proof.rtc=false}
setTimeout(async()=>{await window.__COBBLE_PLUGIN_BRIDGE__.request('report.check',proof);window.location.href=endpoint+'/navigate';},1800);
"#;
