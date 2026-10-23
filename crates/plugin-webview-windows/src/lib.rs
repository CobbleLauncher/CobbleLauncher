// SPDX-License-Identifier: MIT
//! Read the actual child WebView2 environment version on its owning UI thread.
//! The desktop retains unsafe_code=forbid; only this typed COM getter needs an
//! unsafe call. This adapter does not intercept or claim to contain networking.
#[cfg(windows)]
pub mod native {
    use webview2_com::{CoTaskMemPWSTR, Microsoft::Web::WebView2::Win32::ICoreWebView2Environment};
    use windows::core::PWSTR;

    /// Call from Tauri's with_webview callback on the environment's owning thread.
    pub fn browser_version(environment: ICoreWebView2Environment) -> Result<String, String> {
        // SAFETY: the typed environment is owned by the current with_webview
        // callback; WebView2 initializes the COM-allocated UTF-16 out pointer,
        // which CoTaskMemPWSTR owns and releases before returning.
        unsafe {
            let mut version = PWSTR::null();
            environment
                .BrowserVersionString(&mut version)
                .map_err(|_| "WebView2 version unavailable")?;
            Ok(CoTaskMemPWSTR::from(version).to_string())
        }
    }
}
