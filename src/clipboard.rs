use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Mutex, MutexGuard};
use std::thread::sleep;
use std::time::Duration;

use windows::core::w;
use windows::Win32::Foundation::{HANDLE, HGLOBAL};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, GetClipboardData, IsClipboardFormatAvailable, OpenClipboard,
    RegisterClipboardFormatW, SetClipboardData,
};
use windows::Win32::System::Memory::{
    GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock, GMEM_MOVEABLE, GMEM_ZEROINIT,
};

use crate::payload::ClipPayload;

const CF_UNICODETEXT: u32 = 13;
const CF_BITMAP: u32 = 2;
const CF_DIB: u32 = 8;
const CF_HDROP: u32 = 15;

/// Uma abertura de clipboard por vez. O gancho só tenta; se estiver ocupado, desiste.
static CLIP_GATE: Mutex<()> = Mutex::new(());

fn clip_gate() -> MutexGuard<'static, ()> {
    CLIP_GATE.lock().unwrap_or_else(|e| e.into_inner())
}

/// Arquivo/pasta sempre pausa. Imagem pura também. Texto (mesmo com bitmap de preview) não.
pub fn should_pause_queue(hdrop: bool, image: bool, rich: bool, has_text: bool) -> bool {
    if hdrop {
        return true;
    }
    image && !rich && !has_text
}

fn html_fmt() -> u32 {
    static ID: AtomicU32 = AtomicU32::new(0);
    registered(&ID, w!("HTML Format"))
}

fn rtf_fmt() -> u32 {
    static ID: AtomicU32 = AtomicU32::new(0);
    registered(&ID, w!("Rich Text Format"))
}

fn png_fmt() -> u32 {
    static ID: AtomicU32 = AtomicU32::new(0);
    registered(&ID, w!("PNG"))
}

fn registered(slot: &AtomicU32, name: windows::core::PCWSTR) -> u32 {
    let cur = slot.load(Ordering::Relaxed);
    if cur != 0 {
        return cur;
    }
    let id = unsafe { RegisterClipboardFormatW(name) };
    if id != 0 {
        slot.store(id, Ordering::Relaxed);
    }
    id
}

pub fn is_hdrop() -> bool {
    unsafe { IsClipboardFormatAvailable(CF_HDROP).is_ok() }
}

pub fn has_rich_format() -> bool {
    let html = html_fmt();
    let rtf = rtf_fmt();
    unsafe {
        (html != 0 && IsClipboardFormatAvailable(html).is_ok())
            || (rtf != 0 && IsClipboardFormatAvailable(rtf).is_ok())
    }
}

/// Arquivo do Explorer, ou imagem crua (print) sem texto e sem HTML/RTF.
pub fn is_file_or_image_only() -> bool {
    let image = unsafe {
        IsClipboardFormatAvailable(CF_BITMAP).is_ok()
            || IsClipboardFormatAvailable(CF_DIB).is_ok()
            || {
                let png = png_fmt();
                png != 0 && IsClipboardFormatAvailable(png).is_ok()
            }
    };
    should_pause_queue(is_hdrop(), image, has_rich_format(), has_unicode_text())
}

#[allow(dead_code)]
pub fn is_file_or_bitmap() -> bool {
    is_file_or_image_only()
}

pub fn has_unicode_text() -> bool {
    unsafe { IsClipboardFormatAvailable(CF_UNICODETEXT).is_ok() }
}

pub fn capture() -> Option<ClipPayload> {
    let _gate = clip_gate();
    unsafe {
        OpenClipboard(None).ok()?;
        let result = capture_open();
        let _ = CloseClipboard();
        result
    }
}

unsafe fn capture_open() -> Option<ClipPayload> {
    let unicode = get_unicode_open().unwrap_or_default();
    let html = get_bytes_open(html_fmt());
    let rtf = get_bytes_open(rtf_fmt());
    let png = get_bytes_open(png_fmt());
    let dib = get_bytes_open(CF_DIB);
    let mut payload = ClipPayload {
        unicode,
        html,
        rtf,
        png,
        dib,
    };
    payload.promote_html();
    if payload.is_empty() {
        None
    } else {
        Some(payload)
    }
}

#[allow(dead_code)]
pub fn get_unicode_text() -> Option<String> {
    let _gate = clip_gate();
    unsafe {
        OpenClipboard(None).ok()?;
        let text = get_unicode_open();
        let _ = CloseClipboard();
        text.filter(|t| !t.is_empty())
    }
}

unsafe fn get_unicode_open() -> Option<String> {
    IsClipboardFormatAvailable(CF_UNICODETEXT).ok()?;
    let handle = GetClipboardData(CF_UNICODETEXT).ok()?;
    if handle.0.is_null() {
        return None;
    }
    let hglobal = HGLOBAL(handle.0);
    let size = GlobalSize(hglobal);
    if size < 2 {
        return None;
    }
    let ptr = GlobalLock(hglobal) as *const u16;
    if ptr.is_null() {
        return None;
    }
    let max = (size as usize / 2).min(4_000_000);
    let mut len = 0usize;
    while len < max && *ptr.add(len) != 0 {
        len += 1;
    }
    let slice = std::slice::from_raw_parts(ptr, len);
    let text = String::from_utf16_lossy(slice);
    let _ = GlobalUnlock(hglobal);
    Some(text)
}

unsafe fn get_bytes_open(fmt: u32) -> Option<Vec<u8>> {
    if fmt == 0 {
        return None;
    }
    IsClipboardFormatAvailable(fmt).ok()?;
    let handle = GetClipboardData(fmt).ok()?;
    if handle.0.is_null() {
        return None;
    }
    let hglobal = HGLOBAL(handle.0);
    let size = GlobalSize(hglobal);
    if size == 0 || size > EXPANDER_CLIP_CAP {
        return None;
    }
    let ptr = GlobalLock(hglobal) as *const u8;
    if ptr.is_null() {
        return None;
    }
    let bytes = std::slice::from_raw_parts(ptr, size).to_vec();
    let _ = GlobalUnlock(hglobal);
    if bytes.is_empty() {
        None
    } else {
        Some(bytes)
    }
}

const EXPANDER_CLIP_CAP: usize = 12 * 1024 * 1024;

pub fn set_payload(payload: &ClipPayload) -> bool {
    let _gate = clip_gate();
    set_payload_unlocked(payload)
}

/// Não espera. O gancho usa isto para não travar o teclado se o worker estiver no clipboard.
pub fn try_set_payload(payload: &ClipPayload) -> bool {
    let Ok(_gate) = CLIP_GATE.try_lock() else {
        return false;
    };
    set_payload_unlocked(payload)
}

fn set_payload_unlocked(payload: &ClipPayload) -> bool {
    unsafe {
        if OpenClipboard(None).is_err() {
            return false;
        }
        let ok = set_payload_open(payload);
        let _ = CloseClipboard();
        ok
    }
}

pub fn set_unicode_text(text: &str) -> bool {
    set_payload(&ClipPayload::from_unicode(text))
}

unsafe fn set_payload_open(payload: &ClipPayload) -> bool {
    let _ = EmptyClipboard();
    let mut any = false;
    if !payload.unicode.is_empty() {
        any |= set_unicode_open(&payload.unicode);
    }
    if let Some(html) = payload.html.as_deref() {
        any |= set_bytes_open(html_fmt(), html);
    }
    if let Some(rtf) = payload.rtf.as_deref() {
        any |= set_bytes_open(rtf_fmt(), rtf);
    }
    if let Some(png) = payload.png.as_deref() {
        any |= set_bytes_open(png_fmt(), png);
    }
    if let Some(dib) = payload.dib.as_deref() {
        any |= set_bytes_open(CF_DIB, dib);
    }
    any
}

unsafe fn set_unicode_open(text: &str) -> bool {
    let mut wide: Vec<u16> = text.encode_utf16().collect();
    wide.push(0);
    let bytes = wide.len() * 2;
    let Ok(hmem) = GlobalAlloc(GMEM_MOVEABLE | GMEM_ZEROINIT, bytes) else {
        return false;
    };
    if hmem.0.is_null() {
        return false;
    }
    let ptr = GlobalLock(hmem) as *mut u16;
    if ptr.is_null() {
        return false;
    }
    std::ptr::copy_nonoverlapping(wide.as_ptr(), ptr, wide.len());
    let _ = GlobalUnlock(hmem);
    SetClipboardData(CF_UNICODETEXT, Some(HANDLE(hmem.0))).is_ok()
}

unsafe fn set_bytes_open(fmt: u32, data: &[u8]) -> bool {
    if fmt == 0 || data.is_empty() {
        return false;
    }
    let Ok(hmem) = GlobalAlloc(GMEM_MOVEABLE | GMEM_ZEROINIT, data.len()) else {
        return false;
    };
    if hmem.0.is_null() {
        return false;
    }
    let ptr = GlobalLock(hmem) as *mut u8;
    if ptr.is_null() {
        return false;
    }
    std::ptr::copy_nonoverlapping(data.as_ptr(), ptr, data.len());
    let _ = GlobalUnlock(hmem);
    SetClipboardData(fmt, Some(HANDLE(hmem.0))).is_ok()
}

pub fn clear() {
    let _gate = clip_gate();
    unsafe {
        if OpenClipboard(None).is_ok() {
            let _ = EmptyClipboard();
            let _ = CloseClipboard();
        }
    }
}

pub fn wait_unicode(timeout: Duration) -> bool {
    let start = std::time::Instant::now();
    while start.elapsed() < timeout {
        if has_unicode_text() || has_rich_format() {
            return true;
        }
        sleep(Duration::from_millis(15));
    }
    false
}

#[cfg(test)]
mod tests {
    use super::should_pause_queue;

    #[test]
    fn arquivo_pausa_mesmo_com_texto() {
        assert!(should_pause_queue(true, false, false, true));
    }

    #[test]
    fn imagem_pura_pausa() {
        assert!(should_pause_queue(false, true, false, false));
    }

    #[test]
    fn texto_com_bitmap_nao_pausa() {
        assert!(!should_pause_queue(false, true, false, true));
        assert!(!should_pause_queue(false, true, true, true));
    }

    #[test]
    fn texto_simples_nao_pausa() {
        assert!(!should_pause_queue(false, false, false, true));
    }
}
