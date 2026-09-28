//! Windows placement for the native-messaging host manifests — split from `register.rs`
//! (R8 relief): the manifest location is arbitrary on Windows, so the browser is pointed at it
//! via an HKCU registry value (`set_hkcu_default`) rather than a fixed well-known directory.

#![cfg(windows)]

use std::path::Path;

use super::super::NATIVE_HOST_NAME;
use super::manifest::write_manifest;

/// Write both host manifests under `data_dir` and point Firefox/Chrome at them via HKCU.
pub(super) fn register_windows(data_dir: &Path, firefox_json: &[u8], chrome_json: &[u8]) {
    // Windows: the manifest location is arbitrary; the browser is pointed at
    // it by an HKCU registry value. Keep both under the app data dir.
    let dir = data_dir.join("native-messaging");
    let firefox_path = dir.join(format!("{NATIVE_HOST_NAME}.firefox.json"));
    let chrome_path = dir.join(format!("{NATIVE_HOST_NAME}.chrome.json"));
    write_manifest("firefox", &firefox_path, firefox_json);
    write_manifest("chrome", &chrome_path, chrome_json);

    let firefox_key = format!("Software\\Mozilla\\NativeMessagingHosts\\{NATIVE_HOST_NAME}");
    let chrome_key = format!("Software\\Google\\Chrome\\NativeMessagingHosts\\{NATIVE_HOST_NAME}");
    if let Err(e) = set_hkcu_default(&firefox_key, &firefox_path.to_string_lossy()) {
        log::warn!(
            "[native_host] HKCU firefox key failed (non-fatal): {}",
            crate::observability::sanitize_reason(&e.to_string())
        );
    }
    if let Err(e) = set_hkcu_default(&chrome_key, &chrome_path.to_string_lossy()) {
        log::warn!(
            "[native_host] HKCU chrome key failed (non-fatal): {}",
            crate::observability::sanitize_reason(&e.to_string())
        );
    }
}

/// Set the default (`""`) value of an HKCU subkey to `value` (REG_SZ). Creates
/// the key if absent. Encapsulates the raw Win32 FFI; the registry crate would
/// be a new dependency, so this hand-rolls `RegCreateKeyExW` + `RegSetValueExW`
/// against the already-present `windows` crate.
fn set_hkcu_default(subkey: &str, value: &str) -> std::io::Result<()> {
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::ERROR_SUCCESS;
    use windows::Win32::System::Registry::{
        RegCloseKey, RegCreateKeyExW, RegSetValueExW, HKEY, HKEY_CURRENT_USER, KEY_WRITE,
        REG_OPTION_NON_VOLATILE, REG_SZ,
    };

    // UTF-16, NUL-terminated, for the PCWSTR args.
    let subkey_w: Vec<u16> = subkey.encode_utf16().chain(std::iter::once(0)).collect();
    let value_w: Vec<u16> = value.encode_utf16().chain(std::iter::once(0)).collect();

    // SAFETY: standard advapi32 registry FFI. `subkey_w` is a valid NUL-terminated
    // UTF-16 buffer kept alive across the call; `hkey` is written by
    // RegCreateKeyExW and closed unconditionally below. `value_w` (incl. its NUL)
    // is written as REG_SZ with an explicit byte length.
    unsafe {
        let mut hkey = HKEY::default();
        let status = RegCreateKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(subkey_w.as_ptr()),
            None,
            PCWSTR::null(),
            REG_OPTION_NON_VOLATILE,
            KEY_WRITE,
            None,
            &mut hkey,
            None,
        );
        if status != ERROR_SUCCESS {
            return Err(std::io::Error::from_raw_os_error(status.0 as i32));
        }

        // REG_SZ byte length includes the trailing NUL (2 bytes per u16).
        let bytes = std::slice::from_raw_parts(
            value_w.as_ptr() as *const u8,
            std::mem::size_of_val(value_w.as_slice()),
        );
        let set = RegSetValueExW(hkey, PCWSTR::null(), None, REG_SZ, Some(bytes));
        let _ = RegCloseKey(hkey);
        if set != ERROR_SUCCESS {
            return Err(std::io::Error::from_raw_os_error(set.0 as i32));
        }
    }
    Ok(())
}
