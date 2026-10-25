// SPDX-License-Identifier: MIT
//! Narrow Windows Shell recycle adapter; never falls back to permanent deletion.
use std::path::Path;
pub fn recycle_directory(path: &Path) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        recycle_windows(path)
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = path;
        Err("STORAGE_RECYCLE_UNSUPPORTED".into())
    }
}
#[cfg(target_os = "windows")]
fn recycle_windows(path: &Path) -> Result<(), String> {
    use windows::{
        Win32::{
            System::Com::{
                CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
                CoUninitialize,
            },
            UI::Shell::{
                FOF_ALLOWUNDO, FOF_NOCONFIRMATION, FOF_NOERRORUI, FOF_SILENT, FOF_WANTNUKEWARNING,
                FOFX_EARLYFAILURE, FOFX_RECYCLEONDELETE, FileOperation, IFileOperation, IShellItem,
                SHCreateItemFromParsingName,
            },
        },
        core::HSTRING,
    };
    let text = path.to_str().ok_or("STORAGE_RECYCLE_UNAVAILABLE")?;
    if !path.is_absolute() || text.starts_with(r"\\") || text.len() >= 248 {
        return Err("STORAGE_RECYCLE_UNAVAILABLE".into());
    }
    struct Apartment;
    impl Drop for Apartment {
        fn drop(&mut self) {
            // SAFETY: balances this thread's successful initialization.
            unsafe {
                CoUninitialize();
            }
        }
    }
    // SAFETY: this dedicated worker owns its COM apartment.
    unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok() }
        .map_err(|_| "STORAGE_RECYCLE_UNAVAILABLE")?;
    let _apartment = Apartment;
    // SAFETY: typed Shell class/interface pair, no aggregation.
    let operation: IFileOperation =
        unsafe { CoCreateInstance(&FileOperation, None, CLSCTX_INPROC_SERVER) }
            .map_err(|_| "STORAGE_RECYCLE_UNAVAILABLE")?;
    let name = HSTRING::from(path.as_os_str());
    // SAFETY: live NUL-terminated HSTRING and requested Shell item interface.
    let item: IShellItem = unsafe { SHCreateItemFromParsingName(&name, None) }
        .map_err(|_| "STORAGE_RECYCLE_UNAVAILABLE")?;
    // SAFETY: all COM objects remain live on the initialized worker thread. RECYCLEONDELETE
    // explicitly requires recycling; there is no fallback call to delete files ourselves.
    unsafe {
        operation
            .SetOperationFlags(
                FOFX_RECYCLEONDELETE
                    | FOFX_EARLYFAILURE
                    | FOF_NOERRORUI
                    | FOF_SILENT
                    | FOF_NOCONFIRMATION
                    | FOF_WANTNUKEWARNING
                    | FOF_ALLOWUNDO,
            )
            .map_err(|_| "STORAGE_RECYCLE_UNAVAILABLE")?;
        operation
            .DeleteItem(&item, None)
            .map_err(|_| "STORAGE_RECYCLE_UNAVAILABLE")?;
        operation
            .PerformOperations()
            .map_err(|_| "STORAGE_RECYCLE_UNAVAILABLE")?;
        if operation
            .GetAnyOperationsAborted()
            .map_err(|_| "STORAGE_RECYCLE_UNAVAILABLE")?
            .as_bool()
        {
            return Err("STORAGE_RECYCLE_UNAVAILABLE".into());
        }
    }
    if path.exists() {
        return Err("STORAGE_RECYCLE_UNAVAILABLE".into());
    }
    Ok(())
}
pub fn available_disk_space(path: &Path) -> Option<u64> {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::ffi::OsStrExt;
        #[link(name = "kernel32")]
        unsafe extern "system" {
            fn GetDiskFreeSpaceExW(
                path: *const u16,
                available: *mut u64,
                total: *mut u64,
                free: *mut u64,
            ) -> i32;
        }
        let path: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        let mut bytes = 0;
        // SAFETY: NUL-terminated path and valid output; remaining outputs are optional.
        if unsafe {
            GetDiskFreeSpaceExW(
                path.as_ptr(),
                &mut bytes,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        } != 0
        {
            Some(bytes)
        } else {
            None
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = path;
        None
    }
}
