use eframe::egui::{
    self, Color32, CursorIcon, FontData, FontDefinitions, FontFamily, FontId, Frame, Key, Margin,
    Pos2, RichText, ScrollArea, Sense, Stroke, TextEdit, Ui, Vec2, ViewportBuilder, ViewportCommand,
    ViewportId, Visuals,
};
use std::sync::Arc;

use crate::config::{MAX_EXPANDER_ROWS, MIN_EXPANDER_ROWS};
use crate::hook::{send_cmd, WorkCmd};
use crate::payload::{
    check_expander_size, expander_limit_msg, format_mb, ClipPayload, SizeDecision, EXPANDER_ITEM_MAX,
};
use crate::state::{lock, DataDialog, Shared, TransformKind};

const BG: Color32 = Color32::from_rgb(0x2B, 0x2B, 0x2B);
const BORDER: Color32 = Color32::from_rgb(0x5A, 0x5A, 0x5A);
const TEXT: Color32 = Color32::WHITE;
const BTN: Color32 = Color32::from_rgb(0x3A, 0x3A, 0x3A);
const BTN_HOVER: Color32 = Color32::from_rgb(0x4A, 0x4A, 0x4A);
const ACCENT: Color32 = Color32::from_rgb(0x3D, 0x6E, 0xA8);

pub struct UiMemory {
    pub fonts_set: bool,
    pub queue_place: bool,
    pub transform_place: bool,
    pub config_place: bool,
    pub help_place: bool,
    pub last_queue_len: usize,
    pub last_transform: bool,
    pub last_config: bool,
    pub last_help: bool,
    pub last_toast: bool,
    pub last_editor: bool,
    pub toast_place: bool,
    pub last_queue_pos: Option<(i32, i32)>,
    pub editor_place: bool,
    pub editor_tex: Option<egui::TextureHandle>,
    pub editor_tex_sig: usize,
    /// Origem da janela + cursor ao começar a arrastar a fila (arraste manual, sem SC_MOVE).
    pub queue_drag: Option<(Pos2, i32, i32)>,
}

impl Default for UiMemory {
    fn default() -> Self {
        Self {
            fonts_set: false,
            queue_place: true,
            transform_place: true,
            config_place: true,
            help_place: true,
            last_queue_len: 0,
            last_transform: false,
            last_config: false,
            last_help: false,
            last_toast: false,
            last_editor: false,
            toast_place: true,
            last_queue_pos: None,
            queue_drag: None,
            editor_place: true,
            editor_tex: None,
            editor_tex_sig: 0,
        }
    }
}

pub fn apply_theme(ctx: &egui::Context, mem: &mut UiMemory) {
    if !mem.fonts_set {
        load_segoe(ctx);
        mem.fonts_set = true;
    }

    let mut visuals = Visuals::dark();
    visuals.panel_fill = BG;
    visuals.window_fill = BG;
    visuals.extreme_bg_color = Color32::from_rgb(0x22, 0x22, 0x22);
    visuals.override_text_color = Some(TEXT);
    visuals.widgets.inactive.bg_fill = BTN;
    visuals.widgets.inactive.weak_bg_fill = BTN;
    visuals.widgets.hovered.bg_fill = BTN_HOVER;
    visuals.widgets.hovered.weak_bg_fill = BTN_HOVER;
    visuals.widgets.active.bg_fill = ACCENT;
    visuals.widgets.active.weak_bg_fill = ACCENT;
    visuals.selection.bg_fill = ACCENT;
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT);
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, TEXT);
    visuals.widgets.active.fg_stroke = Stroke::new(1.0, TEXT);
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, TEXT);
    ctx.set_visuals(visuals);
}

fn load_segoe(ctx: &egui::Context) {
    let path = r"C:\Windows\Fonts\segoeui.ttf";
    let Ok(bytes) = std::fs::read(path) else {
        return;
    };
    let mut fonts = FontDefinitions::default();
    fonts
        .font_data
        .insert("segoe".into(), Arc::new(FontData::from_owned(bytes)));
    if let Some(family) = fonts.families.get_mut(&FontFamily::Proportional) {
        family.insert(0, "segoe".into());
    }
    if let Some(family) = fonts.families.get_mut(&FontFamily::Monospace) {
        family.insert(0, "segoe".into());
    }
    ctx.set_fonts(fonts);
}

fn dark_frame() -> Frame {
    Frame::new()
        .fill(BG)
        .stroke(Stroke::new(1.0, BORDER))
        .inner_margin(Margin::same(10))
}

fn drag_title(ui: &mut Ui, title: &str) {
    let (rect, resp) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), 28.0),
        Sense::drag(),
    );
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        title,
        FontId::proportional(16.0),
        TEXT,
    );
    if resp.drag_started() {
        ui.ctx().send_viewport_cmd(ViewportCommand::StartDrag);
    }
}

/// Filho com largura fixa: o pai `top_down(Center)` consegue centralizar de verdade.
fn centered_box(ui: &mut Ui, size: Vec2, add: impl FnOnce(&mut Ui)) {
    ui.allocate_ui_with_layout(size, egui::Layout::left_to_right(egui::Align::Center), add);
}

fn remember_queue_pos(
    ctx: &egui::Context,
    last: &mut Option<(i32, i32)>,
    shared: &Shared,
    write_disk: bool,
) {
    let Some(rect) = ctx.input(|i| i.viewport().outer_rect) else {
        return;
    };
    let pos = (rect.min.x.round() as i32, rect.min.y.round() as i32);
    if last.map(|old| old != pos).unwrap_or(true) {
        let mut s = lock(shared);
        s.window_pos.queue = Some((pos.0 as f32, pos.1 as f32));
        if write_disk {
            s.persist();
        }
    }
    *last = Some(pos);
}

fn screen_size() -> (f32, f32) {
    use windows::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN};
    unsafe {
        (
            GetSystemMetrics(SM_CXSCREEN) as f32,
            GetSystemMetrics(SM_CYSCREEN) as f32,
        )
    }
}

fn mouse_work_area() -> Option<(f32, f32, f32, f32)> {
    use windows::Win32::Foundation::POINT;
    use windows::Win32::Graphics::Gdi::{
        GetMonitorInfoW, MonitorFromPoint, MONITORINFO, MONITOR_DEFAULTTONEAREST,
    };
    use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;

    unsafe {
        let mut pt = POINT::default();
        GetCursorPos(&mut pt).ok()?;
        let mon = MonitorFromPoint(pt, MONITOR_DEFAULTTONEAREST);
        let mut info: MONITORINFO = std::mem::zeroed();
        info.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
        if !GetMonitorInfoW(mon, &mut info).as_bool() {
            return None;
        }
        let r = info.rcWork;
        Some((r.left as f32, r.top as f32, r.right as f32, r.bottom as f32))
    }
}

fn work_area_at(x: i32, y: i32) -> Option<(f32, f32, f32, f32)> {
    use windows::Win32::Foundation::POINT;
    use windows::Win32::Graphics::Gdi::{
        GetMonitorInfoW, MonitorFromPoint, MONITORINFO, MONITOR_DEFAULTTONEAREST,
    };

    unsafe {
        let pt = POINT { x, y };
        let mon = MonitorFromPoint(pt, MONITOR_DEFAULTTONEAREST);
        let mut info: MONITORINFO = std::mem::zeroed();
        info.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
        if !GetMonitorInfoW(mon, &mut info).as_bool() {
            return None;
        }
        let r = info.rcWork;
        Some((r.left as f32, r.top as f32, r.right as f32, r.bottom as f32))
    }
}

fn clamp_to_work(pos: Pos2, win_w: f32, win_h: f32) -> Pos2 {
    let work = work_area_at(pos.x.round() as i32, pos.y.round() as i32).or_else(mouse_work_area);
    let (left, top, right, bottom) = work.unwrap_or_else(|| {
        let (sw, sh) = screen_size();
        (0.0, 0.0, sw, sh)
    });
    let x = pos.x.clamp(left, (right - win_w).max(left));
    let y = pos.y.clamp(top, (bottom - win_h).max(top));
    Pos2::new(x, y)
}

/// Canto inferior direito da área útil (sem barra de tarefas, sem vazar de monitor).
fn bottom_right_on_mouse_monitor(win_w: f32, win_h: f32, margin: f32) -> Pos2 {
    let (_left, _top, right, bottom) = mouse_work_area().unwrap_or_else(|| {
        let (sw, sh) = screen_size();
        (0.0, 0.0, sw, sh)
    });
    clamp_to_work(
        Pos2::new(right - win_w - margin, bottom - win_h - margin),
        win_w,
        win_h,
    )
}

/// Centro da área útil do monitor onde o mouse está agora.
fn center_on_mouse_monitor(win_w: f32, win_h: f32) -> Pos2 {
    if let Some((left, top, right, bottom)) = mouse_work_area() {
        clamp_to_work(
            Pos2::new(
                left + (right - left) / 2.0 - win_w / 2.0,
                top + (bottom - top) / 2.0 - win_h / 2.0,
            ),
            win_w,
            win_h,
        )
    } else {
        let (sw, sh) = screen_size();
        Pos2::new((sw - win_w) / 2.0, (sh - win_h) / 2.0)
    }
}

pub fn draw_all(ctx: &egui::Context, shared: &Shared, mem: &mut UiMemory) {
    let (queue_len, transform, config_on, help_on, copy_on, toast, editor_on, data_dlg) = {
        let mut s = lock(shared);
        if s.toast.as_ref().is_some_and(|(_, until)| std::time::Instant::now() >= *until) {
            s.toast = None;
        }
        (
            s.queue.len(),
            s.transform_visible,
            s.config_visible,
            s.help_visible,
            s.copy_queue_on,
            s.toast_text().map(|t| t.to_string()),
            s.editor_visible,
            s.data_dialog.clone(),
        )
    };

    if queue_len > 0 && mem.last_queue_len == 0 {
        mem.queue_place = true;
    }
    if transform && !mem.last_transform {
        mem.transform_place = true;
    }
    if config_on && !mem.last_config {
        mem.config_place = true;
    }
    if help_on && !mem.last_help {
        mem.help_place = true;
    }
    if editor_on && !mem.last_editor {
        mem.editor_place = true;
    }
    let toast_on = toast.is_some();
    if toast_on && !mem.last_toast {
        mem.toast_place = true;
    }
    mem.last_queue_len = queue_len;
    mem.last_transform = transform;
    mem.last_config = config_on;
    mem.last_help = help_on;
    mem.last_toast = toast_on;
    mem.last_editor = editor_on;

    draw_queue(ctx, shared, mem, copy_on && queue_len > 0);
    if let Some(msg) = toast {
        draw_toast(ctx, mem, &msg);
    }
    if transform {
        draw_transform(ctx, mem);
    }
    if config_on {
        draw_config(ctx, shared, mem);
    }
    if help_on {
        draw_help(ctx, shared, mem);
    }
    if editor_on {
        draw_editor(ctx, shared, mem);
    }
    if data_dlg.is_some() {
        draw_data_dialog(ctx, shared);
    }
}

fn cursor_pos() -> Option<(i32, i32)> {
    use windows::Win32::Foundation::POINT;
    use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;
    unsafe {
        let mut pt = POINT::default();
        GetCursorPos(&mut pt).ok()?;
        Some((pt.x, pt.y))
    }
}

fn set_move_cursor() {
    use windows::Win32::UI::WindowsAndMessaging::{LoadCursorW, SetCursor, IDC_SIZEALL};
    unsafe {
        if let Ok(cur) = LoadCursorW(None, IDC_SIZEALL) {
            let _ = SetCursor(Some(cur));
        }
    }
}

fn draw_queue(ctx: &egui::Context, shared: &Shared, mem: &mut UiMemory, visible: bool) {
    const WIN_W: f32 = 176.0;
    const WIN_H: f32 = 132.0;
    let saved = lock(shared).window_pos.queue;
    let pos = saved
        .map(|(x, y)| clamp_to_work(Pos2::new(x, y), WIN_W, WIN_H))
        .unwrap_or_else(|| bottom_right_on_mouse_monitor(WIN_W, WIN_H, 16.0));

    let mut builder = ViewportBuilder::default()
        .with_title("Fila de Cópia")
        .with_decorations(false)
        .with_always_on_top()
        .with_resizable(false)
        .with_taskbar(false)
        .with_transparent(false)
        .with_active(false)
        .with_visible(visible)
        .with_inner_size([WIN_W, WIN_H]);
    // Mantém o HWND criado (invisível) desde o 1º frame, para a 1ª cópia não abrir
    // uma janela OpenGL nova — isso piscava a tela.
    if !visible {
        builder = builder.with_position(pos);
    } else if mem.queue_drag.is_none() && (mem.queue_place || mem.last_queue_pos.is_none()) {
        builder = builder.with_position(pos);
        mem.queue_place = false;
        mem.last_queue_pos = Some((pos.x.round() as i32, pos.y.round() as i32));
    }

    ctx.show_viewport_immediate(
        ViewportId::from_hash_of("queue"),
        builder,
        |ctx, _class| {
            if !visible {
                return;
            }
            egui::CentralPanel::default().frame(dark_frame()).show(ctx, |ui| {
                let (cur, total, paused) = {
                    let s = lock(shared);
                    let (c, t) = s.cycle_label();
                    (c, t, s.queue_paused)
                };
                ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                    ui.add(
                        egui::Label::new(
                            RichText::new("Fila de Cópia").strong().size(15.0).color(TEXT),
                        )
                        .selectable(false),
                    );
                    ui.add_space(4.0);
                    ui.add(
                        egui::Label::new(
                            RichText::new(format!("{cur}/{total}"))
                                .strong()
                                .size(22.0)
                                .color(TEXT),
                        )
                        .selectable(false),
                    );
                    ui.add(
                        egui::Label::new(
                            RichText::new(if paused {
                                "pausado"
                            } else {
                                "próximo a colar"
                            })
                            .size(12.0)
                            .color(TEXT),
                        )
                        .selectable(false),
                    );
                    ui.add(
                        egui::Label::new(
                            RichText::new("Ctrl+Alt+R limpa").size(12.0).color(TEXT),
                        )
                        .selectable(false),
                    );
                });
                let grab = ui.interact(
                    ui.max_rect(),
                    ui.scope_id().with("queue-grab"),
                    Sense::click_and_drag(),
                );
                if grab.hovered() || grab.dragged() {
                    ui.ctx().set_cursor_icon(CursorIcon::Move);
                    set_move_cursor();
                }
                if grab.drag_started() {
                    if let (Some(rect), Some((cx, cy))) = (
                        ui.ctx().input(|i| i.viewport().outer_rect),
                        cursor_pos(),
                    ) {
                        mem.queue_drag = Some((rect.min, cx, cy));
                    }
                }
                if grab.dragged() {
                    if let (Some((win0, cx0, cy0)), Some((cx, cy))) = (mem.queue_drag, cursor_pos())
                    {
                        let next = clamp_to_work(
                            Pos2::new(win0.x + (cx - cx0) as f32, win0.y + (cy - cy0) as f32),
                            WIN_W,
                            WIN_H,
                        );
                        ui.ctx()
                            .send_viewport_cmd(ViewportCommand::OuterPosition(next));
                    }
                }
                if grab.drag_stopped() {
                    mem.queue_drag = None;
                    remember_queue_pos(&ui.ctx().clone(), &mut mem.last_queue_pos, shared, true);
                }
            });
        },
    );
}

fn draw_transform(ctx: &egui::Context, mem: &mut UiMemory) {
    let mut builder = ViewportBuilder::default()
        .with_title("Transformação")
        .with_decorations(false)
        .with_always_on_top()
        .with_resizable(false)
        .with_taskbar(false)
        .with_transparent(false)
        .with_inner_size([228.0, 210.0]);
    if mem.transform_place {
        builder = builder
            .with_position(center_on_mouse_monitor(228.0, 210.0))
            .with_active(true);
        mem.transform_place = false;
    }

    ctx.show_viewport_immediate(
        ViewportId::from_hash_of("transform"),
        builder,
        |ctx, _class| {
            if ctx.input(|i| i.viewport().close_requested()) {
                send_cmd(WorkCmd::TransformCancel);
            }
            egui::CentralPanel::default().frame(dark_frame()).show(ctx, |ui| {
                drag_title(ui, "Transformação:");
                ui.add_space(6.0);
                let w = ui.available_width();
                if big_btn(ui, w, "1 - Nomes Próprios") {
                    send_cmd(WorkCmd::Transform(TransformKind::Title));
                }
                if big_btn(ui, w, "2 - MAIÚSCULA") {
                    send_cmd(WorkCmd::Transform(TransformKind::Upper));
                }
                if big_btn(ui, w, "3 - minúscula") {
                    send_cmd(WorkCmd::Transform(TransformKind::Lower));
                }
                if big_btn(ui, w, "4 - Cancelar") {
                    send_cmd(WorkCmd::TransformCancel);
                }
            });
        },
    );
}

fn big_btn(ui: &mut Ui, w: f32, text: &str) -> bool {
    ui.add_sized(
        [w, 32.0],
        egui::Button::new(RichText::new(text).strong().color(TEXT)),
    )
    .clicked()
}

fn draw_config(ctx: &egui::Context, shared: &Shared, mem: &mut UiMemory) {
    let dialog_up = lock(shared).data_dialog.is_some();
    let mut builder = ViewportBuilder::default()
        .with_title("Expansor")
        .with_decorations(false)
        .with_resizable(false)
        .with_taskbar(false)
        .with_transparent(false)
        .with_inner_size([540.0, 560.0]);
    if !dialog_up {
        builder = builder.with_always_on_top();
    }
    if mem.config_place {
        builder = builder
            .with_position(center_on_mouse_monitor(540.0, 560.0))
            .with_active(true);
        mem.config_place = false;
    }

    ctx.show_viewport_immediate(
        ViewportId::from_hash_of("config"),
        builder,
        |ctx, _class| {
            if ctx.input(|i| i.viewport().close_requested())
                || ctx.input(|i| i.key_pressed(Key::Escape))
            {
                lock(shared).config_visible = false;
            }
            let mut save_now = ctx.input(|i| i.modifiers.ctrl && i.key_pressed(Key::S));
            if ctx.input(|i| i.modifiers.ctrl && i.key_pressed(Key::Q)) {
                lock(shared).config_visible = false;
            }

            egui::CentralPanel::default().frame(dark_frame()).show(ctx, |ui| {
                drag_title(ui, "Expansor de Texto");
                const COL_W: f32 = 500.0;
                const GAP: f32 = 16.0;
                let remaining = ui.available_size();
                ui.allocate_ui_with_layout(
                    remaining,
                    egui::Layout::top_down(egui::Align::Center),
                    |ui| {
                        ui.spacing_mut().item_spacing.y = 6.0;
                        ui.add_space(4.0);

                        let btn_w = (COL_W - GAP) / 2.0;
                        centered_box(ui, Vec2::new(COL_W, 36.0), |ui| {
                            ui.spacing_mut().item_spacing.x = GAP;
                            if ui
                                .add_sized(
                                    [btn_w, 32.0],
                                    egui::Button::new(RichText::new("Salvar").strong().color(TEXT)),
                                )
                                .clicked()
                            {
                                save_now = true;
                            }
                            if ui
                                .add_sized(
                                    [btn_w, 32.0],
                                    egui::Button::new(
                                        RichText::new("Cancelar").strong().color(TEXT),
                                    ),
                                )
                                .clicked()
                            {
                                lock(shared).config_visible = false;
                            }
                        });

                        ui.label(
                            RichText::new(
                                "Ctrl+S = Salvar, Ctrl+Q = Cancelar\nTAB / SHIFT+TAB para navegar nos menus",
                            )
                            .size(13.0)
                            .color(TEXT),
                        );
                        ui.label(
                            RichText::new("Comando fixo: !data → data atual (DD/MM/AAAA)")
                                .size(13.0)
                                .color(TEXT),
                        );

                        let (mut draft_trigs, mut draft_slots, mut draft_payloads, just_opened) = {
                            let mut s = lock(shared);
                            let opened = s.config_just_opened;
                            s.config_just_opened = false;
                            if opened || s.draft_payloads.is_empty() {
                                s.load_drafts_from_db();
                            }
                            (
                                s.draft_triggers.clone(),
                                s.draft_slots.clone(),
                                s.draft_payloads.clone(),
                                opened,
                            )
                        };
                        if draft_trigs.len() != draft_slots.len() {
                            let n = draft_trigs.len().max(draft_slots.len());
                            draft_trigs.resize(n, String::new());
                            draft_slots.resize(n, String::new());
                        }
                        draft_payloads.resize(draft_trigs.len(), ClipPayload::default());
                        if draft_trigs.len() > MAX_EXPANDER_ROWS {
                            draft_trigs.truncate(MAX_EXPANDER_ROWS);
                            draft_slots.truncate(MAX_EXPANDER_ROWS);
                            draft_payloads.truncate(MAX_EXPANDER_ROWS);
                        }
                        let mut remove_at = None;
                        let mut add_row = false;
                        let mut open_editor: Option<usize> = None;
                        ui.group(|ui| {
                            ui.set_width(COL_W);
                            ui.spacing_mut().item_spacing.y = 6.0;
                            ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                                ui.label(
                                    RichText::new("Gatilhos (até 100)").strong().color(TEXT),
                                );
                                let footer_h = 32.0 + 8.0 + 20.0 + 36.0;
                                let scroll_h =
                                    (ui.available_height() - footer_h).clamp(80.0, 420.0);
                                ScrollArea::vertical()
                                    .max_height(scroll_h)
                                    .auto_shrink([false, false])
                                    .show(ui, |ui| {
                                        let row_w = ui.available_width();
                                        ui.spacing_mut().item_spacing.x = 8.0;
                                        const SIDE: f32 = 8.0;
                                        const NUM_W: f32 = 42.0;
                                        const TRIG_W: f32 = 128.0;
                                        const MINUS_W: f32 = 28.0;
                                        const EDIT_W: f32 = 28.0;
                                        const ROW_H: f32 = 24.0;
                                        for i in 0..draft_trigs.len() {
                                            ui.horizontal(|ui| {
                                                ui.set_width(row_w);
                                                let gap = ui.spacing().item_spacing.x;
                                                let info_w = (row_w
                                                    - SIDE * 2.0
                                                    - NUM_W
                                                    - TRIG_W
                                                    - EDIT_W
                                                    - MINUS_W
                                                    - gap * 4.0)
                                                    .max(80.0);
                                                ui.add_space(SIDE);
                                                ui.add_sized(
                                                    [NUM_W, ROW_H],
                                                    egui::Label::new(
                                                        RichText::new(format!("{}:", i + 1))
                                                            .color(TEXT),
                                                    ),
                                                );
                                                let trig = ui.add_sized(
                                                    [TRIG_W, ROW_H],
                                                    TextEdit::singleline(&mut draft_trigs[i])
                                                        .id_salt(("trig", i))
                                                        .desired_width(TRIG_W)
                                                        .char_limit(20)
                                                        .font(FontId::proportional(13.0)),
                                                );
                                                ui.add_sized(
                                                    [info_w, ROW_H],
                                                    TextEdit::singleline(&mut draft_slots[i])
                                                        .id_salt(("name", i))
                                                        .desired_width(info_w)
                                                        .char_limit(80)
                                                        .hint_text("nome")
                                                        .font(FontId::proportional(13.0)),
                                                );
                                                if ui
                                                    .add(
                                                        egui::Button::new(
                                                            RichText::new("✎").color(TEXT),
                                                        )
                                                        .min_size(Vec2::new(EDIT_W, ROW_H)),
                                                    )
                                                    .on_hover_text("Editar conteúdo (colar rico)")
                                                    .clicked()
                                                {
                                                    open_editor = Some(i);
                                                }
                                                let can_remove =
                                                    draft_trigs.len() > MIN_EXPANDER_ROWS;
                                                if ui
                                                    .add_enabled(
                                                        can_remove,
                                                        egui::Button::new(
                                                            RichText::new("−").color(TEXT),
                                                        )
                                                        .min_size(Vec2::new(MINUS_W, ROW_H)),
                                                    )
                                                    .on_hover_text("Remover esta linha")
                                                    .clicked()
                                                {
                                                    remove_at = Some(i);
                                                }
                                                if just_opened && i == 0 {
                                                    trig.request_focus();
                                                }
                                            });
                                        }
                                    });
                                let count = draft_trigs.len();
                                let can_add = count < MAX_EXPANDER_ROWS;
                                let (row, _) = ui.allocate_exact_size(
                                    Vec2::new(COL_W - 12.0, 32.0),
                                    Sense::hover(),
                                );
                                let btn_rect = egui::Rect::from_center_size(
                                    row.center(),
                                    Vec2::new(200.0, 28.0),
                                );
                                if ui
                                    .put(
                                        btn_rect,
                                        egui::Button::new(
                                            RichText::new("+ Adicionar linha")
                                                .strong()
                                                .color(TEXT),
                                        )
                                        .min_size(Vec2::new(200.0, 28.0)),
                                    )
                                    .clicked()
                                    && can_add
                                {
                                    add_row = true;
                                }
                                let count_rect = egui::Rect::from_min_size(
                                    Pos2::new(btn_rect.max.x + 10.0, row.center().y - 10.0),
                                    Vec2::new(56.0, 20.0),
                                );
                                ui.put(
                                    count_rect,
                                    egui::Label::new(
                                        RichText::new(format!("{count}/{MAX_EXPANDER_ROWS}"))
                                            .size(12.0)
                                            .color(TEXT),
                                    ),
                                );
                                ui.add_space(8.0);
                                ui.label(
                                    RichText::new("Backup e restauração")
                                        .strong()
                                        .size(13.0)
                                        .color(TEXT),
                                );
                                centered_box(ui, Vec2::new(COL_W, 36.0), |ui| {
                                    ui.spacing_mut().item_spacing.x = GAP;
                                    if ui
                                        .add_sized(
                                            [btn_w, 32.0],
                                            egui::Button::new(
                                                RichText::new("Exportar backup…")
                                                    .strong()
                                                    .color(TEXT),
                                            ),
                                        )
                                        .clicked()
                                    {
                                        if let Some(path) = crate::dialog::save_backup() {
                                            match crate::backup::export_zip(&path) {
                                                Ok(()) => {
                                                    lock(shared).show_toast("Backup exportado")
                                                }
                                                Err(e) => lock(shared).show_toast(e),
                                            }
                                        }
                                    }
                                    if ui
                                        .add_sized(
                                            [btn_w, 32.0],
                                            egui::Button::new(
                                                RichText::new("Importar backup…")
                                                    .strong()
                                                    .color(TEXT),
                                            ),
                                        )
                                        .clicked()
                                    {
                                        if let Some(path) = crate::dialog::open_backup() {
                                            lock(shared).data_dialog =
                                                Some(DataDialog::ConfirmImport { path });
                                        }
                                    }
                                });
                            });
                        });
                        if let Some(i) = remove_at {
                            if i < draft_trigs.len() && draft_trigs.len() > MIN_EXPANDER_ROWS {
                                draft_trigs.remove(i);
                                draft_slots.remove(i);
                                if i < draft_payloads.len() {
                                    draft_payloads.remove(i);
                                }
                            }
                        }
                        if add_row && draft_trigs.len() < MAX_EXPANDER_ROWS {
                            draft_trigs.push(String::new());
                            draft_slots.push(String::new());
                            draft_payloads.push(ClipPayload::default());
                        }
                        {
                            let mut s = lock(shared);
                            s.draft_triggers = draft_trigs;
                            s.draft_slots = draft_slots;
                            if !s.editor_visible {
                                s.draft_payloads = draft_payloads;
                            }
                            if let Some(i) = open_editor {
                                s.editor_row = i;
                                s.editor_visible = true;
                                let p = s.draft_payloads.get(i).cloned().unwrap_or_default();
                                s.editor_buf = crate::text::visible_text(&p.unicode);
                                s.editor_orig_visible = s.editor_buf.clone();
                            }
                        }
                        if save_now {
                            persist_config(shared);
                        }
                    },
                );
            });
        },
    );
}

fn persist_config(shared: &Shared) {
    let mut s = lock(shared);
    s.slots = s.draft_slots.clone();
    s.triggers = s.draft_triggers.clone();
    s.config_visible = false;
    s.editor_visible = false;
    s.persist();
    s.persist_expander_db();
}

fn draw_toast(ctx: &egui::Context, mem: &mut UiMemory, msg: &str) {
    let lines = msg.lines().count().max(1);
    let win_w = if lines >= 3 { 400.0 } else { 320.0 };
    let win_h = (72.0 + lines as f32 * 26.0).clamp(120.0, 200.0);
    let mut builder = ViewportBuilder::default()
        .with_title("b-snip")
        .with_decorations(false)
        .with_always_on_top()
        .with_resizable(false)
        .with_taskbar(false)
        .with_transparent(false)
        .with_active(false)
        .with_inner_size([win_w, win_h]);
    if mem.toast_place {
        builder = builder.with_position(bottom_right_on_mouse_monitor(win_w, win_h, 16.0));
        mem.toast_place = false;
    }
    ctx.show_viewport_immediate(
        ViewportId::from_hash_of("toast"),
        builder,
        |ctx, _class| {
            egui::CentralPanel::default().frame(dark_frame()).show(ctx, |ui| {
                ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                    ui.add_space(8.0);
                    ui.add(
                        egui::Label::new(
                            RichText::new("b-snip").strong().size(16.0).color(TEXT),
                        )
                        .selectable(false),
                    );
                    ui.add_space(10.0);
                    ui.add(
                        egui::Label::new(
                            RichText::new(msg)
                                .strong()
                                .size(if lines >= 3 { 14.0 } else { 22.0 })
                                .color(TEXT),
                        )
                        .selectable(false)
                        .wrap(),
                    );
                });
            });
        },
    );
}

fn draw_help(ctx: &egui::Context, shared: &Shared, mem: &mut UiMemory) {
    let mut builder = ViewportBuilder::default()
        .with_title("Central de Ajuda")
        .with_decorations(false)
        .with_always_on_top()
        .with_resizable(false)
        .with_taskbar(false)
        .with_transparent(false)
        .with_inner_size([560.0, 460.0]);
    if mem.help_place {
        builder = builder
            .with_position(center_on_mouse_monitor(560.0, 460.0))
            .with_active(true);
        mem.help_place = false;
    }

    ctx.show_viewport_immediate(
        ViewportId::from_hash_of("help"),
        builder,
        |ctx, _class| {
            if ctx.input(|i| i.viewport().close_requested()) {
                lock(shared).help_visible = false;
            }
            egui::CentralPanel::default().frame(dark_frame()).show(ctx, |ui| {
                ui.horizontal(|ui| {
                    drag_title(ui, "Central de Ajuda");
                });
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    tab_btn(ui, shared, 1, "Fila");
                    tab_btn(ui, shared, 2, "Transform");
                    tab_btn(ui, shared, 3, "Expansor");
                    tab_btn(ui, shared, 4, "Sistema");
                    tab_btn(ui, shared, 5, "Dados");
                });
                ui.add_space(8.0);
                let tab = lock(shared).help_tab;
                ui.group(|ui| {
                    ui.set_min_size(Vec2::new(ui.available_width(), 260.0));
                    match tab {
                        2 => {
                            ui.label(RichText::new("Transformação de Texto").strong().size(16.0));
                            ui.add_space(8.0);
                            ui.label("Use Ctrl+Alt+T sobre um texto selecionado:\n\n1. Nomes Próprios: Ex: 'texto teste' -> 'Texto Teste'.\n2. MAIÚSCULA: Ex: 'texto' -> 'TEXTO'.\n3. minúscula: Ex: 'TEXTO' -> 'texto'.\n\nO texto fica no lugar — não reseleciona. Formato rico (Word/HTML) mantém tags, fonte e imagens; só o texto visível muda.\nA janela abre no monitor onde o mouse está.");
                        }
                        3 => {
                            ui.label(RichText::new("Expansor de Gatilhos").strong().size(16.0));
                            ui.add_space(8.0);
                            ui.label("Atalhos para textos prontos, inclusive ricos (HTML/RTF/imagem). Começa com 3 linhas; até 100.\n\n1. Ctrl+Alt+E ou o menu da bandeja.\n2. Gatilho (ex: !mail) e um nome para ler a lista.\n3. ✎ abre a janela grande (redimensionável). O botão Colar captura o pacote. O texto aparece inteiro, com rolagem; dá para apagar e inserir. Salvar conteúdo grava na hora (o gatilho já usa o novo).\n4. Backup e restauração abaixo da lista. Importar pede confirmação na frente, reinicia e volta no expansor.\n5. Ctrl+S ou Salvar; Ctrl+Q ou Cancelar.\n\nFixo: !data → data de hoje (DD/MM/AAAA). O que o expansor cola não entra na fila de cópia contínua.\nTeto por item: 8 MB. Texto e imagens ficam no SQLite no disco; reiniciar o PC não apaga.");
                        }
                        4 => {
                            ui.label(RichText::new("Comandos de Sistema").strong().size(16.0));
                            ui.add_space(8.0);
                            ui.label("• Clique esquerdo no ícone: se a fila tem itens, pausa/retoma (não apaga). Se a fila está vazia, liga/desliga a cópia contínua.\n• Clique direito: menu. Desligar a cópia contínua pelo menu apaga a fila e o clipboard do Windows.\n• Pausa (menu ou Ctrl+Alt+P): a fila para, os itens ficam, e o Windows copia/cola normal. Despausar só na mão — copiar texto de novo não despausa.\n• Copiar arquivo, pasta ou print puro (sem texto) com a fila cheia: pausa sozinha. Texto com imagem entra na fila. Despausar e Ctrl+V cola a fila, mesmo com o arquivo ainda no clipboard.\n• Loop e “Limpar ao colar o último” não ligam juntos.\n• Todo ligar/desligar mostra um aviso. Som só na limpeza (Ctrl+Alt+R ou ao colar o último): zera a fila e o clipboard do Windows, para o Ctrl+V não colar nada.\n• Fila: até 100 itens, 2 MB cada, 32 MB no total. Aviso aos 80% e recusa acima do teto.\n\n--- Créditos ---\nWBC");
                        }
                        5 => {
                            ui.label(RichText::new("Onde os dados ficam").strong().size(16.0));
                            ui.add_space(8.0);
                            let portable = crate::paths::is_portable();
                            ui.label(format!(
                                "Pasta atual:\n{}",
                                crate::paths::data_dir().display()
                            ));
                            ui.add_space(8.0);
                            let mut on = portable;
                            if ui
                                .checkbox(
                                    &mut on,
                                    "Gravar dados ao lado do programa (modo portátil)",
                                )
                                .changed()
                            {
                                if on != portable {
                                    lock(shared).data_dialog =
                                        Some(DataDialog::Switch { to_portable: on });
                                }
                            }
                            ui.add_space(6.0);
                            ui.label("Desligado (padrão): pasta deste usuário — cada login do Windows tem o seu.\nLigado: ao lado do EXE (pendrive). Cria ou apaga portable.txt. Sem permissão (ex.: AD / Program Files), o interruptor não muda e avisa.\nAo trocar, dá para copiar os dados e é recomendado um backup.");
                        }
                        _ => {
                            ui.label(RichText::new("Como funciona a Fila?").strong().size(16.0));
                            ui.add_space(8.0);
                            ui.label("O programa guarda o que o Windows copiou (texto e formato rico), até 100 itens / 2 MB cada / 32 MB no total. Arquivo, pasta e print puro (sem texto) não entram: se a fila já tem itens, ela pausa sozinha. Texto que também traz uma imagem entra na fila. Despausar e Ctrl+V volta a colar a fila, mesmo com o arquivo ainda no clipboard do Windows.\n\nCtrl+C de novo no mesmo texto coloca ele de novo na fila (ex.: o mesmo telefone em dois campos). Um Ctrl+C = um item.\n\nO balão 1/5 arrasta (seta de quatro pontas). No teto, avisa e nada novo entra.\n\nPausa (Ctrl+Alt+P): congela sem apagar. Despausar só pelo menu/atalho/clique esquerdo.\n\nLoop ligado: Ctrl+V cicla. Loop desligado: cola do mais antigo até acabar. “Limpar ao colar o último” só com o Loop desligado: ao colar o último item, limpa a fila e o clipboard do Windows (aviso + som, como Ctrl+Alt+R). Ligar um desliga o outro.\nCtrl+Shift+V cola só o texto, sem estilo.");
                        }
                    }
                });
                ui.add_space(10.0);
                ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                    if ui
                        .add_sized(
                            [140.0, 36.0],
                            egui::Button::new(RichText::new("Entendido").strong().color(TEXT)),
                        )
                        .clicked()
                    {
                        lock(shared).help_visible = false;
                    }
                });
            });
        },
    );
}

fn apply_editor_capture(shared: &Shared, payload: &mut ClipPayload) -> bool {
    if let Some(cap) = crate::clipboard::capture() {
        match check_expander_size(cap.byte_size()) {
            SizeDecision::ItemTooBig => {
                lock(shared).show_toast(expander_limit_msg(cap.byte_size()));
                false
            }
            SizeDecision::AcceptWarn => {
                lock(shared).show_toast(format!(
                    "Item grande: {} / {}.",
                    format_mb(cap.byte_size()),
                    format_mb(EXPANDER_ITEM_MAX)
                ));
                *payload = cap;
                true
            }
            _ => {
                *payload = cap;
                true
            }
        }
    } else {
        lock(shared).show_toast("Nada para colar na área de transferência");
        false
    }
}

fn apply_editor_buf(payload: &mut ClipPayload, buf: &str, orig: &str) {
    if buf == orig {
        return;
    }
    payload.unicode = buf.to_string();
    payload.html = None;
    payload.rtf = None;
    payload.promote_html();
}

fn sync_editor_png(ctx: &egui::Context, mem: &mut UiMemory, payload: &ClipPayload) {
    let sig = payload.png.as_ref().map(|v| v.len()).unwrap_or(0);
    if sig == mem.editor_tex_sig && mem.editor_tex.is_some() {
        return;
    }
    mem.editor_tex_sig = sig;
    mem.editor_tex = None;
    let Some(bytes) = payload.png.as_ref() else {
        return;
    };
    let Ok(img) = image::load_from_memory(bytes) else {
        return;
    };
    let rgba = img.into_rgba8();
    let size = [rgba.width() as usize, rgba.height() as usize];
    let color = egui::ColorImage::from_rgba_unmultiplied(size, &rgba);
    mem.editor_tex = Some(ctx.load_texture("expander-editor-png", color, Default::default()));
}

fn draw_editor(ctx: &egui::Context, shared: &Shared, mem: &mut UiMemory) {
    const WIN_W: f32 = 800.0;
    const WIN_H: f32 = 900.0;
    let mut builder = ViewportBuilder::default()
        .with_title("Conteúdo do gatilho")
        .with_decorations(false)
        .with_always_on_top()
        .with_resizable(true)
        .with_min_inner_size([480.0, 400.0])
        .with_taskbar(false)
        .with_transparent(false)
        .with_inner_size([WIN_W, WIN_H]);
    if mem.editor_place {
        builder = builder
            .with_position(center_on_mouse_monitor(WIN_W, WIN_H))
            .with_active(true);
        mem.editor_place = false;
    }
    ctx.show_viewport_immediate(
        ViewportId::from_hash_of("editor"),
        builder,
        |ctx, _class| {
            if ctx.input(|i| i.viewport().close_requested())
                || ctx.input(|i| i.key_pressed(Key::Escape))
            {
                lock(shared).editor_visible = false;
            }
            egui::CentralPanel::default().frame(dark_frame()).show(ctx, |ui| {
                drag_title(ui, "Colar texto rico");
                ui.add_space(4.0);
                let row = lock(shared).editor_row;
                let mut payload = lock(shared)
                    .draft_payloads
                    .get(row)
                    .cloned()
                    .unwrap_or_default();
                ui.label(
                    RichText::new(
                        "O botão Colar captura o pacote (texto, HTML, RTF e imagem). No quadro abaixo dá para ler tudo, apagar e inserir. Ctrl+V no texto insere no cursor.",
                    )
                    .size(13.0),
                );
                ui.add_space(6.0);
                if ui
                    .add_sized(
                        [ui.available_width(), 32.0],
                        egui::Button::new(
                            RichText::new("Colar da área de transferência")
                                .strong()
                                .color(TEXT),
                        ),
                    )
                    .clicked()
                {
                    if apply_editor_capture(shared, &mut payload) {
                        let vis = crate::text::visible_text(&payload.unicode);
                        let mut s = lock(shared);
                        s.editor_buf = vis.clone();
                        s.editor_orig_visible = vis;
                        if row < s.draft_payloads.len() {
                            s.draft_payloads[row] = payload.clone();
                        }
                    }
                }
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(format!(
                            "{}  ·  {} / {}",
                            payload.flags_label(),
                            format_mb(payload.byte_size()),
                            format_mb(EXPANDER_ITEM_MAX)
                        ))
                        .size(13.0),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .add_sized(
                                [120.0, 28.0],
                                egui::Button::new(RichText::new("Limpar").color(TEXT)),
                            )
                            .clicked()
                        {
                            payload = ClipPayload::default();
                            let mut s = lock(shared);
                            s.editor_buf.clear();
                            s.editor_orig_visible.clear();
                            if row < s.draft_payloads.len() {
                                s.draft_payloads[row] = payload.clone();
                            }
                        }
                    });
                });
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ui
                        .add_sized(
                            [160.0, 32.0],
                            egui::Button::new(RichText::new("Salvar conteúdo").strong().color(TEXT)),
                        )
                        .clicked()
                    {
                        let buf = lock(shared).editor_buf.clone();
                        let orig = lock(shared).editor_orig_visible.clone();
                        apply_editor_buf(&mut payload, &buf, &orig);
                        match check_expander_size(payload.byte_size()) {
                            SizeDecision::ItemTooBig => {
                                lock(shared).show_toast(expander_limit_msg(payload.byte_size()));
                            }
                            _ => {
                                let mut s = lock(shared);
                                match s.save_editor_row(row, payload.clone()) {
                                    Ok(()) => {
                                        s.editor_visible = false;
                                        s.show_toast("Conteúdo salvo");
                                    }
                                    Err(e) => s.show_toast(e),
                                }
                            }
                        }
                    }
                    if ui
                        .add_sized(
                            [120.0, 32.0],
                            egui::Button::new(RichText::new("Fechar").strong().color(TEXT)),
                        )
                        .clicked()
                    {
                        lock(shared).editor_visible = false;
                    }
                });
                ui.add_space(6.0);
                sync_editor_png(ui.ctx(), mem, &payload);
                let scroll_h = (ui.available_height() - 8.0).max(120.0);
                ScrollArea::vertical()
                    .max_height(scroll_h)
                    .auto_shrink([false, false])
                    .id_salt("editor-body")
                    .show(ui, |ui| {
                        let mut buf = lock(shared).editor_buf.clone();
                        let rows = buf.lines().count().max(20);
                        let resp = ui.add(
                            TextEdit::multiline(&mut buf)
                                .desired_width(ui.available_width())
                                .desired_rows(rows)
                                .font(FontId::proportional(14.0)),
                        );
                        if resp.changed() {
                            lock(shared).editor_buf = buf;
                        }
                        if let Some(tex) = mem.editor_tex.as_ref() {
                            ui.add_space(10.0);
                            let max_w = ui.available_width().min(760.0);
                            let size = tex.size_vec2();
                            let scale = if size.x > 0.0 {
                                (max_w / size.x).min(1.0)
                            } else {
                                1.0
                            };
                            ui.image((tex.id(), size * scale));
                        }
                    });
            });
        },
    );
}

fn draw_data_dialog(ctx: &egui::Context, shared: &Shared) {
    let Some(kind) = lock(shared).data_dialog.clone() else {
        return;
    };
    match kind {
        DataDialog::Switch { to_portable } => {
            ctx.show_viewport_immediate(
                ViewportId::from_hash_of("data-dialog"),
                ViewportBuilder::default()
                    .with_title("Dados")
                    .with_decorations(false)
                    .with_always_on_top()
                    .with_resizable(false)
                    .with_taskbar(false)
                    .with_inner_size([460.0, 260.0])
                    .with_position(center_on_mouse_monitor(460.0, 260.0))
                    .with_active(true),
                |ctx, _class| {
                    if ctx.input(|i| i.viewport().close_requested()) {
                        lock(shared).data_dialog = None;
                    }
                    egui::CentralPanel::default().frame(dark_frame()).show(ctx, |ui| {
                        drag_title(ui, "Mudar local dos dados");
                        ui.add_space(8.0);
                        let dest = if to_portable {
                            "ao lado do programa (modo portátil)"
                        } else {
                            "na pasta deste usuário"
                        };
                        ui.label(format!(
                            "Os dados passam a ser gravados {dest}.\nOs dados atuais podem ser copiados para o destino.\nRecomendado: fazer um Backup antes. Sem backup, segue por sua conta e risco."
                        ));
                        ui.add_space(12.0);
                        if ui.button("Backup agora…").clicked() {
                            if let Some(path) = crate::dialog::save_backup() {
                                match crate::backup::export_zip(&path) {
                                    Ok(()) => lock(shared).show_toast("Backup exportado"),
                                    Err(e) => lock(shared).show_toast(e),
                                }
                            }
                        }
                        ui.add_space(6.0);
                        if ui.button("Copiar e mudar").clicked() {
                            apply_mode_switch(shared, to_portable, true);
                        }
                        if ui.button("Mudar sem copiar").clicked() {
                            apply_mode_switch(shared, to_portable, false);
                        }
                        if ui.button("Cancelar").clicked() {
                            lock(shared).data_dialog = None;
                        }
                    });
                },
            );
        }
        DataDialog::ConfirmImport { path } => {
            ctx.show_viewport_immediate(
                ViewportId::from_hash_of("import-confirm"),
                ViewportBuilder::default()
                    .with_title("Importar backup")
                    .with_decorations(false)
                    .with_always_on_top()
                    .with_resizable(false)
                    .with_taskbar(false)
                    .with_inner_size([480.0, 220.0])
                    .with_position(center_on_mouse_monitor(480.0, 220.0))
                    .with_active(true),
                |ctx, _class| {
                    let enter = ctx.input(|i| i.key_pressed(Key::Enter));
                    let cancel = ctx.input(|i| {
                        i.viewport().close_requested() || i.key_pressed(Key::Escape)
                    });
                    if cancel {
                        lock(shared).data_dialog = None;
                    }
                    egui::CentralPanel::default().frame(dark_frame()).show(ctx, |ui| {
                        drag_title(ui, "Importar backup");
                        ui.add_space(8.0);
                        ui.label(
                            "O programa vai importar os dados e reiniciar.\nO expansor abre de novo em seguida.\n\nContinuar?",
                        );
                        ui.add_space(6.0);
                        ui.label(
                            RichText::new(path.display().to_string())
                                .size(12.0)
                                .color(TEXT),
                        );
                        ui.add_space(16.0);
                        ui.horizontal(|ui| {
                            if ui
                                .add_sized(
                                    [140.0, 32.0],
                                    egui::Button::new(
                                        RichText::new("Continuar").strong().color(TEXT),
                                    ),
                                )
                                .clicked()
                                || enter
                            {
                                lock(shared).data_dialog = None;
                                match crate::backup::import_zip(&path) {
                                    Ok(()) => {
                                        crate::paths::write_open_expander_sentinel();
                                        crate::hook::restart_process();
                                    }
                                    Err(e) => lock(shared).show_toast(e),
                                }
                            }
                            if ui
                                .add_sized(
                                    [120.0, 32.0],
                                    egui::Button::new(RichText::new("Cancelar").color(TEXT)),
                                )
                                .clicked()
                            {
                                lock(shared).data_dialog = None;
                            }
                        });
                    });
                },
            );
        }
    }
}

fn apply_mode_switch(shared: &Shared, to_portable: bool, copy: bool) {
    let from = crate::paths::data_dir();
    match crate::paths::set_portable(to_portable) {
        Err(e) => {
            lock(shared).show_toast(e);
            lock(shared).data_dialog = None;
        }
        Ok(()) => {
            if copy {
                let to = crate::paths::data_dir();
                if let Err(e) = crate::paths::copy_data_dir(&from, &to) {
                    lock(shared).show_toast(e);
                    let _ = crate::paths::set_portable(!to_portable);
                    lock(shared).data_dialog = None;
                    return;
                }
            }
            lock(shared).data_dialog = None;
            crate::hook::restart_process();
        }
    }
}

fn tab_btn(ui: &mut Ui, shared: &Shared, id: u8, label: &str) {
    let selected = lock(shared).help_tab == id;
    let text = if selected {
        RichText::new(label).strong().color(TEXT)
    } else {
        RichText::new(label).color(TEXT)
    };
    if ui.add_sized([100.0, 30.0], egui::Button::new(text)).clicked() {
        lock(shared).help_tab = id;
    }
}

pub fn app_icon() -> egui::IconData {
    crate::icon::egui_icon()
}

pub fn run_already_running_dialog() {
    let mut mem = UiMemory::default();
    let icon = app_icon();
    let options = eframe::NativeOptions {
        viewport: ViewportBuilder::default()
            .with_title("b-snip")
            .with_inner_size([400.0, 170.0])
            .with_decorations(false)
            .with_always_on_top()
            .with_resizable(false)
            .with_taskbar(true)
            .with_transparent(false)
            .with_icon(egui::IconData {
                rgba: icon.rgba.clone(),
                width: icon.width,
                height: icon.height,
            }),
        persist_window: false,
        persistence_path: None,
        clipboard_shortcuts: false,
        centered: true,
        ..Default::default()
    };
    struct AlreadyDlg {
        mem: UiMemory,
    }
    impl eframe::App for AlreadyDlg {
        fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
            [
                0x2B as f32 / 255.0,
                0x2B as f32 / 255.0,
                0x2B as f32 / 255.0,
                1.0,
            ]
        }
        fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
            apply_theme(ui.ctx(), &mut self.mem);
            ui.vertical_centered(|ui| {
                ui.add_space(16.0);
                ui.label(RichText::new("b-snip").strong().size(18.0).color(TEXT));
                ui.add_space(10.0);
                ui.label(
                    RichText::new("O b-snip já está em execução.")
                        .size(14.0)
                        .color(TEXT),
                );
                ui.add_space(18.0);
                let enter = ui
                    .ctx()
                    .input(|i| i.key_pressed(Key::Enter) || i.key_pressed(Key::Escape));
                if ui
                    .add_sized(
                        [120.0, 32.0],
                        egui::Button::new(RichText::new("OK").strong().color(TEXT)),
                    )
                    .clicked()
                    || enter
                {
                    ui.ctx().send_viewport_cmd(ViewportCommand::Close);
                }
            });
        }
    }
    let _ = eframe::run_native(
        "b-snip",
        options,
        Box::new(move |cc| {
            apply_theme(&cc.egui_ctx, &mut mem);
            Ok(Box::new(AlreadyDlg { mem }))
        }),
    );
}
