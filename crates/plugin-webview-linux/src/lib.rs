// SPDX-License-Identifier: MIT
//! Read the loaded WebKitGTK library version before enabling plugin UIs.
#[cfg(target_os = "linux")]
pub fn browser_version() -> (u32, u32, u32) {
    // SAFETY: These functions return scalar version components, with no pointer
    // arguments, ownership transfer, or borrowed WebKit objects.
    unsafe {
        (
            webkit2gtk::ffi::webkit_get_major_version(),
            webkit2gtk::ffi::webkit_get_minor_version(),
            webkit2gtk::ffi::webkit_get_micro_version(),
        )
    }
}
