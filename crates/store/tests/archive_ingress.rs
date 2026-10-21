// SPDX-License-Identifier: MIT
//! Original synthetic ZIPs; no Minecraft or launcher implementation fixtures.

use std::io::{Cursor, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use cap_std::{ambient_authority, fs::Dir};
use cobble_store::{ArchiveError, ArchiveLimits, SafeRelativePath, extract_zip};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let name = format!(
            "cobble-archive-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        );
        let path = std::env::temp_dir().join(name);
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn dir(&self) -> Dir {
        Dir::open_ambient_dir(&self.0, ambient_authority()).unwrap()
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}

#[derive(Clone)]
struct Item<'a> {
    name: &'a [u8],
    data: &'a [u8],
    mode: u32,
    deflate: bool,
    lie_size: Option<u32>,
}
fn item<'a>(name: &'a str, data: &'a [u8]) -> Item<'a> {
    Item {
        name: name.as_bytes(),
        data,
        mode: 0o100644,
        deflate: false,
        lie_size: None,
    }
}
fn word(bytes: &mut Vec<u8>, value: u16) {
    bytes.extend(value.to_le_bytes());
}
fn dword(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend(value.to_le_bytes());
}

// Minimal ZIP32 per APPNOTE layout, deliberately independently constructed.
fn zip(items: &[Item<'_>]) -> Vec<u8> {
    let mut bytes = Vec::new();
    let mut central = Vec::new();
    for item in items {
        let offset = bytes.len() as u32;
        let compressed = if item.deflate {
            let mut encoder =
                flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::best());
            encoder.write_all(item.data).unwrap();
            encoder.finish().unwrap()
        } else {
            item.data.to_vec()
        };
        let method = if item.deflate { 8 } else { 0 };
        let expanded = item.lie_size.unwrap_or(item.data.len() as u32);
        let crc = crc32fast::hash(item.data);
        bytes.extend(b"PK\x03\x04");
        word(&mut bytes, 20);
        word(&mut bytes, 0x800);
        word(&mut bytes, method);
        word(&mut bytes, 0);
        word(&mut bytes, 0);
        dword(&mut bytes, crc);
        dword(&mut bytes, compressed.len() as u32);
        dword(&mut bytes, expanded);
        word(&mut bytes, item.name.len() as u16);
        word(&mut bytes, 0);
        bytes.extend(item.name);
        bytes.extend(&compressed);
        central.extend(b"PK\x01\x02");
        word(&mut central, 0x314);
        word(&mut central, 20);
        word(&mut central, 0x800);
        word(&mut central, method);
        word(&mut central, 0);
        word(&mut central, 0);
        dword(&mut central, crc);
        dword(&mut central, compressed.len() as u32);
        dword(&mut central, expanded);
        word(&mut central, item.name.len() as u16);
        word(&mut central, 0);
        word(&mut central, 0);
        word(&mut central, 0);
        word(&mut central, 0);
        dword(&mut central, item.mode << 16);
        dword(&mut central, offset);
        central.extend(item.name);
    }
    let offset = bytes.len() as u32;
    let size = central.len() as u32;
    bytes.extend(central);
    bytes.extend(b"PK\x05\x06");
    word(&mut bytes, 0);
    word(&mut bytes, 0);
    word(&mut bytes, items.len() as u16);
    word(&mut bytes, items.len() as u16);
    dword(&mut bytes, size);
    dword(&mut bytes, offset);
    word(&mut bytes, 0);
    bytes
}

fn run(
    bytes: Vec<u8>,
    limits: ArchiveLimits,
) -> Result<cobble_store::ArchiveSummary, ArchiveError> {
    let temp = Temp::new();
    extract_zip(Cursor::new(bytes), &temp.dir(), limits)
}

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures")
}

#[test]
fn sec002_path_fixture_and_platform_rules() {
    let cases: serde_json::Value = serde_json::from_slice(
        &std::fs::read(fixtures().join("security/path-cases.json")).unwrap(),
    )
    .unwrap();
    for case in cases.as_array().unwrap() {
        let path = case["path"].as_str().unwrap();
        assert_eq!(
            SafeRelativePath::parse(path).is_ok(),
            case["allowed"].as_bool().unwrap(),
            "{path:?}"
        );
    }
    for path in [
        "foo/<x",
        "foo/a\u{7f}",
        "foo/a\u{85}",
        "mods/COM¹.jar",
        "LPT²",
        "conout$.txt",
        "a/CONIN$",
        "a/x?",
        "a/x*",
        "a/x\"",
        "a/x|",
        "a/x>",
        "mods/CON .txt",
    ] {
        assert!(SafeRelativePath::parse(path).is_err(), "{path:?}");
    }
    assert_eq!(
        SafeRelativePath::parse("config/e\u{301}.txt")
            .unwrap()
            .as_str(),
        "config/é.txt"
    );
    assert!(SafeRelativePath::parse(&"a".repeat(241)).is_err());
    assert!(SafeRelativePath::parse(&["a"; 65].join("/")).is_err());
}

#[test]
fn sec003_existing_malicious_archives_rejected() {
    for name in [
        "traversal.zip",
        "absolute.zip",
        "drive.zip",
        "duplicate.zip",
        "case-collision.zip",
        "symlink.zip",
    ] {
        let temp = Temp::new();
        let bytes = std::fs::read(fixtures().join("security").join(name)).unwrap();
        assert!(
            extract_zip(Cursor::new(bytes), &temp.dir(), ArchiveLimits::default()).is_err(),
            "{name}"
        );
        assert_eq!(
            std::fs::read_dir(&temp.0).unwrap().count(),
            0,
            "preflight must leave staging untouched: {name}"
        );
    }
}

#[test]
fn sec003_unicode_directory_aliases_duplicates_and_prefixes() {
    for names in [
        ["A/x", "a/y"],
        ["é/x", "e\u{301}/y"],
        ["straße", "STRASSE"],
        ["ı/x", "I/y"],
        ["Σ/x", "ς/y"],
        ["Ｋ/x", "K/y"],
        ["a", "a/b"],
        ["a/b", "a"],
        ["x", "x"],
        ["a/", "a/"],
    ] {
        let files: Vec<_> = names
            .iter()
            .map(|name| {
                let mut file = item(name, b"");
                if name.ends_with('/') {
                    file.mode = 0o040755;
                }
                file
            })
            .collect();
        assert!(
            matches!(
                run(zip(&files), ArchiveLimits::default()),
                Err(ArchiveError::Collision)
            ),
            "{names:?}"
        );
    }
}

#[test]
fn stored_deflate_and_implicit_directory_order_work() {
    let temp = Temp::new();
    let mut compressed = item("config/配置.toml", b"hello = true\n");
    compressed.deflate = true;
    let mut directory = item("config/", b"");
    directory.mode = 0o040755;
    let bytes = zip(&[
        compressed,
        directory,
        item("mods/demo.jar", b"synthetic executable marker"),
    ]);
    let summary = extract_zip(Cursor::new(bytes), &temp.dir(), ArchiveLimits::default()).unwrap();
    assert_eq!(summary.entries, 3);
    assert_eq!(
        std::fs::read(temp.0.join("config/配置.toml")).unwrap(),
        b"hello = true\n"
    );
    assert_eq!(
        std::fs::read(temp.0.join("mods/demo.jar")).unwrap(),
        b"synthetic executable marker"
    );
}

#[test]
fn sec003_non_utf8_links_devices_and_unsupported_profiles_rejected() {
    let non_utf8 = Item {
        name: b"mods/\xff.jar",
        ..item("", b"")
    };
    assert!(matches!(
        run(zip(&[non_utf8]), ArchiveLimits::default()),
        Err(ArchiveError::InvalidPath(_))
    ));
    for mode in [0o120777, 0o060644, 0o020644, 0o010644, 0o140644] {
        assert!(matches!(
            run(
                zip(&[Item {
                    mode,
                    ..item("a", b"")
                }]),
                ArchiveLimits::default()
            ),
            Err(ArchiveError::UnsafeObject)
        ));
    }
    for flag in [1_u16, 8, 0x40, 0x2000] {
        let mut bytes = zip(&[item("a", b"x")]);
        let central = bytes.windows(4).position(|x| x == b"PK\x01\x02").unwrap();
        bytes[central + 8..central + 10].copy_from_slice(&flag.to_le_bytes());
        assert!(matches!(
            run(bytes, ArchiveLimits::default()),
            Err(ArchiveError::Unsupported(_))
        ));
    }
    for header_field in [28, 30] {
        let mut bytes = zip(&[item("a", b"x")]);
        let at = if header_field == 28 {
            0
        } else {
            bytes.windows(4).position(|x| x == b"PK\x01\x02").unwrap()
        };
        bytes[at + header_field..at + header_field + 2].copy_from_slice(&1_u16.to_le_bytes());
        assert!(matches!(
            run(bytes, ArchiveLimits::default()),
            Err(ArchiveError::Unsupported(_))
        ));
    }
}

#[test]
fn sec004_actual_expansion_rejects_consistent_lying_headers() {
    let payload = [b'x'; 6000];
    let file = Item {
        deflate: true,
        lie_size: Some(1),
        ..item("metadata.json", &payload)
    };
    let bytes = zip(&[file]);
    let temp = Temp::new();
    let limits = ArchiveLimits {
        max_entry_bytes: 1000,
        ..ArchiveLimits::default()
    };
    assert!(matches!(
        extract_zip(Cursor::new(bytes), &temp.dir(), limits),
        Err(ArchiveError::Limit("actual expanded bytes"))
    ));
    assert!(
        std::fs::metadata(temp.0.join("metadata.json"))
            .unwrap()
            .len()
            <= 1000
    );
}

#[test]
fn sec004_declared_and_actual_budgets_crc_and_headers() {
    let bytes = zip(&[item("a", b"1234"), item("b", b"5678")]);
    for limits in [
        ArchiveLimits {
            max_entries: 1,
            ..ArchiveLimits::default()
        },
        ArchiveLimits {
            max_archive_bytes: 20,
            ..ArchiveLimits::default()
        },
        ArchiveLimits {
            max_entry_bytes: 3,
            ..ArchiveLimits::default()
        },
        ArchiveLimits {
            max_total_bytes: 7,
            ..ArchiveLimits::default()
        },
    ] {
        assert!(matches!(
            run(bytes.clone(), limits),
            Err(ArchiveError::Limit(_))
        ));
    }
    let payload = [0; 10000];
    let file = Item {
        deflate: true,
        ..item("a", &payload)
    };
    assert!(matches!(
        run(zip(&[file]), ArchiveLimits::default()),
        Err(ArchiveError::Limit("declared compression ratio"))
    ));
    let files = [
        item("a", b"1234"),
        Item {
            lie_size: Some(1),
            ..item("b", b"5678")
        },
    ];
    assert!(matches!(
        run(
            zip(&files),
            ArchiveLimits {
                max_total_bytes: 7,
                ..ArchiveLimits::default()
            }
        ),
        Err(ArchiveError::Limit("actual expanded bytes"))
    ));
    let mut bad_crc = zip(&[item("a", b"1234")]);
    bad_crc[31] ^= 1;
    assert!(matches!(
        run(bad_crc, ArchiveLimits::default()),
        Err(ArchiveError::InvalidArchive(_))
    ));
    let mut mismatch = zip(&[item("a", b"1234")]);
    mismatch[22] ^= 1;
    assert!(matches!(
        run(mismatch, ArchiveLimits::default()),
        Err(ArchiveError::InvalidArchive(_))
    ));
    let mut name_mismatch = zip(&[item("a", b"1234")]);
    name_mismatch[30] = b'b';
    assert!(matches!(
        run(name_mismatch, ArchiveLimits::default()),
        Err(ArchiveError::InvalidArchive(_))
    ));
}

#[test]
fn existing_hardlink_target_is_never_overwritten() {
    let temp = Temp::new();
    let outside = Temp::new();
    std::fs::write(outside.0.join("precious"), b"original").unwrap();
    std::fs::hard_link(outside.0.join("precious"), temp.0.join("a")).unwrap();
    assert!(run_into(&temp, &[item("a", b"malicious")]).is_err());
    assert_eq!(
        std::fs::read(outside.0.join("precious")).unwrap(),
        b"original"
    );
}

#[test]
fn sec004_budgets_cannot_disable_hard_ceilings() {
    for limits in [
        ArchiveLimits {
            max_archive_bytes: u64::MAX,
            ..ArchiveLimits::default()
        },
        ArchiveLimits {
            max_entry_bytes: u64::MAX,
            ..ArchiveLimits::default()
        },
        ArchiveLimits {
            max_total_bytes: u64::MAX,
            ..ArchiveLimits::default()
        },
        ArchiveLimits {
            max_entries: u32::MAX,
            ..ArchiveLimits::default()
        },
        ArchiveLimits {
            max_ratio: u32::MAX,
            ..ArchiveLimits::default()
        },
        ArchiveLimits {
            max_ratio: 0,
            ..ArchiveLimits::default()
        },
    ] {
        assert!(matches!(
            run(zip(&[]), limits),
            Err(ArchiveError::Limit("invalid budget or hard ceiling"))
        ));
    }
    let many_empty: Vec<_> = (0..100).map(|n| format!("entry-{n}")).collect();
    let entries: Vec<_> = many_empty.iter().map(|name| item(name, b"")).collect();
    assert!(matches!(
        run(
            zip(&entries),
            ArchiveLimits {
                max_entries: 99,
                ..ArchiveLimits::default()
            }
        ),
        Err(ArchiveError::Limit("entry count"))
    ));
}

#[test]
fn sec004_padding_cannot_fake_a_safe_deflate_ratio() {
    let payload = [b'x'; 6000];
    let mut bytes = zip(&[Item {
        deflate: true,
        ..item("a", &payload)
    }]);
    let central = bytes.windows(4).position(|x| x == b"PK\x01\x02").unwrap();
    let actual_compressed = u32::from_le_bytes(bytes[18..22].try_into().unwrap()) as usize;
    let padding = 6000 - actual_compressed;
    // Both headers claim 6000 compressed bytes. Actual deflate consumes only
    // the tiny stream: zero padding must not buy a bigger ratio allowance.
    bytes.splice(central..central, std::iter::repeat_n(0, padding));
    let central = central + padding;
    bytes[18..22].copy_from_slice(&6000_u32.to_le_bytes());
    bytes[central + 20..central + 24].copy_from_slice(&6000_u32.to_le_bytes());
    let end = bytes.len() - 22;
    bytes[end + 16..end + 20].copy_from_slice(&(central as u32).to_le_bytes());
    assert!(matches!(
        run(
            bytes,
            ArchiveLimits {
                max_ratio: 1,
                ..ArchiveLimits::default()
            }
        ),
        Err(ArchiveError::InvalidArchive(
            "actual size, compressed consumption or CRC mismatch"
        ))
    ));
}

#[test]
fn sec004_truncated_deflate_cannot_report_success() {
    let mut bytes = zip(&[Item {
        deflate: true,
        ..item("a", b"hello")
    }]);
    let central = bytes.windows(4).position(|x| x == b"PK\x01\x02").unwrap();
    let count = u32::from_le_bytes(bytes[18..22].try_into().unwrap()) - 1;
    bytes.remove(central - 1);
    let central = central - 1;
    bytes[18..22].copy_from_slice(&count.to_le_bytes());
    bytes[central + 20..central + 24].copy_from_slice(&count.to_le_bytes());
    let end = bytes.len() - 22;
    bytes[end + 16..end + 20].copy_from_slice(&(central as u32).to_le_bytes());
    assert!(run(bytes, ArchiveLimits::default()).is_err());
}
fn run_into(temp: &Temp, items: &[Item<'_>]) -> Result<cobble_store::ArchiveSummary, ArchiveError> {
    extract_zip(
        Cursor::new(zip(items)),
        &temp.dir(),
        ArchiveLimits::default(),
    )
}

#[cfg(unix)]
#[test]
fn preexisting_symlink_directory_cannot_escape() {
    let temp = Temp::new();
    let outside = Temp::new();
    std::os::unix::fs::symlink(&outside.0, temp.0.join("mods")).unwrap();
    assert!(run_into(&temp, &[item("mods/escaped", b"malicious")]).is_err());
    assert!(!outside.0.join("escaped").exists());
}

#[cfg(windows)]
fn junction(link: &Path, destination: &Path) {
    let output = std::process::Command::new("cmd.exe")
        .args(["/d", "/c", "mklink", "/J"])
        .arg(link)
        .arg(destination)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "junction creation failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[cfg(windows)]
#[test]
fn windows_junction_directory_cannot_escape() {
    let temp = Temp::new();
    let outside = Temp::new();
    junction(&temp.0.join("mods"), &outside.0);
    assert!(run_into(&temp, &[item("mods/escaped", b"malicious")]).is_err());
    assert!(!outside.0.join("escaped").exists());
    std::fs::remove_dir(temp.0.join("mods")).unwrap();
}
