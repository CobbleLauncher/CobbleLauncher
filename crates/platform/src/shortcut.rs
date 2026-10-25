// SPDX-License-Identifier: MIT
//! Windows Shell Link adapter. Call from a dedicated blocking thread.
use std::path::Path;
/// Reads a local Shell Link's recorded target. Never resolves, launches or updates it.
pub fn read_shortcut_target(path: &Path) -> Result<std::path::PathBuf, String> {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::fs::MetadataExt;
        use windows::{
            Win32::{
                System::Com::{
                    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance,
                    CoInitializeEx, CoUninitialize, IPersistFile, STGM_READ,
                },
                UI::Shell::{IShellLinkW, SLGP_RAWPATH, ShellLink},
            },
            core::{HSTRING, Interface},
        };
        let meta = std::fs::symlink_metadata(path).map_err(|_| "SHORTCUT_UNAVAILABLE")?;
        if !meta.is_file()
            || meta.file_attributes() & 0x400 != 0
            || meta.len() > 1024 * 1024
            || !path
                .extension()
                .is_some_and(|v| v.eq_ignore_ascii_case("lnk"))
        {
            return Err("SHORTCUT_INVALID".into());
        }
        struct Apartment;
        impl Drop for Apartment {
            fn drop(&mut self) {
                // SAFETY: balances this thread's successful COM initialization.
                unsafe {
                    CoUninitialize();
                }
            }
        }
        // SAFETY: no reserved pointer; the blocking caller owns this apartment.
        unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok() }
            .map_err(|_| "SHORTCUT_UNAVAILABLE")?;
        let _apartment = Apartment;
        // SAFETY: the standard Shell Link CLSID implements the requested interface.
        let link: IShellLinkW = unsafe { CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER) }
            .map_err(|_| "SHORTCUT_UNAVAILABLE")?;
        let persist: IPersistFile = link.cast().map_err(|_| "SHORTCUT_UNAVAILABLE")?;
        let mut buffer = [0u16; 4096];
        // SAFETY: buffers outlive synchronous read-only methods; no Resolve or execution.
        unsafe {
            persist
                .Load(&HSTRING::from(path.as_os_str()), STGM_READ)
                .map_err(|_| "SHORTCUT_INVALID")?;
            link.GetPath(&mut buffer, std::ptr::null_mut(), SLGP_RAWPATH.0 as u32)
                .map_err(|_| "SHORTCUT_INVALID")?;
        }
        let end = buffer
            .iter()
            .position(|v| *v == 0)
            .ok_or("SHORTCUT_INVALID")?;
        let target = std::path::PathBuf::from(
            String::from_utf16(&buffer[..end]).map_err(|_| "SHORTCUT_INVALID")?,
        );
        let text = target.to_string_lossy();
        if !target.is_absolute() || text.starts_with(r"\\") || text.starts_with("//") {
            return Err("SHORTCUT_INVALID".into());
        }
        Ok(target)
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = path;
        Err("SHORTCUT_UNSUPPORTED_PLATFORM".into())
    }
}
pub fn create_launch_shortcut(
    executable: &Path,
    destination: &Path,
    id: &str,
) -> Result<(), String> {
    if id.is_empty()
        || id.len() > 128
        || !id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        return Err("INVALID_GAME_ID".into());
    }
    #[cfg(target_os = "windows")]
    {
        windows_shortcut(executable, destination, id)
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (executable, destination);
        Err("SHORTCUT_UNSUPPORTED_PLATFORM".into())
    }
}
#[cfg(target_os = "windows")]
fn windows_shortcut(executable: &Path, destination: &Path, id: &str) -> Result<(), String> {
    use windows::{
        Win32::{
            System::Com::{
                CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
                CoUninitialize, IPersistFile,
            },
            UI::Shell::{IShellLinkW, ShellLink},
        },
        core::{HSTRING, Interface},
    };
    struct Com;
    impl Drop for Com {
        fn drop(&mut self) {
            // SAFETY: balances the successful initialization on this same thread.
            unsafe {
                CoUninitialize();
            }
        }
    }
    // SAFETY: no reserved pointer; this dedicated worker owns its apartment.
    unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok() }.map_err(|e| e.to_string())?;
    let _com = Com;
    // SAFETY: CLSID and requested typed COM interface are the Windows Shell Link pair.
    let link: IShellLinkW = unsafe { CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER) }
        .map_err(|e| e.to_string())?;
    let executable = HSTRING::from(executable.as_os_str());
    let arguments = HSTRING::from(format!("--instance {id} --play"));
    let destination_string = HSTRING::from(destination.as_os_str());
    // SAFETY: HSTRING buffers remain live through each synchronous COM method.
    unsafe {
        link.SetPath(&executable).map_err(|e| e.to_string())?;
        link.SetArguments(&arguments).map_err(|e| e.to_string())?;
        link.SetIconLocation(&executable, 0)
            .map_err(|e| e.to_string())?;
    }
    let persist: IPersistFile = link.cast().map_err(|e| e.to_string())?;
    let reservation = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)
        .map_err(|e| e.to_string())?;
    drop(reservation);
    // SAFETY: destination buffer lives through Save; only our newly reserved link is replaced.
    let result = unsafe { persist.Save(&destination_string, true) }.map_err(|e| e.to_string());
    if result.is_err() {
        let _ = std::fs::remove_file(destination);
    }
    result
}
#[cfg(all(test, target_os = "windows"))]
mod tests {
    use super::*;
    #[test]
    fn shell_link_roundtrips_arguments_without_launching_or_overwriting() {
        use windows::{
            Win32::{
                System::Com::{
                    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance,
                    CoInitializeEx, CoUninitialize, IPersistFile, STGM_READ,
                },
                UI::Shell::{IShellLinkW, ShellLink},
            },
            core::{HSTRING, Interface},
        };
        let root = std::env::temp_dir().join(format!(
            "cobble-shortcut-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&root).unwrap();
        let target = root.join("Play.lnk");
        create_launch_shortcut(&std::env::current_exe().unwrap(), &target, "legacy_test-1")
            .unwrap();
        let original = std::fs::read(&target).unwrap();
        assert_eq!(
            read_shortcut_target(&target).unwrap(),
            std::env::current_exe().unwrap()
        );
        assert!(
            create_launch_shortcut(&std::env::current_exe().unwrap(), &target, "other").is_err()
        );
        assert_eq!(std::fs::read(&target).unwrap(), original);
        // SAFETY: this test thread owns and balances its COM apartment; buffers and typed interfaces outlive calls.
        unsafe {
            CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok().unwrap();
            {
                let link: IShellLinkW =
                    CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).unwrap();
                let persist: IPersistFile = link.cast().unwrap();
                persist
                    .Load(&HSTRING::from(target.as_os_str()), STGM_READ)
                    .unwrap();
                let mut arguments = [0u16; 256];
                link.GetArguments(&mut arguments).unwrap();
                let end = arguments.iter().position(|value| *value == 0).unwrap();
                assert_eq!(
                    String::from_utf16(&arguments[..end]).unwrap(),
                    "--instance legacy_test-1 --play"
                );
            }
            CoUninitialize();
        }
        std::fs::remove_file(target).unwrap();
        std::fs::remove_dir(root).unwrap();
    }
}
