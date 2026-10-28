use windows::core::w;
use windows::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS};
use windows::Win32::System::Registry::{
    RegCloseKey, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW,
    HKEY_CURRENT_USER, KEY_QUERY_VALUE, KEY_SET_VALUE, REG_SZ, HKEY,
};

const RUN_SUBKEY: windows::core::PCWSTR =
    w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
const VALUE_NAME: windows::core::PCWSTR = w!("b-snip");
const LEGACY_VALUE_NAME: windows::core::PCWSTR = w!("b-abcABC");

fn open_run_key() -> Option<HKEY> {
    unsafe {
        let mut hkey = HKEY::default();
        let err = RegOpenKeyExW(
            HKEY_CURRENT_USER,
            RUN_SUBKEY,
            None,
            KEY_QUERY_VALUE | KEY_SET_VALUE,
            &mut hkey,
        );
        if err == ERROR_SUCCESS {
            Some(hkey)
        } else {
            None
        }
    }
}

pub fn migrate_legacy_run_key() {
    if is_enabled() {
        delete_value(LEGACY_VALUE_NAME);
        return;
    }
    if !value_exists(LEGACY_VALUE_NAME) {
        return;
    }
    if set_enabled(true) {
        delete_value(LEGACY_VALUE_NAME);
    }
}

fn value_exists(name: windows::core::PCWSTR) -> bool {
    let Some(hkey) = open_run_key() else {
        return false;
    };
    unsafe {
        let mut size = 0u32;
        let err = RegQueryValueExW(hkey, name, None, None, None, Some(&mut size));
        let _ = RegCloseKey(hkey);
        err == ERROR_SUCCESS && size > 0
    }
}

fn delete_value(name: windows::core::PCWSTR) {
    let Some(hkey) = open_run_key() else {
        return;
    };
    unsafe {
        let _ = RegDeleteValueW(hkey, name);
        let _ = RegCloseKey(hkey);
    }
}

pub fn is_enabled() -> bool {
    let Some(hkey) = open_run_key() else {
        return false;
    };
    unsafe {
        let mut size = 0u32;
        let err = RegQueryValueExW(hkey, VALUE_NAME, None, None, None, Some(&mut size));
        let _ = RegCloseKey(hkey);
        err == ERROR_SUCCESS && size > 0
    }
}

pub fn set_enabled(on: bool) -> bool {
    let Some(hkey) = open_run_key() else {
        return false;
    };
    let ok = unsafe {
        let err = if on {
            let exe = match std::env::current_exe() {
                Ok(p) => p,
                Err(_) => {
                    let _ = RegCloseKey(hkey);
                    return false;
                }
            };
            let quoted = format!("\"{}\"", exe.display());
            let wide: Vec<u16> = quoted.encode_utf16().chain(std::iter::once(0)).collect();
            let bytes = std::slice::from_raw_parts(
                wide.as_ptr() as *const u8,
                wide.len() * 2,
            );
            RegSetValueExW(hkey, VALUE_NAME, None, REG_SZ, Some(bytes))
        } else {
            let err = RegDeleteValueW(hkey, VALUE_NAME);
            if err == ERROR_SUCCESS || err == ERROR_FILE_NOT_FOUND {
                ERROR_SUCCESS
            } else {
                err
            }
        };
        let _ = RegCloseKey(hkey);
        err == ERROR_SUCCESS
    };
    ok
}

pub fn toggle() -> bool {
    let now = !is_enabled();
    set_enabled(now);
    is_enabled()
}
