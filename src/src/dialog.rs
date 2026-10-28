use std::path::PathBuf;

use windows::Win32::Foundation::{HWND, LPARAM, TRUE};
use windows::Win32::System::Threading::GetCurrentProcessId;
use windows::Win32::UI::Controls::Dialogs::{
    GetOpenFileNameW, GetSaveFileNameW, OPENFILENAMEW, OFN_FILEMUSTEXIST, OFN_OVERWRITEPROMPT,
    OFN_PATHMUSTEXIST,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetWindowTextW, GetWindowThreadProcessId, IsWindowVisible, SetWindowPos,
    HWND_NOTOPMOST, HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
};

fn filter() -> Vec<u16> {
    let mut s = String::new();
    s.push_str("Backup b-snip");
    s.push('\0');
    s.push_str("*.zip");
    s.push('\0');
    s.push_str("Todos");
    s.push('\0');
    s.push_str("*.*");
    s.push('\0');
    s.push('\0');
    s.encode_utf16().collect()
}

fn ofn(buf: &mut [u16], filter: &mut [u16], title: &mut [u16], owner: HWND) -> OPENFILENAMEW {
    let mut ofn = OPENFILENAMEW::default();
    ofn.lStructSize = std::mem::size_of::<OPENFILENAMEW>() as u32;
    ofn.hwndOwner = owner;
    ofn.lpstrFile = windows::core::PWSTR(buf.as_mut_ptr());
    ofn.nMaxFile = buf.len() as u32;
    ofn.lpstrFilter = windows::core::PCWSTR(filter.as_ptr());
    ofn.nFilterIndex = 1;
    ofn.lpstrTitle = windows::core::PCWSTR(title.as_ptr());
    ofn.Flags = OFN_PATHMUSTEXIST | OFN_OVERWRITEPROMPT;
    ofn
}

fn from_wide(buf: &[u16]) -> Option<PathBuf> {
    let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    if len == 0 {
        return None;
    }
    Some(PathBuf::from(String::from_utf16_lossy(&buf[..len])))
}

struct EnumCtx {
    pid: u32,
    hwnds: Vec<HWND>,
}

unsafe extern "system" fn enum_cb(hwnd: HWND, lparam: LPARAM) -> windows::core::BOOL {
    let ctx = unsafe { &mut *(lparam.0 as *mut EnumCtx) };
    let mut pid = 0u32;
    unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
    if pid == ctx.pid && unsafe { IsWindowVisible(hwnd).as_bool() } {
        ctx.hwnds.push(hwnd);
    }
    TRUE
}

fn our_visible_windows() -> Vec<HWND> {
    let mut ctx = EnumCtx {
        pid: unsafe { GetCurrentProcessId() },
        hwnds: Vec::new(),
    };
    unsafe {
        let _ = EnumWindows(Some(enum_cb), LPARAM(&mut ctx as *mut EnumCtx as isize));
    }
    ctx.hwnds
}

fn window_title(hwnd: HWND) -> String {
    let mut buf = [0u16; 128];
    let n = unsafe { GetWindowTextW(hwnd, &mut buf) };
    String::from_utf16_lossy(&buf[..n as usize])
}

fn pick_owner(hwnds: &[HWND]) -> HWND {
    for &h in hwnds {
        let t = window_title(h);
        if t == "Expansor" || t == "Conteúdo do gatilho" || t == "Dados" {
            return h;
        }
    }
    hwnds.first().copied().unwrap_or_default()
}

fn set_topmost(hwnds: &[HWND], top: bool) {
    let after = if top { HWND_TOPMOST } else { HWND_NOTOPMOST };
    for &h in hwnds {
        let _ = unsafe {
            SetWindowPos(
                h,
                Some(after),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            )
        };
    }
}

fn with_native_dialog<T>(f: impl FnOnce(HWND) -> T) -> T {
    let wins = our_visible_windows();
    set_topmost(&wins, false);
    let owner = pick_owner(&wins);
    let result = f(owner);
    set_topmost(&wins, true);
    result
}

pub fn save_backup() -> Option<PathBuf> {
    with_native_dialog(|owner| {
        let mut buf = [0u16; 520];
        let stamp = chrono_stamp();
        let suggested: Vec<u16> = format!("b-snip-{stamp}.zip")
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let n = suggested.len().min(buf.len());
        buf[..n].copy_from_slice(&suggested[..n]);
        let mut filter = filter();
        let mut title: Vec<u16> = "Exportar backup\0".encode_utf16().collect();
        let mut ofn = ofn(&mut buf, &mut filter, &mut title, owner);
        ofn.Flags = OFN_PATHMUSTEXIST | OFN_OVERWRITEPROMPT;
        unsafe {
            if GetSaveFileNameW(&mut ofn).as_bool() {
                from_wide(&buf)
            } else {
                None
            }
        }
    })
}

pub fn open_backup() -> Option<PathBuf> {
    with_native_dialog(|owner| {
        let mut buf = [0u16; 520];
        let mut filter = filter();
        let mut title: Vec<u16> = "Importar backup\0".encode_utf16().collect();
        let mut ofn = ofn(&mut buf, &mut filter, &mut title, owner);
        ofn.Flags = OFN_PATHMUSTEXIST | OFN_FILEMUSTEXIST;
        unsafe {
            if GetOpenFileNameW(&mut ofn).as_bool() {
                from_wide(&buf)
            } else {
                None
            }
        }
    })
}

fn chrono_stamp() -> String {
    use windows::Win32::System::SystemInformation::GetLocalTime;
    let st = unsafe { GetLocalTime() };
    format!("{:04}-{:02}-{:02}", st.wYear, st.wMonth, st.wDay)
}
