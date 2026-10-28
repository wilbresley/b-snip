#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod backup;
mod clipboard;
mod config;
mod dialog;
mod hook;
mod icon;
mod input;
mod paths;
mod payload;
mod sound;
mod startup;
mod state;
mod store;
mod text;
mod ui;

use std::sync::{Arc, Mutex};

use eframe::egui::{IconData, ViewportBuilder};
use state::AppState;

fn main() {
    if !hook::try_single_instance() {
        app::show_already_running();
        return;
    }

    paths::migrate_legacy_ini_if_needed();
    startup::migrate_legacy_run_key();
    let ini = config::ini_path();
    let mut cfg = config::load(&ini);
    store::import_from_ini_if_empty(&cfg.triggers, &cfg.slots);
    let meta = store::load_meta();
    if !meta.is_empty() {
        cfg.triggers = meta.iter().map(|m| m.trigger.clone()).collect();
        cfg.slots = meta.iter().map(|m| m.name.clone()).collect();
    }
    let welcome_already = cfg.welcome_shown;
    let open_expander = paths::take_open_expander_sentinel();
    let shared = Arc::new(Mutex::new(AppState::from_config(&cfg)));
    app::maybe_welcome(&shared, welcome_already);
    if open_expander {
        let mut s = state::lock(&shared);
        s.config_visible = true;
        s.config_just_opened = true;
        s.help_visible = false;
    }
    hook::start(shared.clone());
    unsafe {
        let _ = windows::Win32::UI::WindowsAndMessaging::AllowSetForegroundWindow(u32::MAX);
    }

    let icon = ui::app_icon();
    let options = eframe::NativeOptions {
        viewport: ViewportBuilder::default()
            .with_title("b-snip")
            .with_visible(false)
            .with_taskbar(false)
            .with_decorations(false)
            .with_transparent(false)
            .with_has_shadow(false)
            .with_active(false)
            .with_inner_size([1.0, 1.0])
            .with_position([-32000.0, -32000.0])
            .with_icon(IconData {
                rgba: icon.rgba.clone(),
                width: icon.width,
                height: icon.height,
            }),
        persist_window: false,
        persistence_path: None,
        clipboard_shortcuts: false,
        centered: false,
        ..Default::default()
    };

    let _ = eframe::run_native(
        "b-snip",
        options,
        Box::new(move |cc| Ok(Box::new(app::WbcApp::new(cc, shared)))),
    );

    hook::release_instance_mutex();
    hook::shutdown_hook_thread();
}
