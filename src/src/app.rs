use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
use std::time::Duration;

use eframe::egui::{self, ViewportCommand};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use windows::Win32::Foundation::{HWND, LPARAM, TRUE};
use windows::Win32::Graphics::Dwm::{
    DwmSetWindowAttribute, DWMNCRP_DISABLED, DWMWA_CLOAK, DWMWA_NCRENDERING_POLICY,
    DWMWA_TRANSITIONS_FORCEDISABLED, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_DONOTROUND,
};
use windows::Win32::System::Threading::GetCurrentProcessId;
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetWindowLongPtrW, GetWindowRect, GetWindowTextW, GetWindowThreadProcessId,
    SetWindowLongPtrW, SetWindowPos, GWL_EXSTYLE, SWP_NOACTIVATE, SWP_NOSIZE, SWP_NOZORDER,
    WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
};

use crate::config;
use crate::hook::{self, set_ui_ctx};
use crate::state::{lock, Shared};
use crate::ui::{self, UiMemory};

static ROOT_HWND: AtomicIsize = AtomicIsize::new(0);
static ROOT_STYLED: AtomicBool = AtomicBool::new(false);

pub struct WbcApp {
    shared: Shared,
    mem: UiMemory,
}

impl WbcApp {
    pub fn new(cc: &eframe::CreationContext<'_>, shared: Shared) -> Self {
        set_ui_ctx(cc.egui_ctx.clone());
        let mut mem = UiMemory::default();
        ui::apply_theme(&cc.egui_ctx, &mut mem);
        Self { shared, mem }
    }
}

impl eframe::App for WbcApp {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        // Opaco = evita flash de lixo da GPU / janela transparente no 1º frame.
        [0x2B as f32 / 255.0, 0x2B as f32 / 255.0, 0x2B as f32 / 255.0, 1.0]
    }

    fn logic(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        set_ui_ctx(ctx.clone());
        ctx.send_viewport_cmd(ViewportCommand::RequestPaintWhileHidden);
        hide_root_host(frame);
        harden_child_viewports();

        let (exit, reload) = {
            let s = lock(&self.shared);
            (s.request_exit, s.request_reload)
        };

        if reload {
            restart_self();
            ctx.send_viewport_cmd(ViewportCommand::Close);
            return;
        }
        if exit {
            hook::shutdown_hook_thread();
            ctx.send_viewport_cmd(ViewportCommand::Close);
            return;
        }

        ctx.request_repaint_after(Duration::from_millis(120));
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        ui::apply_theme(&ctx, &mut self.mem);
        ui::draw_all(&ctx, &self.shared, &mut self.mem);
    }
}

fn restart_self() {
    hook::restart_process();
}

pub fn show_already_running() {
    crate::ui::run_already_running_dialog();
}

fn hwnd_from_frame(frame: &eframe::Frame) -> Option<HWND> {
    let handle = frame.window_handle().ok()?;
    match handle.as_raw() {
        RawWindowHandle::Win32(h) => Some(HWND(h.hwnd.get() as *mut c_void)),
        _ => None,
    }
}

/// Só a janela raiz 1×1. Não mexe em toast/transform/expansor.
fn hide_root_host(frame: &eframe::Frame) {
    if let Some(hwnd) = hwnd_from_frame(frame) {
        ROOT_HWND.store(hwnd.0 as isize, Ordering::Relaxed);
        cloak_and_park(hwnd);
        return;
    }
    let stored = ROOT_HWND.load(Ordering::Relaxed);
    if stored != 0 {
        cloak_and_park(HWND(stored as *mut c_void));
    }
}

fn cloak_and_park(hwnd: HWND) {
    if hwnd.0.is_null() {
        return;
    }
    unsafe {
        let no_anim = TRUE;
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_TRANSITIONS_FORCEDISABLED,
            &no_anim as *const _ as *const c_void,
            std::mem::size_of_val(&no_anim) as u32,
        );
        let on = TRUE;
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_CLOAK,
            &on as *const _ as *const c_void,
            std::mem::size_of_val(&on) as u32,
        );
        if !ROOT_STYLED.swap(true, Ordering::Relaxed) {
            let policy = DWMNCRP_DISABLED;
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWA_NCRENDERING_POLICY,
                &policy as *const _ as *const c_void,
                std::mem::size_of_val(&policy) as u32,
            );
            let corners = DWMWCP_DONOTROUND;
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWA_WINDOW_CORNER_PREFERENCE,
                &corners as *const _ as *const c_void,
                std::mem::size_of_val(&corners) as u32,
            );
        }
        let mut rc = windows::Win32::Foundation::RECT::default();
        let already_parked = GetWindowRect(hwnd, &mut rc).is_ok() && rc.left <= -30000;
        if !already_parked {
            let _ = SetWindowPos(
                hwnd,
                None,
                -32000,
                -32000,
                0,
                0,
                SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
            );
        }
    }
}

/// Desliga animação DWM nas janelas filhas e impede que fila/toast roubem o foco
/// (o flash da 1ª cópia vinha de uma viewport OpenGL nova + ativação).
fn harden_child_viewports() {
    let root = ROOT_HWND.load(Ordering::Relaxed);
    if root == 0 {
        return;
    }
    unsafe {
        let _ = EnumWindows(Some(harden_enum), LPARAM(root));
    }
}

unsafe extern "system" fn harden_enum(hwnd: HWND, lparam: LPARAM) -> windows::core::BOOL {
    let root = HWND(lparam.0 as *mut c_void);
    if hwnd == root || hwnd.0.is_null() {
        return TRUE;
    }
    let mut pid = 0u32;
    GetWindowThreadProcessId(hwnd, Some(&mut pid));
    if pid != GetCurrentProcessId() {
        return TRUE;
    }
    apply_child_style(hwnd);
    TRUE
}

fn apply_child_style(hwnd: HWND) {
    unsafe {
        let no_anim = TRUE;
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_TRANSITIONS_FORCEDISABLED,
            &no_anim as *const _ as *const c_void,
            std::mem::size_of_val(&no_anim) as u32,
        );

        let mut buf = [0u16; 64];
        let n = GetWindowTextW(hwnd, &mut buf);
        let title = String::from_utf16_lossy(&buf[..n as usize]);
        let no_focus = title == "Fila de Cópia" || title == "b-snip";
        if !no_focus {
            return;
        }
        let ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        let add = WS_EX_NOACTIVATE.0 as isize | WS_EX_TOOLWINDOW.0 as isize;
        if ex & add != add {
            SetWindowLongPtrW(hwnd, GWL_EXSTYLE, ex | add);
        }
    }
}

pub fn maybe_welcome(shared: &Shared, welcome_already: bool) {
    if !welcome_already {
        lock(shared).help_visible = true;
        config::mark_welcome_shown(&config::ini_path());
    }
}
