use std::sync::{Mutex, OnceLock};
use std::thread;

use windows::core::PCWSTR;
use windows::Win32::Media::Multimedia::mciSendStringW;

use crate::state::{lock, Shared};

const MP3: &[u8] = include_bytes!("../assets/sondapp.mp3");
const ALIAS: &str = "bsnipSnd";

static MCI: Mutex<()> = Mutex::new(());
static MP3_PATH: OnceLock<std::path::PathBuf> = OnceLock::new();

fn mp3_path() -> &'static std::path::PathBuf {
    MP3_PATH.get_or_init(|| {
        let path = std::env::temp_dir().join("b-snip-sondapp.mp3");
        let _ = std::fs::write(&path, MP3);
        path
    })
}

fn mci(cmd: &str) -> u32 {
    let wide: Vec<u16> = cmd.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe { mciSendStringW(PCWSTR(wide.as_ptr()), None, None) }
}

/// Toca o som do app (limpar fila, etc.). Respeita a opção Som do menu.
pub fn play_notify(shared: &Shared) {
    if !lock(shared).sound_on {
        return;
    }
    thread::spawn(|| {
        let _guard = MCI.lock().unwrap_or_else(|e| e.into_inner());
        let path = mp3_path();
        let _ = mci(&format!("stop {ALIAS}"));
        let _ = mci(&format!("close {ALIAS}"));
        let quoted = format!("\"{}\"", path.display());
        let opened = mci(&format!("open {quoted} alias {ALIAS}")) == 0
            || mci(&format!("open {quoted} type mpegvideo alias {ALIAS}")) == 0;
        if opened {
            let _ = mci(&format!("play {ALIAS}"));
        }
    });
}

#[allow(dead_code)]
pub fn beep_if(shared: &Shared, freq: u32, ms: u32) {
    if lock(shared).sound_on {
        crate::input::beep(freq, ms);
    }
}
