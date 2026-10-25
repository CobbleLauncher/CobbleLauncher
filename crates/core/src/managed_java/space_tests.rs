// SPDX-License-Identifier: MIT
use super::*;
#[test]
fn peak_space_uses_the_same_pinned_java_selection_as_installer_and_never_deletes_bad_metadata() {
    let path = std::env::temp_dir().join(format!(
        "cobble-java-space-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&path).unwrap();
    let root = cap_std::fs::Dir::open_ambient_dir(&path, cap_std::ambient_authority()).unwrap();
    let metadata = serde_json::json!([{"vendor":"eclipse","release_type":"ga","release_name":"jdk-21.0.1+1","version_data":{"major":21},"binaries":[{"architecture":"x64","os":"windows","image_type":"jre","jvm_impl":"hotspot","heap_size":"normal","package":{"name":"OpenJDK21U-jre_x64_windows_hotspot_21.0.1_1.zip","link":"https://github.com/adoptium/temurin21-binaries/releases/download/jdk-21.0.1%2B1/OpenJDK21U-jre_x64_windows_hotspot_21.0.1_1.zip","size":123,"checksum":"ab".repeat(32)}}]}]);
    root.write(
        ".selection-21-x64.json",
        serde_json::to_vec(&metadata).unwrap(),
    )
    .unwrap();
    assert_eq!(
        cached_space_package(&root, 21).unwrap(),
        Some((123, [0xab; 32]))
    );
    root.write(".selection-21-x64.json", b"invalid").unwrap();
    assert!(cached_space_package(&root, 21).unwrap().is_none());
    assert_eq!(root.read(".selection-21-x64.json").unwrap(), b"invalid");
    drop(root);
    std::fs::remove_dir_all(path).unwrap();
}
