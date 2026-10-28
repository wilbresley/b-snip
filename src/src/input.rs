use std::mem::size_of;
use std::thread::sleep;
use std::time::{Duration, Instant};

use std::ffi::c_void;

use windows::Win32::Foundation::HWND;
use windows::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, MapVirtualKeyW, SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT,
    KEYBD_EVENT_FLAGS, KEYEVENTF_EXTENDEDKEY, KEYEVENTF_KEYUP, KEYEVENTF_UNICODE, MAPVK_VK_TO_VSC,
    VIRTUAL_KEY, VK_BACK, VK_C, VK_CONTROL, VK_LCONTROL, VK_LEFT, VK_LMENU, VK_LSHIFT, VK_MENU,
    VK_RCONTROL, VK_RMENU, VK_RSHIFT, VK_SHIFT, VK_V,
};
use windows::Win32::UI::WindowsAndMessaging::{
    BringWindowToTop, GetForegroundWindow, GetWindowThreadProcessId, IsWindow, SetForegroundWindow,
};

#[link(name = "kernel32")]
extern "system" {
    fn Beep(dwFreq: u32, dwDuration: u32) -> i32;
}

pub fn beep(freq: u32, ms: u32) {
    unsafe {
        Beep(freq, ms);
    }
}

pub fn key_down(vk: VIRTUAL_KEY) -> bool {
    unsafe { GetAsyncKeyState(vk.0 as i32) as u16 & 0x8000 != 0 }
}

pub fn any_modifier_down() -> bool {
    key_down(VK_CONTROL)
        || key_down(VK_LCONTROL)
        || key_down(VK_RCONTROL)
        || key_down(VK_MENU)
        || key_down(VK_LMENU)
        || key_down(VK_RMENU)
        || key_down(VK_SHIFT)
        || key_down(VK_LSHIFT)
        || key_down(VK_RSHIFT)
}

/// No ABNT2, Ctrl+Alt+C gera ₢. Solta os modificadores e espera o usuário largar o atalho.
pub fn wait_modifiers_up(timeout: Duration) {
    let start = Instant::now();
    release_all_modifiers();
    while start.elapsed() < timeout {
        if !any_modifier_down() {
            break;
        }
        release_all_modifiers();
        sleep(Duration::from_millis(15));
    }
    release_all_modifiers();
    sleep(Duration::from_millis(30));
}

pub fn release_all_modifiers() {
    send_inputs(&[
        vk_input(VK_RMENU, true, true),
        vk_input(VK_LMENU, true, false),
        vk_input(VK_MENU, true, false),
        vk_input(VK_RCONTROL, true, true),
        vk_input(VK_LCONTROL, true, false),
        vk_input(VK_CONTROL, true, false),
        vk_input(VK_RSHIFT, true, false),
        vk_input(VK_LSHIFT, true, false),
        vk_input(VK_SHIFT, true, false),
    ]);
}

pub fn send_ctrl_c() {
    send_mod_key(VK_CONTROL, VK_C);
}

pub fn send_ctrl_v() {
    send_mod_key(VK_CONTROL, VK_V);
}

pub fn current_foreground() -> isize {
    unsafe { GetForegroundWindow().0 as isize }
}

/// Devolve o foco ao editor antes de colar/selecionar (a janela do transform rouba o foco).
pub fn focus_window(hwnd_bits: isize) {
    if hwnd_bits == 0 {
        return;
    }
    let hwnd = HWND(hwnd_bits as *mut c_void);
    unsafe {
        if !IsWindow(Some(hwnd)).as_bool() {
            return;
        }
        let fg = GetForegroundWindow();
        let fg_tid = GetWindowThreadProcessId(fg, None);
        let tgt_tid = GetWindowThreadProcessId(hwnd, None);
        let cur = GetCurrentThreadId();
        if fg_tid != 0 && fg_tid != cur {
            let _ = AttachThreadInput(cur, fg_tid, true);
        }
        if tgt_tid != 0 && tgt_tid != cur && tgt_tid != fg_tid {
            let _ = AttachThreadInput(cur, tgt_tid, true);
        }
        let _ = SetForegroundWindow(hwnd);
        let _ = BringWindowToTop(hwnd);
        if fg_tid != 0 && fg_tid != cur {
            let _ = AttachThreadInput(cur, fg_tid, false);
        }
        if tgt_tid != 0 && tgt_tid != cur && tgt_tid != fg_tid {
            let _ = AttachThreadInput(cur, tgt_tid, false);
        }
    }
}

/// Depois do colar, o cursor fica no fim. Shift+Left seleciona o trecho recém-colado.
/// Ctrl tem que estar solto: senão vira Ctrl+Shift+Left (seleção por palavra) e estoura o trecho.
#[allow(dead_code)]
pub fn select_chars_left(count: usize) {
    let n = count.min(20_000);
    if n == 0 {
        return;
    }
    wait_modifiers_up(Duration::from_millis(250));
    let mut inputs = Vec::with_capacity(n * 2 + 4);
    inputs.push(vk_input(VK_CONTROL, true, false));
    inputs.push(vk_input(VK_LCONTROL, true, false));
    inputs.push(vk_input(VK_SHIFT, false, false));
    for _ in 0..n {
        inputs.push(vk_input(VK_LEFT, false, true));
        inputs.push(vk_input(VK_LEFT, true, true));
    }
    inputs.push(vk_input(VK_SHIFT, true, false));
    send_inputs(&inputs);
}

pub fn send_text_replacing(already_in_app: usize, text: &str) {
    let mut inputs = Vec::with_capacity(already_in_app * 2 + text.encode_utf16().count() * 2 + 4);
    for _ in 0..already_in_app {
        inputs.push(vk_input(VK_BACK, false, false));
        inputs.push(vk_input(VK_BACK, true, false));
    }
    push_unicode_text(&mut inputs, text);
    send_inputs(&inputs);
}

fn send_mod_key(modifier: VIRTUAL_KEY, key: VIRTUAL_KEY) {
    send_inputs(&[
        vk_input(VK_MENU, true, false),
        vk_input(VK_LMENU, true, false),
        vk_input(VK_RMENU, true, true),
        vk_input(VK_SHIFT, true, false),
        vk_input(modifier, false, false),
        vk_input(key, false, false),
        vk_input(key, true, false),
        vk_input(modifier, true, false),
    ]);
}

fn send_inputs(inputs: &[INPUT]) {
    if inputs.is_empty() {
        return;
    }
    unsafe {
        SendInput(inputs, size_of::<INPUT>() as i32);
    }
}

fn vk_input(vk: VIRTUAL_KEY, up: bool, extended: bool) -> INPUT {
    let scan = unsafe { MapVirtualKeyW(vk.0 as u32, MAPVK_VK_TO_VSC) as u16 };
    let mut flags = if up {
        KEYEVENTF_KEYUP
    } else {
        KEYBD_EVENT_FLAGS(0)
    };
    if extended {
        flags |= KEYEVENTF_EXTENDEDKEY;
    }
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: scan,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

fn unicode_input(code: u16, up: bool) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(0),
                wScan: code,
                dwFlags: if up {
                    KEYEVENTF_UNICODE | KEYEVENTF_KEYUP
                } else {
                    KEYEVENTF_UNICODE
                },
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

fn push_unicode_text(inputs: &mut Vec<INPUT>, text: &str) {
    for ch in text.encode_utf16() {
        inputs.push(unicode_input(ch, false));
        inputs.push(unicode_input(ch, true));
    }
}
