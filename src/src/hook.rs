use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{sync_channel, Receiver, SyncSender};
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

use windows::core::w;
use windows::Win32::Foundation::{
    GetLastError, ERROR_ALREADY_EXISTS, HINSTANCE, HWND, LPARAM, LRESULT, WPARAM,
};
use windows::Win32::System::DataExchange::{
    AddClipboardFormatListener, RemoveClipboardFormatListener,
};
use windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress, LoadLibraryW};
use windows::Win32::System::Threading::{CreateMutexW, GetCurrentProcessId};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, VK_1, VK_2, VK_3, VK_4, VK_BACK, VK_C, VK_CAPITAL, VK_CONTROL, VK_DELETE, VK_E,
    VK_ESCAPE, VK_H, VK_LCONTROL, VK_LMENU, VK_LSHIFT, VK_MENU, VK_P, VK_Q, VK_R, VK_RCONTROL,
    VK_RETURN, VK_RMENU, VK_RSHIFT, VK_S, VK_SHIFT, VK_T, VK_TAB, VK_V, VK_X,
};
use windows::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CallNextHookEx, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyMenu,
    DestroyWindow, DispatchMessageW, GetCursorPos, GetForegroundWindow, GetMessageW,
    GetWindowThreadProcessId, KillTimer, LoadIconW, PostQuitMessage, RegisterClassExW,
    SetForegroundWindow, SetTimer, SetWindowsHookExW, TrackPopupMenu, TranslateMessage,
    UnhookWindowsHookEx, CS_HREDRAW, CS_VREDRAW, HHOOK, HMENU, HWND_MESSAGE, IDI_APPLICATION,
    KBDLLHOOKSTRUCT, MF_CHECKED, MF_DISABLED, MF_GRAYED, MF_SEPARATOR, MF_STRING, MSG,
    TPM_LEFTALIGN, TPM_RETURNCMD, TPM_RIGHTBUTTON, WH_KEYBOARD_LL, WINDOW_EX_STYLE, WM_APP,
    WM_CLIPBOARDUPDATE, WM_COMMAND, WM_DESTROY, WM_LBUTTONUP, WM_QUIT, WM_RBUTTONUP, WM_TIMER,
    WNDCLASSEXW, WS_POPUP,
};

use crate::clipboard;
use crate::input;
use crate::icon;
use crate::payload::ClipPayload;
use crate::state::{lock, try_lock, Shared, TransformKind};
use crate::text::{str_lower, str_title, str_upper, today_br};

const WM_TRAY: u32 = WM_APP + 1;
const ID_HELP: u32 = 1;
const ID_CONFIG: u32 = 2;
const ID_CLEAR: u32 = 3;
const ID_EXIT: u32 = 4;
const ID_TOGGLE_EXPANDER: u32 = 5;
const ID_TOGGLE_COPY: u32 = 6;
const ID_TOGGLE_TRANSFORM: u32 = 7;
const ID_TOGGLE_LOOP: u32 = 8;
const ID_TOGGLE_STARTUP: u32 = 9;
const ID_TOGGLE_SOUND: u32 = 10;
const ID_RESTART: u32 = 11;
const ID_TOGGLE_PAUSE: u32 = 12;
const ID_TOGGLE_CLEAR_LAST: u32 = 13;
const LLKHF_INJECTED: u32 = 0x10;
const LLKHF_UP: u32 = 0x80;
/// Não pode coincidir com o id do timer de Ctrl+C (copy_gen começa em 1).
const TIMER_WATCHDOG: usize = 0x00B5_1101;
const WATCHDOG_MS: u32 = 30_000;
const PAUSE_FOREIGN_MSG: &str =
    "Fila pausada: entrou arquivo ou imagem. Clique no ícone para voltar.";

static CLIP_FROM_US: AtomicBool = AtomicBool::new(false);
static V_HELD: AtomicBool = AtomicBool::new(false);
const MUTEX_NAME: windows::core::PCWSTR = w!("Local\\b-snip-ClipboardExpander");

static WORK_TX: OnceLock<SyncSender<WorkCmd>> = OnceLock::new();
static SHARED: OnceLock<Shared> = OnceLock::new();
static UI_CTX: Mutex<Option<egui::Context>> = Mutex::new(None);
static TYPE_BUF: Mutex<String> = Mutex::new(String::new());
static mut HOOK: HHOOK = HHOOK(std::ptr::null_mut());
static mut TRAY_HWND: HWND = HWND(std::ptr::null_mut());
static INSTANCE_MUTEX: Mutex<isize> = Mutex::new(0);

#[derive(Debug)]
pub enum WorkCmd {
    ClipboardChanged,
    UserCopy { gen: u64 },
    Paste { unformatted: bool },
    /// Ctrl+V real já passou. Depois de um instante, prepara o próximo item.
    FinishPassthrough { seq: u64 },
    ClearQueue,
    BeginTransform,
    Transform(TransformKind),
    TransformCancel,
    Expand { backspaces: usize, payload: ClipPayload },
    OpenConfig,
    ToggleHelp,
    ToggleCopyQueue,
    ToggleExpander,
    ToggleTransform,
    ToggleLoop,
    ToggleStartup,
    ToggleSound,
    TogglePause,
    ToggleClearLast,
    TrayLeftClick,
    Reload,
    Exit,
}

pub fn send_cmd(cmd: WorkCmd) {
    if let Some(tx) = WORK_TX.get() {
        let _ = tx.try_send(cmd);
    }
}

pub fn set_ui_ctx(ctx: egui::Context) {
    if let Ok(mut g) = UI_CTX.lock() {
        *g = Some(ctx);
    }
}

fn request_ui() {
    if let Ok(g) = UI_CTX.lock() {
        if let Some(ctx) = g.as_ref() {
            ctx.request_repaint();
        }
    }
}

pub fn try_single_instance() -> bool {
    unsafe {
        match CreateMutexW(None, true, MUTEX_NAME) {
            Ok(handle) => {
                if GetLastError() == ERROR_ALREADY_EXISTS {
                    let _ = windows::Win32::Foundation::CloseHandle(handle);
                    return false;
                }
                if let Ok(mut g) = INSTANCE_MUTEX.lock() {
                    *g = handle.0 as isize;
                }
                true
            }
            Err(_) => true,
        }
    }
}

pub fn release_instance_mutex() {
    if let Ok(mut g) = INSTANCE_MUTEX.lock() {
        if *g != 0 {
            unsafe {
                let _ = windows::Win32::Foundation::CloseHandle(windows::Win32::Foundation::HANDLE(
                    *g as *mut std::ffi::c_void,
                ));
            }
            *g = 0;
        }
    }
}

pub fn start(shared: Shared) {
    let _ = SHARED.set(shared.clone());
    let (tx, rx) = sync_channel::<WorkCmd>(64);
    let _ = WORK_TX.set(tx);

    thread::Builder::new()
        .name("b-snip-worker".into())
        .spawn(move || worker_loop(shared.clone(), rx))
        .expect("worker");

    thread::Builder::new()
        .name("b-snip-hook".into())
        .spawn(message_thread)
        .expect("hook thread");
}

fn worker_loop(shared: Shared, rx: Receiver<WorkCmd>) {
    while let Ok(cmd) = rx.recv() {
        let shared = shared.clone();
        let _ = catch_unwind(AssertUnwindSafe(move || match cmd {
            WorkCmd::ClipboardChanged => on_clipboard_changed(&shared),
            WorkCmd::UserCopy { gen } => on_user_copy(&shared, gen),
            WorkCmd::Paste { unformatted } => on_paste(&shared, unformatted),
            WorkCmd::FinishPassthrough { seq } => on_finish_passthrough(&shared, seq),
            WorkCmd::ClearQueue => on_clear_queue(&shared),
            WorkCmd::BeginTransform => on_begin_transform(&shared),
            WorkCmd::Transform(kind) => on_transform(&shared, kind),
            WorkCmd::TransformCancel => on_transform_cancel(&shared),
            WorkCmd::Expand { backspaces, payload } => on_expand(&shared, backspaces, payload),
            WorkCmd::OpenConfig => {
                {
                    let mut s = lock(&shared);
                    s.load_drafts_from_db();
                    s.config_visible = true;
                    s.config_just_opened = true;
                }
                request_ui();
            }
            WorkCmd::ToggleHelp => {
                {
                    let mut s = lock(&shared);
                    s.help_visible = !s.help_visible;
                }
                request_ui();
            }
            WorkCmd::ToggleCopyQueue => {
                let on = {
                    let mut s = lock(&shared);
                    let on = s.toggle_copy_queue();
                    if !on {
                        s.ignore_clip_until = Some(Instant::now() + Duration::from_millis(600));
                    }
                    s.persist();
                    on
                };
                if !on {
                    clipboard::clear();
                }
                request_ui();
            }
            WorkCmd::ToggleExpander => {
                {
                    let mut s = lock(&shared);
                    s.expander_on = !s.expander_on;
                    let on = s.expander_on;
                    s.show_toast(if on {
                        "Expansor ligado"
                    } else {
                        "Expansor desligado"
                    });
                    s.persist();
                }
                request_ui();
            }
            WorkCmd::ToggleTransform => {
                {
                    let mut s = lock(&shared);
                    s.transform_on = !s.transform_on;
                    let on = s.transform_on;
                    s.show_toast(if on {
                        "Transformador ligado"
                    } else {
                        "Transformador desligado"
                    });
                    s.persist();
                }
                request_ui();
            }
            WorkCmd::ToggleLoop => {
                {
                    let mut s = lock(&shared);
                    s.loop_on = !s.loop_on;
                    if s.loop_on {
                        s.done_count = 0;
                        s.clear_on_last = false;
                    }
                    let on = s.loop_on;
                    s.show_toast(if on {
                        "Loop ligado"
                    } else {
                        "Loop desligado"
                    });
                    s.persist();
                }
                request_ui();
            }
            WorkCmd::ToggleClearLast => {
                {
                    let mut s = lock(&shared);
                    s.clear_on_last = !s.clear_on_last;
                    if s.clear_on_last {
                        s.loop_on = false;
                    }
                    let on = s.clear_on_last;
                    s.show_toast(if on {
                        "Limpar ao colar o último ligado"
                    } else {
                        "Limpar ao colar o último desligado"
                    });
                    s.persist();
                }
                request_ui();
            }
            WorkCmd::TrayLeftClick => {
                let clear_win = {
                    let mut s = lock(&shared);
                    if s.copy_queue_on && !s.queue.is_empty() {
                        s.queue_paused = !s.queue_paused;
                        let paused = s.queue_paused;
                        s.show_toast(if paused {
                            "Fila pausada"
                        } else {
                            "Fila retomada"
                        });
                        false
                    } else {
                        let on = s.toggle_copy_queue();
                        if !on {
                            s.ignore_clip_until = Some(Instant::now() + Duration::from_millis(600));
                        }
                        s.persist();
                        !on
                    }
                };
                if clear_win {
                    clipboard::clear();
                }
                request_ui();
            }
            WorkCmd::ToggleStartup => {
                let on = crate::startup::toggle();
                {
                    let mut s = lock(&shared);
                    s.show_toast(if on {
                        "Início com o sistema ligado"
                    } else {
                        "Início com o sistema desligado"
                    });
                }
                request_ui();
            }
            WorkCmd::ToggleSound => {
                {
                    let mut s = lock(&shared);
                    s.sound_on = !s.sound_on;
                    let on = s.sound_on;
                    s.show_toast(if on {
                        "Som ligado"
                    } else {
                        "Som desligado"
                    });
                    s.persist();
                }
                request_ui();
            }
            WorkCmd::TogglePause => {
                {
                    let mut s = lock(&shared);
                    if !s.copy_queue_on {
                        s.show_toast("Cópia contínua desligada");
                    } else {
                        s.queue_paused = !s.queue_paused;
                        let paused = s.queue_paused;
                        s.show_toast(if paused {
                            "Fila pausada"
                        } else {
                            "Fila retomada"
                        });
                    }
                }
                request_ui();
            }
            WorkCmd::Reload => restart_process(),
            WorkCmd::Exit => {
                lock(&shared).request_exit = true;
                request_ui();
            }
        }));
    }
}

fn release_stuck_pasting(shared: &Shared) {
    let stuck = lock(shared).release_stuck_pasting();
    if stuck {
        V_HELD.store(false, Ordering::SeqCst);
    }
}

fn pause_for_foreign_clip(shared: &Shared) {
    let mut s = lock(shared);
    if s.copy_queue_on && !s.queue_paused && !s.queue.is_empty() {
        s.queue_paused = true;
        s.show_toast(PAUSE_FOREIGN_MSG);
    }
}

fn on_clipboard_changed(shared: &Shared) {
    if CLIP_FROM_US.load(Ordering::SeqCst) {
        return;
    }
    release_stuck_pasting(shared);
    if clipboard::is_file_or_image_only() {
        pause_for_foreign_clip(shared);
        request_ui();
        return;
    }
    {
        let s = lock(shared);
        if s.should_ignore_clipboard() {
            return;
        }
    }
    let Some(payload) = clipboard::capture() else {
        return;
    };
    {
        let mut s = lock(shared);
        if s.should_ignore_clipboard() {
            return;
        }
        if s.skip_clip.as_ref().is_some_and(|p| payload.same_as(p)) {
            s.skip_clip = None;
            return;
        }
        // UPDATE duplicado do Windows: ignora. Ctrl+C de novo no mesmo texto
        // entra pelo timer (UserCopy), não por aqui.
        if payload.same_as(&s.last_payload) {
            return;
        }
        s.last_enqueued_gen = s.copy_gen;
        s.try_enqueue(payload);
    }
    request_ui();
}

fn note_user_copy() {
    let Some(shared) = SHARED.get() else {
        return;
    };
    let Some(mut s) = try_lock(shared) else {
        return;
    };
    if !s.copy_queue_on || s.queue_paused {
        return;
    }
    s.copy_gen = s.copy_gen.wrapping_add(1);
    if s.copy_gen == 0 {
        s.copy_gen = 1;
    }
    let gen = s.copy_gen;
    drop(s);
    // Espera o Windows terminar de gravar o clipboard. Abrir no meio da cópia
    // (thread extra + loop) derrubava o processo.
    schedule_user_copy(gen);
}

fn schedule_user_copy(gen: u64) {
    unsafe {
        let hwnd = TRAY_HWND;
        let id = gen as usize;
        if hwnd.0.is_null() || id == 0 || id == TIMER_WATCHDOG {
            send_cmd(WorkCmd::UserCopy { gen });
            return;
        }
        if SetTimer(Some(hwnd), id, 200, None) == 0 {
            send_cmd(WorkCmd::UserCopy { gen });
        }
    }
}

fn on_user_copy(shared: &Shared, gen: u64) {
    if CLIP_FROM_US.load(Ordering::SeqCst) {
        return;
    }
    release_stuck_pasting(shared);
    {
        let s = lock(shared);
        if s.last_enqueued_gen >= gen || s.should_ignore_clipboard() {
            return;
        }
    }
    if clipboard::is_file_or_image_only() {
        lock(shared).last_enqueued_gen = gen;
        pause_for_foreign_clip(shared);
        request_ui();
        return;
    }
    let Some(payload) = clipboard::capture() else {
        return;
    };
    {
        let mut s = lock(shared);
        if s.last_enqueued_gen >= gen || s.should_ignore_clipboard() {
            return;
        }
        if s.skip_clip.as_ref().is_some_and(|p| payload.same_as(p)) {
            s.skip_clip = None;
            s.last_enqueued_gen = gen;
            return;
        }
        let same = payload.same_as(&s.last_payload);
        if !s.accept_repeat_copy(same) {
            return;
        }
        s.last_enqueued_gen = gen;
        s.try_enqueue(payload);
    }
    request_ui();
}

fn on_paste(shared: &Shared, unformatted: bool) {
    release_stuck_pasting(shared);
    input::wait_modifiers_up(Duration::from_millis(400));

    let item = {
        let mut s = lock(shared);
        if s.is_pasting {
            None
        } else if !s.copy_queue_on || s.queue_paused || s.queue.is_empty() {
            s.begin_pasting();
            s.ignore_clip_until = Some(Instant::now() + Duration::from_millis(400));
            Some((None, false))
        } else {
            s.begin_pasting();
            s.ignore_clip_until = Some(Instant::now() + Duration::from_millis(500));
            let payload = if s.loop_on {
                if s.next_index >= s.queue.len() {
                    s.next_index = 0;
                }
                let payload = s.queue[s.next_index].clone();
                s.next_index = (s.next_index + 1) % s.queue.len();
                payload
            } else {
                let payload = s.queue.pop_front().unwrap_or_default();
                s.done_count += 1;
                if s.queue.is_empty() {
                    s.done_count = 0;
                    s.next_index = 0;
                }
                payload
            };
            let clear_last = !s.loop_on && s.clear_on_last && s.queue.is_empty();
            if clear_last {
                s.clear_queue_data();
            }
            Some((Some(payload), clear_last))
        }
    };

    // Solta is_pasting mesmo se a colagem falhar no meio. O ramo None é outra colagem ainda viva.
    struct PastingGuard<'a>(&'a Shared, bool);
    impl Drop for PastingGuard<'_> {
        fn drop(&mut self) {
            if self.1 {
                lock(self.0).end_pasting();
            }
        }
    }
    let _guard = PastingGuard(shared, item.is_some());

    let mut play_clear = false;
    match item {
        None => {
            input::send_ctrl_v();
            return;
        }
        Some((None, _)) => {
            input::send_ctrl_v();
        }
        Some((Some(payload), clear_last)) => {
            if unformatted {
                let _ = clipboard::set_unicode_text(&payload.unicode);
            } else {
                let _ = clipboard::set_payload(&payload);
            }
            thread::sleep(Duration::from_millis(50));
            input::send_ctrl_v();
            thread::sleep(Duration::from_millis(80));
            play_clear = clear_last;
        }
    }

    if play_clear {
        thread::sleep(Duration::from_millis(120));
        clipboard::clear();
        {
            let mut s = lock(shared);
            s.ignore_clip_until = Some(Instant::now() + Duration::from_millis(600));
            s.show_toast("Área de transferência limpa");
        }
        crate::sound::play_notify(shared);
    }
    request_ui();
}

fn on_finish_passthrough(shared: &Shared, seq: u64) {
    // A tecla real já colou. Espera o jogo ler o clipboard antes de esvaziar.
    thread::sleep(Duration::from_millis(150));
    let should_clear = {
        let s = lock(shared);
        s.pending_clear_seq == Some(seq) && s.paste_seq == seq
    };
    if !should_clear {
        return;
    }
    clipboard::clear();
    {
        let mut s = lock(shared);
        if s.pending_clear_seq == Some(seq) && s.paste_seq == seq {
            s.pending_clear_seq = None;
            s.ignore_clip_until = Some(Instant::now() + Duration::from_millis(600));
            s.show_toast("Área de transferência limpa");
        }
    }
    crate::sound::play_notify(shared);
    request_ui();
}

enum PasteKey {
    /// A tecla física segue. O clipboard já está com o item, ou a fila não intercepta.
    LetRealThrough,
    /// Repetição da mesma tecla: não manda de novo.
    Swallow,
    /// Clipboard ocupado: segura a tecla e cola com SendInput, como antes.
    SwallowAndSynth,
}

fn decide_paste_key(unformatted: bool) -> PasteKey {
    let Some(shared) = SHARED.get() else {
        return PasteKey::SwallowAndSynth;
    };
    let Some(payload) = ({
        let Some(mut s) = try_lock(shared) else {
            return PasteKey::SwallowAndSynth;
        };
        if s.release_stuck_pasting() {
            V_HELD.store(false, Ordering::SeqCst);
        }
        if V_HELD.load(Ordering::SeqCst) || s.is_pasting || s.passthrough_armed {
            return PasteKey::Swallow;
        }
        if !s.copy_queue_on || s.queue_paused || s.queue.is_empty() {
            return PasteKey::LetRealThrough;
        }
        s.peek_paste_payload()
    }) else {
        return PasteKey::LetRealThrough;
    };

    CLIP_FROM_US.store(true, Ordering::SeqCst);
    struct ReleaseOurs;
    impl Drop for ReleaseOurs {
        fn drop(&mut self) {
            CLIP_FROM_US.store(false, Ordering::SeqCst);
        }
    }
    let _release = ReleaseOurs;
    let wrote = if unformatted {
        clipboard::try_set_payload(&ClipPayload::from_unicode(&payload.unicode))
    } else {
        clipboard::try_set_payload(&payload)
    };
    if !wrote {
        V_HELD.store(true, Ordering::SeqCst);
        return PasteKey::SwallowAndSynth;
    }

    let Some(mut s) = try_lock(shared) else {
        V_HELD.store(true, Ordering::SeqCst);
        return PasteKey::LetRealThrough;
    };
    s.paste_seq = s.paste_seq.wrapping_add(1);
    if s.paste_seq == 0 {
        s.paste_seq = 1;
    }
    let seq = s.paste_seq;
    s.begin_pasting();
    s.passthrough_armed = true;
    s.skip_clip = Some(if unformatted {
        ClipPayload::from_unicode(&payload.unicode)
    } else {
        payload
    });
    s.ignore_clip_until = Some(Instant::now() + Duration::from_millis(800));
    if s.commit_shown_paste() {
        s.pending_clear_seq = Some(seq);
    }
    drop(s);
    V_HELD.store(true, Ordering::SeqCst);
    request_ui();
    PasteKey::LetRealThrough
}

fn on_clear_queue(shared: &Shared) {
    {
        let mut s = lock(shared);
        s.clear_queue_data();
        s.begin_pasting();
        s.ignore_clip_until = Some(Instant::now() + Duration::from_millis(600));
        s.show_toast("Área de transferência limpa");
    }
    clipboard::clear();
    lock(shared).end_pasting();
    crate::sound::play_notify(shared);
    request_ui();
}

fn on_begin_transform(shared: &Shared) {
    let target = input::current_foreground();
    {
        let mut s = lock(shared);
        if !s.transform_on || s.transform_visible {
            return;
        }
        s.is_transforming = true;
        s.transform_hwnd = target;
    }
    // Espera largar Ctrl+Alt, senão o C vira ₢ no teclado ABNT.
    input::wait_modifiers_up(Duration::from_millis(800));
    clipboard::clear();
    thread::sleep(Duration::from_millis(20));
    input::send_ctrl_c();
    if !clipboard::wait_unicode(Duration::from_millis(700)) {
        lock(shared).is_transforming = false;
        request_ui();
        return;
    }
    let Some(payload) = clipboard::capture() else {
        lock(shared).is_transforming = false;
        request_ui();
        return;
    };
    if payload
        .unicode
        .chars()
        .all(|c| c == '₢' || c.is_whitespace())
        && !payload.has_rich()
    {
        lock(shared).is_transforming = false;
        request_ui();
        return;
    }
    {
        let mut s = lock(shared);
        s.saved_payload = payload;
        s.transform_visible = true;
        s.ignore_clip_until = Some(Instant::now() + Duration::from_millis(400));
    }
    request_ui();
}

fn on_transform(shared: &Shared, kind: TransformKind) {
    let (saved, target) = {
        let mut s = lock(shared);
        s.transform_visible = false;
        let payload = std::mem::take(&mut s.saved_payload);
        let hwnd = s.transform_hwnd;
        (payload, hwnd)
    };
    request_ui();
    if saved.is_empty() {
        lock(shared).is_transforming = false;
        return;
    }
    let map = match kind {
        TransformKind::Title => str_title,
        TransformKind::Upper => str_upper,
        TransformKind::Lower => str_lower,
    };
    let final_payload = saved.mapped(map);
    thread::sleep(Duration::from_millis(80));
    input::focus_window(target);
    thread::sleep(Duration::from_millis(40));
    input::wait_modifiers_up(Duration::from_millis(300));
    {
        let mut s = lock(shared);
        s.begin_pasting();
        s.ignore_clip_until = Some(Instant::now() + Duration::from_millis(500));
    }
    let _ = clipboard::set_payload(&final_payload);
    thread::sleep(Duration::from_millis(50));
    input::send_ctrl_v();
    thread::sleep(Duration::from_millis(80));
    {
        let mut s = lock(shared);
        s.end_pasting();
        s.is_transforming = false;
        s.saved_payload = ClipPayload::default();
        s.transform_hwnd = 0;
    }
    request_ui();
}

fn on_transform_cancel(shared: &Shared) {
    let mut s = lock(shared);
    s.transform_visible = false;
    s.is_transforming = false;
    s.saved_payload = ClipPayload::default();
    s.transform_hwnd = 0;
    request_ui();
}

fn on_expand(shared: &Shared, backspaces: usize, payload: ClipPayload) {
    {
        let mut s = lock(shared);
        if !s.expander_on || s.is_pasting {
            return;
        }
        s.begin_pasting();
        s.is_expanding = true;
        s.skip_clip = Some(payload.clone());
        s.ignore_clip_until = Some(Instant::now() + Duration::from_millis(1500));
    }
    input::send_text_replacing(backspaces, "");
    thread::sleep(Duration::from_millis(30));
    let _ = clipboard::set_payload(&payload);
    thread::sleep(Duration::from_millis(40));
    input::send_ctrl_v();
    thread::sleep(Duration::from_millis(80));
    {
        let mut s = lock(shared);
        s.end_pasting();
        s.is_expanding = false;
        s.ignore_clip_until = Some(Instant::now() + Duration::from_millis(800));
    }
}

fn install_keyboard_hook(module: HINSTANCE) -> HHOOK {
    unsafe {
        SetWindowsHookExW(WH_KEYBOARD_LL, Some(low_level_keyboard_proc), Some(module), 0)
            .unwrap_or_default()
    }
}

fn reinstall_input_hooks(hwnd: HWND) {
    if V_HELD.load(Ordering::SeqCst) {
        return;
    }
    unsafe {
        if !HOOK.0.is_null() {
            let _ = UnhookWindowsHookEx(HOOK);
            HOOK = HHOOK(std::ptr::null_mut());
        }
        let module = GetModuleHandleW(None).unwrap_or_default();
        HOOK = install_keyboard_hook(HINSTANCE(module.0));
        let _ = RemoveClipboardFormatListener(hwnd);
        let _ = AddClipboardFormatListener(hwnd);
    }
}

fn handle_keyup(vk: u32, injected: bool) -> bool {
    if injected || vk != VK_V.0 as u32 {
        return false;
    }
    if !V_HELD.swap(false, Ordering::SeqCst) {
        return false;
    }
    let Some(shared) = SHARED.get() else {
        return false;
    };
    let Some(mut s) = try_lock(shared) else {
        return false;
    };
    if !s.passthrough_armed {
        return false;
    }
    let seq = s.paste_seq;
    let clear_after = s.pending_clear_seq == Some(seq);
    s.end_pasting();
    drop(s);
    if clear_after {
        send_cmd(WorkCmd::FinishPassthrough { seq });
    }
    false
}

fn message_thread() {
    unsafe {
        let hinstance = GetModuleHandleW(None).unwrap_or_default();
        let class_name = w!("b-snip-MsgWnd");
        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wnd_proc),
            hInstance: HINSTANCE(hinstance.0),
            lpszClassName: class_name,
            ..Default::default()
        };
        RegisterClassExW(&wc);

        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            class_name,
            w!("b-snip"),
            WS_POPUP,
            0,
            0,
            0,
            0,
            Some(HWND_MESSAGE),
            None,
            Some(HINSTANCE(hinstance.0)),
            None,
        )
        .unwrap_or_default();

        TRAY_HWND = hwnd;
        let _ = AddClipboardFormatListener(hwnd);
        add_tray_icon(hwnd);

        HOOK = install_keyboard_hook(HINSTANCE(hinstance.0));
        let _ = SetTimer(Some(hwnd), TIMER_WATCHDOG, WATCHDOG_MS, None);

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        let _ = UnhookWindowsHookEx(HOOK);
        let _ = RemoveClipboardFormatListener(hwnd);
        remove_tray_icon(hwnd);
        let _ = DestroyWindow(hwnd);
    }
}

unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_CLIPBOARDUPDATE => {
            send_cmd(WorkCmd::ClipboardChanged);
            LRESULT(0)
        }
        WM_TIMER => {
            let id = wparam.0;
            if id == TIMER_WATCHDOG {
                reinstall_input_hooks(hwnd);
                LRESULT(0)
            } else {
                let _ = KillTimer(Some(hwnd), id);
                if id != 0 {
                    send_cmd(WorkCmd::UserCopy { gen: id as u64 });
                }
                LRESULT(0)
            }
        }
        WM_TRAY => {
            let mouse = (lparam.0 as u32) & 0xFFFF;
            if mouse == WM_LBUTTONUP {
                send_cmd(WorkCmd::TrayLeftClick);
            } else if mouse == WM_RBUTTONUP {
                show_tray_menu(hwnd);
            }
            LRESULT(0)
        }
        WM_COMMAND => {
            match (wparam.0 as u32) & 0xFFFF {
                ID_HELP => send_cmd(WorkCmd::ToggleHelp),
                ID_CONFIG => send_cmd(WorkCmd::OpenConfig),
                ID_CLEAR => send_cmd(WorkCmd::ClearQueue),
                ID_TOGGLE_EXPANDER => send_cmd(WorkCmd::ToggleExpander),
                ID_TOGGLE_COPY => send_cmd(WorkCmd::ToggleCopyQueue),
                ID_TOGGLE_TRANSFORM => send_cmd(WorkCmd::ToggleTransform),
                ID_TOGGLE_LOOP => send_cmd(WorkCmd::ToggleLoop),
                ID_TOGGLE_STARTUP => send_cmd(WorkCmd::ToggleStartup),
                ID_TOGGLE_SOUND => send_cmd(WorkCmd::ToggleSound),
                ID_TOGGLE_PAUSE => send_cmd(WorkCmd::TogglePause),
                ID_TOGGLE_CLEAR_LAST => send_cmd(WorkCmd::ToggleClearLast),
                ID_RESTART => send_cmd(WorkCmd::Reload),
                ID_EXIT => send_cmd(WorkCmd::Exit),
                _ => {}
            }
            LRESULT(0)
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

fn add_tray_icon(hwnd: HWND) {
    unsafe {
        let icon = icon::tray_hicon()
            .or_else(|| LoadIconW(None, IDI_APPLICATION).ok())
            .unwrap_or_default();
        let mut nid = NOTIFYICONDATAW {
            cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: hwnd,
            uID: 1,
            uFlags: NIF_ICON | NIF_MESSAGE | NIF_TIP,
            uCallbackMessage: WM_TRAY,
            hIcon: icon,
            ..Default::default()
        };
        let tip: Vec<u16> = "b-snip".encode_utf16().chain(std::iter::once(0)).collect();
        for (i, ch) in tip.iter().take(nid.szTip.len()).enumerate() {
            nid.szTip[i] = *ch;
        }
        let _ = Shell_NotifyIconW(NIM_ADD, &nid);
    }
}

fn remove_tray_icon(hwnd: HWND) {
    unsafe {
        let nid = NOTIFYICONDATAW {
            cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: hwnd,
            uID: 1,
            ..Default::default()
        };
        let _ = Shell_NotifyIconW(NIM_DELETE, &nid);
    }
}

fn show_tray_menu(hwnd: HWND) {
    enable_dark_popup_menus();
    let (copy_on, expander_on, transform_on, loop_on, sound_on, paused, clear_last) = SHARED
        .get()
        .and_then(try_lock)
        .map(|s| {
            (
                s.copy_queue_on,
                s.expander_on,
                s.transform_on,
                s.loop_on,
                s.sound_on,
                s.queue_paused,
                s.clear_on_last,
            )
        })
        .unwrap_or((true, true, true, true, true, false, false));

    unsafe {
        let menu = CreatePopupMenu().unwrap_or_default();
        if menu.0.is_null() {
            return;
        }

        append_header(menu, "Cópia");
        append_menu(menu, ID_CLEAR, "Limpar fila");
        append_check(menu, ID_TOGGLE_COPY, "Cópia contínua", copy_on);
        append_check(menu, ID_TOGGLE_PAUSE, "Pausa", paused);
        append_check(menu, ID_TOGGLE_LOOP, "Loop", loop_on);
        append_check(
            menu,
            ID_TOGGLE_CLEAR_LAST,
            "Limpar ao colar o último",
            clear_last,
        );

        let _ = AppendMenuW(menu, MF_SEPARATOR, 0, windows::core::PCWSTR::null());
        append_header(menu, "Expansor");
        append_menu(menu, ID_CONFIG, "Configurar");
        append_check(menu, ID_TOGGLE_EXPANDER, "Expansor ligado", expander_on);

        let _ = AppendMenuW(menu, MF_SEPARATOR, 0, windows::core::PCWSTR::null());
        append_header(menu, "Transformador");
        append_check(menu, ID_TOGGLE_TRANSFORM, "Transformador ligado", transform_on);

        let _ = AppendMenuW(menu, MF_SEPARATOR, 0, windows::core::PCWSTR::null());
        append_header(menu, "Sistema");
        let startup_on = crate::startup::is_enabled();
        append_check(menu, ID_TOGGLE_STARTUP, "Inicializar com o sistema", startup_on);
        append_check(menu, ID_TOGGLE_SOUND, "Som", sound_on);
        append_menu(menu, ID_RESTART, "Reiniciar");

        let _ = AppendMenuW(menu, MF_SEPARATOR, 0, windows::core::PCWSTR::null());
        append_menu(menu, ID_HELP, "Central de Ajuda");
        append_menu(menu, ID_EXIT, "Sair");

        let mut pt = windows::Win32::Foundation::POINT::default();
        let _ = GetCursorPos(&mut pt);
        let _ = SetForegroundWindow(hwnd);
        let cmd = TrackPopupMenu(
            menu,
            TPM_LEFTALIGN | TPM_RIGHTBUTTON | TPM_RETURNCMD,
            pt.x,
            pt.y,
            None,
            hwnd,
            None,
        );
        let _ = DestroyMenu(menu);
        if cmd.0 != 0 {
            match cmd.0 as u32 {
                ID_HELP => send_cmd(WorkCmd::ToggleHelp),
                ID_CONFIG => send_cmd(WorkCmd::OpenConfig),
                ID_CLEAR => send_cmd(WorkCmd::ClearQueue),
                ID_TOGGLE_EXPANDER => send_cmd(WorkCmd::ToggleExpander),
                ID_TOGGLE_COPY => send_cmd(WorkCmd::ToggleCopyQueue),
                ID_TOGGLE_TRANSFORM => send_cmd(WorkCmd::ToggleTransform),
                ID_TOGGLE_LOOP => send_cmd(WorkCmd::ToggleLoop),
                ID_TOGGLE_STARTUP => send_cmd(WorkCmd::ToggleStartup),
                ID_TOGGLE_SOUND => send_cmd(WorkCmd::ToggleSound),
                ID_TOGGLE_PAUSE => send_cmd(WorkCmd::TogglePause),
                ID_TOGGLE_CLEAR_LAST => send_cmd(WorkCmd::ToggleClearLast),
                ID_RESTART => send_cmd(WorkCmd::Reload),
                ID_EXIT => send_cmd(WorkCmd::Exit),
                _ => {}
            }
        }
    }
}

fn append_header(menu: HMENU, text: &str) {
    append_menu_flags(menu, 0, text, MF_STRING | MF_DISABLED | MF_GRAYED);
}

fn append_check(menu: HMENU, id: u32, text: &str, on: bool) {
    append_menu_flags(
        menu,
        id,
        text,
        if on {
            MF_STRING | MF_CHECKED
        } else {
            MF_STRING
        },
    );
}

fn enable_dark_popup_menus() {
    use windows::core::PCSTR;
    static DONE: OnceLock<()> = OnceLock::new();
    if DONE.get().is_some() {
        return;
    }
    unsafe {
        let Ok(lib) = LoadLibraryW(w!("uxtheme.dll")) else {
            return;
        };
        // Win10 1903+: SetPreferredAppMode = ordinal 135; FlushMenuThemes = 136.
        let set_mode = GetProcAddress(lib, PCSTR(135usize as *const u8));
        let flush = GetProcAddress(lib, PCSTR(136usize as *const u8));
        if let Some(f) = set_mode {
            let f: unsafe extern "system" fn(i32) -> i32 = std::mem::transmute(f);
            let _ = f(2); // ForceDark
        }
        if let Some(f) = flush {
            let f: unsafe extern "system" fn() = std::mem::transmute(f);
            f();
        }
    }
    let _ = DONE.set(());
}

fn append_menu(menu: HMENU, id: u32, text: &str) {
    append_menu_flags(menu, id, text, MF_STRING);
}

fn append_menu_flags(
    menu: HMENU,
    id: u32,
    text: &str,
    flags: windows::Win32::UI::WindowsAndMessaging::MENU_ITEM_FLAGS,
) {
    let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        let _ = AppendMenuW(
            menu,
            flags,
            id as usize,
            windows::core::PCWSTR(wide.as_ptr()),
        );
    }
}

unsafe extern "system" fn low_level_keyboard_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
        let eat = catch_unwind(AssertUnwindSafe(|| {
            let info = &*(lparam.0 as *const KBDLLHOOKSTRUCT);
            let flags = info.flags.0;
            let is_up = flags & LLKHF_UP != 0;
            let injected = flags & LLKHF_INJECTED != 0;
            let vk = info.vkCode;
            let scan = info.scanCode;
            if is_up {
                handle_keyup(vk, injected)
            } else {
                handle_keydown(vk, scan, injected)
            }
        }))
        .unwrap_or(false);
        if eat {
            return LRESULT(1);
        }
    }
    CallNextHookEx(None, code, wparam, lparam)
}

fn handle_keydown(vk: u32, _scan: u32, injected: bool) -> bool {
    if injected {
        return false;
    }

    let ctrl = input::key_down(VK_CONTROL)
        || input::key_down(VK_LCONTROL)
        || input::key_down(VK_RCONTROL);
    let shift = input::key_down(VK_SHIFT) || input::key_down(VK_LSHIFT) || input::key_down(VK_RSHIFT);
    let alt = input::key_down(VK_MENU) || input::key_down(VK_LMENU) || input::key_down(VK_RMENU);

    let flags = SHARED.get().and_then(try_lock).map(|s| {
        (
            s.transform_visible,
            s.config_visible,
            s.copy_queue_on && !s.queue_paused && !s.queue.is_empty(),
            s.expander_on,
            s.transform_on,
        )
    });
    let (transform_visible, config_visible, intercept_paste, expander_on, transform_on) =
        flags.unwrap_or((false, false, false, true, true));
    let our_fg = is_our_process_foreground();

    if transform_visible && !ctrl && !alt {
        if vk == VK_1.0 as u32 {
            send_cmd(WorkCmd::Transform(TransformKind::Title));
            return true;
        }
        if vk == VK_2.0 as u32 {
            send_cmd(WorkCmd::Transform(TransformKind::Upper));
            return true;
        }
        if vk == VK_3.0 as u32 {
            send_cmd(WorkCmd::Transform(TransformKind::Lower));
            return true;
        }
        if vk == VK_4.0 as u32 {
            send_cmd(WorkCmd::TransformCancel);
            return true;
        }
    }

    if config_visible && our_fg && ctrl && !alt {
        if vk == VK_S.0 as u32 {
            save_config_from_draft();
            return true;
        }
        if vk == VK_Q.0 as u32 {
            if let Some(s) = SHARED.get() {
                lock(s).config_visible = false;
                request_ui();
            }
            return true;
        }
    }

    if ctrl && alt && shift && vk == VK_H.0 as u32 {
        send_cmd(WorkCmd::ToggleHelp);
        return true;
    }

    if ctrl && alt && !shift {
        if vk == VK_T.0 as u32 {
            if transform_on {
                send_cmd(WorkCmd::BeginTransform);
                return true;
            }
            return false;
        }
        if vk == VK_E.0 as u32 {
            send_cmd(WorkCmd::OpenConfig);
            return true;
        }
        if vk == VK_R.0 as u32 {
            send_cmd(WorkCmd::ClearQueue);
            return true;
        }
        if vk == VK_P.0 as u32 {
            send_cmd(WorkCmd::TogglePause);
            return true;
        }
    }

    if ctrl && !alt && vk == VK_V.0 as u32 {
        if intercept_paste {
            match decide_paste_key(shift) {
                PasteKey::LetRealThrough => return false,
                PasteKey::Swallow => return true,
                PasteKey::SwallowAndSynth => {
                    send_cmd(WorkCmd::Paste { unformatted: shift });
                    return true;
                }
            }
        }
        return false;
    }

    if ctrl && !alt && (vk == VK_C.0 as u32 || vk == VK_X.0 as u32) {
        note_user_copy();
        return false;
    }

    if shift && !ctrl && !alt && vk == VK_DELETE.0 as u32 {
        if let Some(shared) = SHARED.get() {
            if let Some(mut s) = try_lock(shared) {
                s.cutting_until = Some(Instant::now() + Duration::from_millis(250));
            }
        }
        return false;
    }

    if vk == VK_BACK.0 as u32 {
        if let Ok(mut buf) = TYPE_BUF.lock() {
            buf.pop();
        }
        return false;
    }

    if vk == VK_RETURN.0 as u32 || vk == VK_TAB.0 as u32 || vk == VK_ESCAPE.0 as u32 {
        if let Ok(mut buf) = TYPE_BUF.lock() {
            buf.clear();
        }
        return false;
    }

    if ctrl && !alt {
        if let Ok(mut buf) = TYPE_BUF.lock() {
            buf.clear();
        }
        return false;
    }

    if expander_on {
        if let Some(ch) = vk_to_char_safe(vk, shift, ctrl && alt) {
            if let Ok(mut buf) = TYPE_BUF.try_lock() {
                buf.push(ch);
                if buf.len() > 64 {
                    let drain = buf.len() - 64;
                    buf.drain(..drain);
                }
                if let Some((chars, payload)) = match_expander(&buf) {
                    let keep = buf.chars().count().saturating_sub(chars);
                    *buf = buf.chars().take(keep).collect();
                    let backspaces = chars.saturating_sub(1);
                    send_cmd(WorkCmd::Expand { backspaces, payload });
                    return true;
                }
            }
        }
    }

    false
}

fn save_config_from_draft() {
    let Some(shared) = SHARED.get() else {
        return;
    };
    {
        let mut s = lock(shared);
        s.slots = s.draft_slots.clone();
        s.triggers = s.draft_triggers.clone();
        s.config_visible = false;
        s.editor_visible = false;
        s.persist();
        s.persist_expander_db();
    }
    request_ui();
}

fn match_expander(buf: &str) -> Option<(usize, ClipPayload)> {
    let mut best_len = 0usize;
    let mut best_sort: i64 = -2;

    if buf.ends_with("!data") {
        best_len = 5;
        best_sort = -1;
    }

    if let Some(shared) = SHARED.get() {
        let Some(s) = try_lock(shared) else {
            return if best_sort == -1 {
                Some((5, ClipPayload::from_unicode(today_br())))
            } else {
                None
            };
        };
        if !s.expander_on {
            return None;
        }
        let n = s.triggers.len().max(s.draft_triggers.len());
        for i in 0..n {
            let trig = if s.config_visible {
                s.draft_triggers.get(i).map(|t| t.as_str()).unwrap_or("")
            } else {
                s.trigger_for_slot(i)
            };
            if trig.is_empty() {
                continue;
            }
            if buf.ends_with(trig) && trig.chars().count() > best_len {
                best_len = trig.chars().count();
                best_sort = i as i64;
            }
        }
    }

    if best_sort == -1 {
        return Some((best_len, ClipPayload::from_unicode(today_br())));
    }
    if best_sort < 0 {
        return None;
    }
    if let Some(shared) = SHARED.get() {
        if let Some(s) = try_lock(shared) {
            if let Some(p) = s.draft_payloads.get(best_sort as usize) {
                if !p.is_empty() {
                    return Some((best_len, p.clone()));
                }
            }
        }
    }
    let payload = crate::store::get_payload(best_sort).unwrap_or_default();
    if payload.is_empty() {
        None
    } else {
        Some((best_len, payload))
    }
}

/// Converte tecla sem ToUnicode — ToUnicode no hook come acento/dead key (Não virava N~~ao).
fn vk_to_char_safe(vk: u32, shift: bool, altgr: bool) -> Option<char> {
    if altgr {
        // ABNT: AltGr+Q = @. Não mexe em dead keys.
        if vk == 0x51 {
            return Some('@');
        }
        return None;
    }
    if (0x41..=0x5A).contains(&vk) {
        let caps = unsafe { GetKeyState(VK_CAPITAL.0 as i32) as u16 & 1 != 0 };
        let upper = shift ^ caps;
        let base = (vk - 0x41) as u8;
        return Some(if upper {
            (b'A' + base) as char
        } else {
            (b'a' + base) as char
        });
    }
    if (0x30..=0x39).contains(&vk) {
        let i = (vk - 0x30) as usize;
        if !shift {
            return Some((b'0' + i as u8) as char);
        }
        // US/ABNT para 1 e 2: ! e @ — o resto não é necessário para os gatilhos padrão.
        return Some([')', '!', '@', '#', '$', '%', '^', '&', '*', '('][i]);
    }
    match vk {
        0xBD | 0x6D => Some(if shift { '_' } else { '-' }),
        0xBE => Some(if shift { '>' } else { '.' }),
        0x20 => Some(' '),
        _ => None,
    }
}

fn is_our_process_foreground() -> bool {
    unsafe {
        let fg = GetForegroundWindow();
        let mut pid = 0u32;
        GetWindowThreadProcessId(fg, Some(&mut pid));
        pid == GetCurrentProcessId()
    }
}

/// Abre de novo e mata este processo. Roda na thread do gancho, então
/// ainda funciona se a interface (egui) estiver congelada.
pub fn restart_process() {
    release_instance_mutex();
    if let Ok(exe) = std::env::current_exe() {
        let _ = std::process::Command::new(exe).spawn();
    }
    std::process::exit(0);
}

pub fn shutdown_hook_thread() {
    unsafe {
        if !TRAY_HWND.0.is_null() {
            windows::Win32::UI::WindowsAndMessaging::PostMessageW(
                Some(TRAY_HWND),
                WM_QUIT,
                WPARAM(0),
                LPARAM(0),
            )
            .ok();
        }
    }
}
