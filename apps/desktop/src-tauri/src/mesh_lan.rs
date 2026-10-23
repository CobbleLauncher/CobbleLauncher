// SPDX-License-Identifier: MIT
//! Bounded own-game LAN announcement lookup, followed by exact PID validation.
use super::*;
use std::io::{Read, Seek, SeekFrom};
#[derive(Serialize)]
pub struct Lan {
    pub state: &'static str,
    pub port: Option<u16>,
}
fn announced_port(text: &str) -> Option<u16> {
    text.lines().rev().find_map(|line| {
        ["Started serving on ", "Local game hosted on port "]
            .iter()
            .find_map(|prefix| {
                let (_, tail) = line.rsplit_once(prefix)?;
                let digits: String = tail.chars().take_while(char::is_ascii_digit).collect();
                digits.parse::<u16>().ok().filter(|port| *port >= 1024)
            })
    })
}
fn log_port(directory: &std::path::Path, started: u64) -> Option<u16> {
    // Core runner truncates this owned output before each launch. latest.log may retain
    // older announcements in modded games and is not session evidence.
    for relative in [".cobble-logs/launcher.log"] {
        let path = directory.join(relative);
        let Ok(parent) = std::fs::symlink_metadata(path.parent()?) else {
            continue;
        };
        if !parent.is_dir() || parent.file_type().is_symlink() {
            continue;
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            if parent.file_attributes() & 0x400 != 0 {
                continue;
            }
        }
        let Ok(metadata) = std::fs::symlink_metadata(&path) else {
            continue;
        };
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            continue;
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            if metadata.file_attributes() & 0x400 != 0 {
                continue;
            }
        }
        if metadata
            .modified()
            .ok()?
            .duration_since(UNIX_EPOCH)
            .ok()?
            .as_secs()
            < started
        {
            continue;
        }
        let mut file = std::fs::File::open(path).ok()?;
        file.seek(SeekFrom::Start(metadata.len().saturating_sub(128 * 1024)))
            .ok()?;
        let mut bytes = Vec::new();
        file.take(128 * 1024).read_to_end(&mut bytes).ok()?;
        if let Some(port) = announced_port(&String::from_utf8_lossy(&bytes)) {
            return Some(port);
        }
    }
    None
}
pub(super) fn discover(game: &sessions::Session) -> Lan {
    let Some(port) = log_port(&game.directory, game.started) else {
        return Lan {
            state: "not_open",
            port: None,
        };
    };
    if port::owned_listener(port, game.pid).is_ok() && sessions::alive(game) {
        Lan {
            state: "detected",
            port: Some(port),
        }
    } else {
        Lan {
            state: "unverified",
            port: None,
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn log_lookup_uses_owned_fallback_and_ignores_stale_generation() {
        let directory = tempfile::tempdir().unwrap();
        std::fs::create_dir(directory.path().join(".cobble-logs")).unwrap();
        std::fs::write(
            directory.path().join(".cobble-logs/launcher.log"),
            "Started serving on 54321",
        )
        .unwrap();
        assert_eq!(log_port(directory.path(), 0), Some(54321));
        let future = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
            + 60;
        assert_eq!(log_port(directory.path(), future), None);
    }
    #[test]
    fn reads_only_known_announcements_and_last_valid_port() {
        assert_eq!(
            announced_port(
                "[Render thread/INFO]: Started serving on 54321\n[CHAT] Local game hosted on port 61234"
            ),
            Some(61234)
        );
        assert_eq!(announced_port("[CHAT] localhost:25565 arbitrary"), None);
        assert_eq!(announced_port("Started serving on 999999"), None);
        assert_eq!(announced_port("Started serving on 80"), None);
    }
}
