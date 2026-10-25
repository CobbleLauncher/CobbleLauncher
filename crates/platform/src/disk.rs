// SPDX-License-Identifier: MIT
//! Stable local volume identity for grouping peak-space reservations.
use std::path::Path;
pub fn disk_volume_identity(path: &Path) -> Option<(String, String)> {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        #[link(name = "kernel32")]
        unsafe extern "system" {
            fn GetVolumePathNameW(path: *const u16, volume: *mut u16, length: u32) -> i32;
            fn GetVolumeNameForVolumeMountPointW(
                path: *const u16,
                name: *mut u16,
                length: u32,
            ) -> i32;
        }
        let input: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        let mut mount = vec![0u16; 32768];
        // SAFETY: input is NUL-terminated and the output buffer has the stated capacity.
        if unsafe { GetVolumePathNameW(input.as_ptr(), mount.as_mut_ptr(), mount.len() as u32) }
            == 0
        {
            return None;
        }
        let end = mount.iter().position(|c| *c == 0)?;
        let label = String::from_utf16(&mount[..end]).ok()?;
        let mut guid = [0u16; 64];
        // SAFETY: mount remains NUL-terminated and guid is a valid writable buffer.
        let id = if unsafe {
            GetVolumeNameForVolumeMountPointW(mount.as_ptr(), guid.as_mut_ptr(), guid.len() as u32)
        } != 0
        {
            let end = guid.iter().position(|c| *c == 0)?;
            String::from_utf16(&guid[..end]).ok()?
        } else {
            label.clone()
        };
        Some((id.to_ascii_lowercase(), label))
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let device = std::fs::metadata(path).ok()?.dev();
        Some((format!("device-{device}"), format!("Volume {device}")))
    }
    #[cfg(not(any(windows, unix)))]
    {
        let _ = path;
        None
    }
}
#[cfg(test)]
mod tests {
    #[test]
    fn existing_paths_on_the_same_volume_share_identity() {
        let temp = std::env::temp_dir();
        let first = super::disk_volume_identity(&temp).unwrap();
        let parent = temp.parent().unwrap_or(&temp);
        let second = super::disk_volume_identity(parent).unwrap();
        assert!(!first.0.is_empty() && !first.1.is_empty());
        #[cfg(windows)]
        assert_eq!(first.0, second.0);
        #[cfg(not(windows))]
        let _ = second;
    }
}
