// SPDX-License-Identifier: MIT
//! Explicit local two-companion fixture. Never run in ordinary CI or with user games.
use base64::Engine;
use cobble_core::game_transfer::{self, Selection};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    process::{Child, ChildStdin, ChildStdout, Command, Stdio},
    sync::atomic::AtomicBool,
    time::{SystemTime, UNIX_EPOCH},
};

struct Companion {
    child: Child,
    input: ChildStdin,
    output: BufReader<ChildStdout>,
    next: u64,
}
impl Companion {
    fn start(executable: &Path, directory: &Path, name: &str) -> Self {
        let mut child = Command::new(executable)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let input = child.stdin.take().unwrap();
        let output = BufReader::new(child.stdout.take().unwrap());
        let mut companion = Self {
            child,
            input,
            output,
            next: 1,
        };
        companion.ask(json!({"op":"app_initialize","directory":directory,"display_name":name}));
        companion
    }
    fn ask(&mut self, command: Value) -> Value {
        let id = self.next;
        self.next += 1;
        let payload = serde_json::to_vec(&json!({"version":1,"id":id,"command":command})).unwrap();
        self.input.write_all(&payload).unwrap();
        self.input.write_all(b"\n").unwrap();
        self.input.flush().unwrap();
        for _ in 0..128 {
            let mut line = String::new();
            assert!(
                self.output.read_line(&mut line).unwrap() > 0,
                "companion exited"
            );
            let reply: Value = serde_json::from_str(&line).unwrap();
            if reply["id"].as_u64() != Some(id) {
                continue;
            }
            assert_eq!(reply["ok"], true, "{reply}");
            return reply["result"].clone();
        }
        panic!("companion response was not received")
    }
}
impl Drop for Companion {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
fn hex(bytes: impl AsRef<[u8]>) -> String {
    bytes.as_ref().iter().map(|b| format!("{b:02x}")).collect()
}
fn companion_binary() -> PathBuf {
    let name = if cfg!(windows) {
        "cobble-mesh-companion.exe"
    } else {
        "cobble-mesh-companion"
    };
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../cobble-mesh-launcher-companion/target/debug")
        .join(name)
}

#[test]
#[ignore = "explicit local two-peer fixture; requires built sibling companion"]
fn selected_game_data_crosses_real_peers_then_core_verifies_independent_target() {
    let executable = companion_binary();
    assert!(
        executable.is_file(),
        "build the sibling companion test binary first"
    );
    let root = std::env::temp_dir().join(format!(
        "cobble-game-transfer-peer-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&root).unwrap();
    let source = root.join("source");
    let local = root.join("local");
    let incoming = root.join("incoming");
    for path in [&source, &local, &incoming] {
        fs::create_dir(path).unwrap();
    }
    fs::create_dir_all(source.join("saves/World")).unwrap();
    fs::create_dir_all(source.join("mods")).unwrap();
    fs::create_dir_all(local.join("mods")).unwrap();
    let world = vec![0x52; 70_001];
    fs::write(source.join("saves/World/level.dat"), &world).unwrap();
    fs::write(source.join("mods/custom.jar"), b"owned fixture mod").unwrap();
    fs::write(local.join("mods/custom.jar"), b"owned fixture mod").unwrap();
    fs::write(source.join("accounts.json"), b"private fixture").unwrap();
    let cancel = AtomicBool::new(false);
    let manifest = game_transfer::prepare(
        &source,
        "1.21.1",
        None,
        None,
        Selection {
            saves: true,
            mods: true,
            config: false,
            options: false,
            resourcepacks: false,
            shaderpacks: false,
        },
        &cancel,
    )
    .unwrap();
    assert_eq!(manifest.files.len(), 2);
    let manifest_id = hex(Sha256::digest(b"one-time fixture manifest"));
    let mut host = Companion::start(&executable, &root.join("host"), "Host");
    let mut guest = Companion::start(&executable, &root.join("guest"), "Guest");
    let invitation = host.ask(json!({"op":"app_invite"}))["invitation"]
        .as_str()
        .unwrap()
        .to_owned();
    guest.ask(json!({"op":"app_accept","invitation":invitation}));
    let host_peer = host.ask(json!({"op":"app_status"}))["profile"]["peer_id"]
        .as_str()
        .unwrap()
        .to_owned();
    let guest_peer = guest.ask(json!({"op":"app_status"}))["profile"]["peer_id"]
        .as_str()
        .unwrap()
        .to_owned();
    let files = manifest
        .files
        .iter()
        .map(|file| {
            json!({
                "file_id":hex(Sha256::digest(file.path.as_bytes())),"relative_path":file.path,
                "path":source.join(&file.path),"size":file.bytes,"sha256":hex(file.sha256)
            })
        })
        .collect::<Vec<_>>();
    host.ask(
        json!({"op":"app_file_offer","peer_id":guest_peer,"manifest_id":manifest_id,
        "metadata":{"game_name":"Fixture","minecraft_version":"1.21.1",
        "loader_kind":"none","loader_version":""},"files":files}),
    );
    host.ask(json!({"op":"app_file_announce","peer_id":guest_peer,"manifest_id":manifest_id}));
    let notices = guest.ask(json!({"op":"app_file_offers"}));
    assert_eq!(notices["offers"][0]["manifest_id"], manifest_id);
    let page = guest.ask(json!({"op":"app_file_manifest","peer_id":host_peer,
        "manifest_id":manifest_id,"offset":0}));
    assert_eq!(page["total_files"], 2);
    guest.ask(json!({"op":"app_file_approve","peer_id":host_peer,
        "manifest_id":manifest_id,"transfer_id":manifest_id}));
    for file in &manifest.files {
        if file.path == "mods/custom.jar" {
            continue;
        }
        let destination = incoming.join(&file.path);
        fs::create_dir_all(destination.parent().unwrap()).unwrap();
        let mut output = fs::File::create(&destination).unwrap();
        let id = hex(Sha256::digest(file.path.as_bytes()));
        for index in 0..file.bytes.div_ceil(32 * 1024) {
            let chunk = guest.ask(json!({"op":"app_file_chunk","transfer_id":manifest_id,
                "file_id":id,"chunk_index":index}));
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(chunk["data_base64"].as_str().unwrap())
                .unwrap();
            assert_eq!(hex(Sha256::digest(&bytes)), chunk["chunk_sha256"]);
            output.write_all(&bytes).unwrap();
        }
    }
    let target = root.join("target");
    let reused =
        game_transfer::receive(&manifest, Some(&local), &incoming, &target, &cancel).unwrap();
    assert_eq!(reused, b"owned fixture mod".len() as u64);
    assert_eq!(
        fs::read(target.join("saves/World/level.dat")).unwrap(),
        world
    );
    assert_eq!(
        fs::read(target.join("mods/custom.jar")).unwrap(),
        b"owned fixture mod"
    );
    assert!(!target.join("accounts.json").exists());
    assert!(source.join("saves/World/level.dat").exists());
    guest.ask(json!({"op":"app_file_complete","transfer_id":manifest_id}));
    assert_eq!(
        host.ask(json!({"op":"app_file_status","transfer_id":manifest_id}))["state"],
        "completed"
    );
    drop(host);
    drop(guest);
    fs::remove_dir_all(root).unwrap();
}
