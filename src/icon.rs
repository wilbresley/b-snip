use std::io::Cursor;

use eframe::egui::IconData;
use image::imageops::{crop_imm, resize, FilterType};
use image::RgbaImage;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateIconFromResourceEx, HICON, LR_DEFAULTCOLOR,
};

const PNG: &[u8] = include_bytes!("../assets/b-snip.png");

/// Recorta o quadrado do logo (sem as barras pretas do PNG widescreen).
fn content_square(img: &RgbaImage) -> RgbaImage {
    let (w, h) = img.dimensions();
    let mut min_x = w;
    let mut min_y = h;
    let mut max_x = 0u32;
    let mut max_y = 0u32;
    for y in 0..h {
        for x in 0..w {
            let [r, g, b, a] = img.get_pixel(x, y).0;
            if a < 12 || (r < 14 && g < 14 && b < 14) {
                continue;
            }
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(x);
            max_y = max_y.max(y);
        }
    }
    if max_x < min_x {
        let side = w.min(h);
        return crop_imm(img, (w - side) / 2, (h - side) / 2, side, side).to_image();
    }
    let bw = max_x - min_x + 1;
    let bh = max_y - min_y + 1;
    let mut side = bw.max(bh);
    let pad = (side / 18).max(2);
    side = (side + pad * 2).min(w).min(h);
    let cx = min_x + bw / 2;
    let cy = min_y + bh / 2;
    let mut x = cx.saturating_sub(side / 2);
    let mut y = cy.saturating_sub(side / 2);
    if x + side > w {
        x = w - side;
    }
    if y + side > h {
        y = h - side;
    }
    crop_imm(img, x, y, side, side).to_image()
}

fn source_square() -> RgbaImage {
    let img = image::load_from_memory(PNG)
        .expect("assets/b-snip.png")
        .to_rgba8();
    content_square(&img)
}

fn logo_rgba(size: u32) -> RgbaImage {
    resize(&source_square(), size, size, FilterType::Lanczos3)
}

pub fn egui_icon() -> IconData {
    let img = logo_rgba(256);
    IconData {
        rgba: img.into_raw(),
        width: 256,
        height: 256,
    }
}

fn png_bytes(size: u32) -> Vec<u8> {
    let img = logo_rgba(size);
    let mut png = Vec::new();
    let _ = img.write_to(&mut Cursor::new(&mut png), image::ImageFormat::Png);
    png
}

pub fn tray_hicon() -> Option<HICON> {
    // 32px preenchendo o canvas: a bandeja escala, mas o logo não fica minúsculo no preto.
    let png = png_bytes(32);
    unsafe {
        CreateIconFromResourceEx(&png, true, 0x0003_0000, 32, 32, LR_DEFAULTCOLOR)
            .ok()
            .filter(|h| !h.0.is_null())
            .or_else(|| {
                let png48 = png_bytes(48);
                CreateIconFromResourceEx(&png48, true, 0x0003_0000, 48, 48, LR_DEFAULTCOLOR)
                    .ok()
                    .filter(|h| !h.0.is_null())
            })
    }
}
