use std::collections::VecDeque;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;
use std::time::Instant;

use crate::config::{AppConfig, Features, WindowPos};
use crate::payload::{
    check_queue_size, queue_item_limit_msg, queue_total_limit_msg, queue_warn_msg, ClipPayload,
    SizeDecision, MAX_QUEUE_ITEMS, QUEUE_FULL_MSG,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransformKind {
    Title,
    Upper,
    Lower,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DataDialog {
    Switch { to_portable: bool },
    ConfirmImport { path: std::path::PathBuf },
}

pub struct AppState {
    pub queue: VecDeque<ClipPayload>,
    pub last_payload: ClipPayload,
    pub is_pasting: bool,
    pub is_transforming: bool,
    pub is_expanding: bool,
    pub cutting_until: Option<Instant>,
    pub transform_visible: bool,
    pub config_visible: bool,
    pub help_visible: bool,
    pub editor_visible: bool,
    pub editor_row: usize,
    pub editor_buf: String,
    pub editor_orig_visible: String,
    pub data_dialog: Option<DataDialog>,
    pub saved_payload: ClipPayload,
    pub transform_hwnd: isize,
    /// Nome amigável da linha do expansor (antes era o texto cru).
    pub slots: Vec<String>,
    pub triggers: Vec<String>,
    pub draft_slots: Vec<String>,
    pub draft_triggers: Vec<String>,
    pub draft_payloads: Vec<ClipPayload>,
    pub window_pos: WindowPos,
    pub config_just_opened: bool,
    pub request_exit: bool,
    pub request_reload: bool,
    pub help_tab: u8,
    pub next_index: usize,
    pub done_count: usize,
    pub ignore_clip_until: Option<Instant>,
    pub copy_queue_on: bool,
    pub expander_on: bool,
    pub transform_on: bool,
    pub loop_on: bool,
    pub sound_on: bool,
    pub clear_on_last: bool,
    pub queue_paused: bool,
    pub toast: Option<(String, Instant)>,
    /// Pacote do expansor: o próximo clipboard igual a este não entra na fila.
    pub skip_clip: Option<ClipPayload>,
    /// Cada Ctrl+C/X do usuário. Serve para o mesmo texto entrar de novo na fila.
    pub copy_gen: u64,
    pub last_enqueued_gen: u64,
    pub pasting_since: Option<Instant>,
    /// Ctrl+V real já foi armado: o clipboard tem o item e a tecla segue para o app.
    pub passthrough_armed: bool,
    pub paste_seq: u64,
    /// Seq do Ctrl+V que esvaziou a fila com “limpar ao colar o último”.
    pub pending_clear_seq: Option<u64>,
}

impl AppState {
    pub fn from_config(cfg: &AppConfig) -> Self {
        Self {
            queue: VecDeque::new(),
            last_payload: ClipPayload::default(),
            is_pasting: false,
            is_transforming: false,
            is_expanding: false,
            cutting_until: None,
            transform_visible: false,
            config_visible: false,
            help_visible: false,
            editor_visible: false,
            editor_row: 0,
            editor_buf: String::new(),
            editor_orig_visible: String::new(),
            data_dialog: None,
            saved_payload: ClipPayload::default(),
            transform_hwnd: 0,
            slots: cfg.slots.clone(),
            triggers: cfg.triggers.clone(),
            draft_slots: cfg.slots.clone(),
            draft_triggers: cfg.triggers.clone(),
            draft_payloads: Vec::new(),
            window_pos: cfg.window_pos,
            config_just_opened: false,
            request_exit: false,
            request_reload: false,
            help_tab: 1,
            next_index: 0,
            done_count: 0,
            ignore_clip_until: None,
            copy_queue_on: cfg.features.copy_queue,
            expander_on: cfg.features.expander,
            transform_on: cfg.features.transform,
            loop_on: cfg.features.loop_on,
            sound_on: cfg.features.sound,
            clear_on_last: cfg.features.clear_on_last && !cfg.features.loop_on,
            queue_paused: false,
            toast: None,
            skip_clip: None,
            copy_gen: 0,
            last_enqueued_gen: 0,
            pasting_since: None,
            passthrough_armed: false,
            paste_seq: 0,
            pending_clear_seq: None,
        }
    }

    pub fn begin_pasting(&mut self) {
        self.is_pasting = true;
        self.pasting_since = Some(Instant::now());
    }

    pub fn end_pasting(&mut self) {
        self.is_pasting = false;
        self.pasting_since = None;
        self.passthrough_armed = false;
    }

    /// Colagem que não terminou em 3s não pode calar a fila para sempre.
    pub fn release_stuck_pasting(&mut self) -> bool {
        if !self.is_pasting {
            return false;
        }
        let stuck = self
            .pasting_since
            .map(|t| t.elapsed() >= Duration::from_secs(3))
            .unwrap_or(true);
        if stuck {
            self.end_pasting();
            true
        } else {
            false
        }
    }

    pub fn peek_paste_payload(&self) -> Option<ClipPayload> {
        if self.queue.is_empty() {
            return None;
        }
        let idx = if self.loop_on {
            if self.next_index >= self.queue.len() {
                0
            } else {
                self.next_index
            }
        } else {
            0
        };
        self.queue.get(idx).cloned()
    }

    /// O clipboard já mostra o item que `peek_paste_payload` devolveu.
    /// Devolve true quando “limpar ao colar o último” deve esvaziar o clipboard depois.
    pub fn commit_shown_paste(&mut self) -> bool {
        if self.queue.is_empty() {
            return false;
        }
        if self.loop_on {
            if self.next_index >= self.queue.len() {
                self.next_index = 0;
            }
            self.next_index = (self.next_index + 1) % self.queue.len();
            return false;
        }
        let _ = self.queue.pop_front();
        self.done_count += 1;
        if self.queue.is_empty() {
            self.done_count = 0;
            self.next_index = 0;
            let clear = self.clear_on_last;
            if clear {
                self.clear_queue_data();
            }
            clear
        } else {
            self.next_index = 0;
            false
        }
    }

    pub fn features(&self) -> Features {
        Features {
            copy_queue: self.copy_queue_on,
            expander: self.expander_on,
            transform: self.transform_on,
            loop_on: self.loop_on,
            sound: self.sound_on,
            clear_on_last: self.clear_on_last && !self.loop_on,
        }
    }

    pub fn persist(&self) {
        crate::config::save_all(
            &crate::config::ini_path(),
            &self.slots,
            &self.triggers,
            self.window_pos,
            true,
            self.features(),
        );
    }

    pub fn persist_expander_db(&self) {
        if self.draft_payloads.is_empty() {
            return;
        }
        let n = self.triggers.len().max(self.slots.len());
        let mut rows = Vec::with_capacity(n);
        for i in 0..n {
            let trig = self.triggers.get(i).cloned().unwrap_or_default();
            let name = self.slots.get(i).cloned().unwrap_or_default();
            let payload = self
                .draft_payloads
                .get(i)
                .cloned()
                .unwrap_or_default();
            rows.push((trig, name, payload));
        }
        let _ = crate::store::replace_all(&rows);
    }

    pub fn save_editor_row(&mut self, row: usize, payload: ClipPayload) -> Result<(), String> {
        if row >= self.draft_payloads.len() {
            return Err("Linha inválida".into());
        }
        self.draft_payloads[row] = payload.clone();
        let trig = self.draft_triggers.get(row).cloned().unwrap_or_default();
        let name = self.draft_slots.get(row).cloned().unwrap_or_default();
        crate::store::set_payload(row as i64, &trig, &name, &payload)?;
        Ok(())
    }

    pub fn load_drafts_from_db(&mut self) {
        let meta = crate::store::load_meta();
        if meta.is_empty() {
            self.draft_triggers = self.triggers.clone();
            self.draft_slots = self.slots.clone();
            let n = self.draft_triggers.len();
            self.draft_payloads = (0..n)
                .map(|i| crate::store::get_payload(i as i64).unwrap_or_default())
                .collect();
            return;
        }
        self.draft_triggers = meta.iter().map(|m| m.trigger.clone()).collect();
        self.draft_slots = meta.iter().map(|m| m.name.clone()).collect();
        self.draft_payloads = (0..meta.len())
            .map(|i| crate::store::get_payload(i as i64).unwrap_or_default())
            .collect();
    }

    pub fn show_toast(&mut self, msg: impl Into<String>) {
        self.show_toast_for(msg, 3);
    }

    pub fn show_toast_for(&mut self, msg: impl Into<String>, secs: u64) {
        self.toast = Some((msg.into(), Instant::now() + Duration::from_secs(secs)));
    }

    pub fn queue_bytes(&self) -> usize {
        self.queue.iter().map(|p| p.byte_size()).sum()
    }

    pub fn try_enqueue(&mut self, payload: ClipPayload) -> bool {
        if payload.is_empty() {
            return false;
        }
        let item = payload.byte_size();
        let total_after = self.queue_bytes() + item;
        let count_after = self.queue.len() + 1;
        match check_queue_size(item, total_after, count_after) {
            SizeDecision::QueueFull => {
                self.show_toast_for(QUEUE_FULL_MSG, 8);
                return false;
            }
            SizeDecision::ItemTooBig => {
                self.show_toast_for(queue_item_limit_msg(item), 6);
                return false;
            }
            SizeDecision::TotalTooBig => {
                self.show_toast_for(queue_total_limit_msg(total_after), 6);
                return false;
            }
            SizeDecision::AcceptWarn => {
                self.show_toast_for(queue_warn_msg(item, total_after), 4);
            }
            SizeDecision::Accept => {}
        }
        let _ = MAX_QUEUE_ITEMS;
        self.last_payload = payload.clone();
        self.queue.push_back(payload);
        if self.queue.len() == 1 {
            self.next_index = 0;
        }
        true
    }

    pub fn toast_text(&self) -> Option<&str> {
        let (msg, until) = self.toast.as_ref()?;
        if Instant::now() < *until {
            Some(msg.as_str())
        } else {
            None
        }
    }

    pub fn clear_queue_data(&mut self) {
        self.queue.clear();
        self.last_payload = ClipPayload::default();
        self.next_index = 0;
        self.done_count = 0;
    }

    pub fn toggle_copy_queue(&mut self) -> bool {
        self.copy_queue_on = !self.copy_queue_on;
        if !self.copy_queue_on {
            self.queue_paused = false;
            self.clear_queue_data();
            self.show_toast("Cópia contínua desativada");
        } else {
            self.queue_paused = false;
            self.show_toast("Cópia contínua ativada");
        }
        self.copy_queue_on
    }

    pub fn is_cutting(&self) -> bool {
        self.cutting_until
            .map(|t| Instant::now() < t)
            .unwrap_or(false)
    }

    pub fn should_ignore_clipboard(&self) -> bool {
        if !self.copy_queue_on || self.queue_paused {
            return true;
        }
        if self.is_pasting || self.is_cutting() || self.is_transforming || self.is_expanding {
            return true;
        }
        self.ignore_clip_until
            .map(|t| Instant::now() < t)
            .unwrap_or(false)
    }

    /// Conteúdo novo sempre entra. Conteúdo igual só entra se foi outro Ctrl+C/X.
    pub fn accept_repeat_copy(&self, same_as_last: bool) -> bool {
        !same_as_last || self.copy_gen > self.last_enqueued_gen
    }

    pub fn cycle_label(&self) -> (usize, usize) {
        let remaining = self.queue.len();
        if remaining == 0 {
            return (0, 0);
        }
        if self.loop_on {
            let current = (self.next_index % remaining) + 1;
            (current, remaining)
        } else {
            (self.done_count + 1, self.done_count + remaining)
        }
    }

    pub fn trigger_for_slot(&self, slot_index: usize) -> &str {
        self.triggers
            .get(slot_index)
            .map(|s| s.as_str())
            .unwrap_or("")
    }

    #[allow(dead_code)]
    pub fn text_for_slot(&self, slot_index: usize) -> &str {
        self.draft_payloads
            .get(slot_index)
            .map(|p| p.unicode.as_str())
            .or_else(|| {
                self.slots
                    .get(slot_index)
                    .map(|s| s.as_str())
            })
            .unwrap_or("")
    }
}

pub type Shared = Arc<Mutex<AppState>>;

pub fn lock(shared: &Shared) -> MutexGuard<'_, AppState> {
    shared.lock().unwrap_or_else(|e| e.into_inner())
}

pub fn try_lock(shared: &Shared) -> Option<MutexGuard<'_, AppState>> {
    shared.try_lock().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AppConfig;

    #[test]
    fn copia_igual_so_entra_com_novo_ctrl_c() {
        let mut s = AppState::from_config(&AppConfig::default());
        s.copy_gen = 1;
        s.last_enqueued_gen = 1;
        assert!(!s.accept_repeat_copy(true));
        s.copy_gen = 2;
        assert!(s.accept_repeat_copy(true));
        assert!(s.accept_repeat_copy(false));
    }

    #[test]
    fn cola_avanca_do_mais_antigo() {
        let mut s = AppState::from_config(&AppConfig::default());
        s.loop_on = false;
        s.queue.push_back(ClipPayload::from_unicode("a"));
        s.queue.push_back(ClipPayload::from_unicode("b"));
        assert_eq!(s.peek_paste_payload().unwrap().unicode, "a");
        assert!(!s.commit_shown_paste());
        assert_eq!(s.peek_paste_payload().unwrap().unicode, "b");
        assert!(!s.commit_shown_paste());
        assert!(s.peek_paste_payload().is_none());
    }

    #[test]
    fn limpar_no_ultimo_so_depois_de_esvaziar() {
        let mut s = AppState::from_config(&AppConfig::default());
        s.loop_on = false;
        s.clear_on_last = true;
        s.queue.push_back(ClipPayload::from_unicode("a"));
        assert!(s.commit_shown_paste());
        assert!(s.queue.is_empty());
    }
}
