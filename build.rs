use std::path::PathBuf;
use std::process::Command;

use image::imageops::{crop_imm, resize, FilterType};
use image::RgbaImage;

fn main() {
    println!("cargo:rerun-if-changed=assets/b-snip.png");

    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let ico_path = out.join("app.ico");
    let png = std::fs::read("assets/b-snip.png").expect("assets/b-snip.png");
    let img = image::load_from_memory(&png).expect("png").to_rgba8();
    let square = content_square(&img);

    // BMP DIB (não PNG-in-ICO): o Explorer e o windres gnu leem isso direito.
    let sizes: [u32; 4] = [16, 32, 48, 256];
    let images: Vec<(u32, Vec<u8>)> = sizes
        .iter()
        .map(|&size| {
            let resized = resize(&square, size, size, FilterType::Lanczos3);
            (size, encode_dib(&resized))
        })
        .collect();

    let count = images.len() as u16;
    let header = 6u32 + 16u32 * u32::from(count);
    let mut ico = Vec::new();
    ico.extend_from_slice(&0u16.to_le_bytes());
    ico.extend_from_slice(&1u16.to_le_bytes());
    ico.extend_from_slice(&count.to_le_bytes());
    let mut offset = header;
    for (size, data) in &images {
        let dim = if *size >= 256 { 0u8 } else { *size as u8 };
        ico.push(dim);
        ico.push(dim);
        ico.push(0);
        ico.push(0);
        ico.extend_from_slice(&1u16.to_le_bytes());
        ico.extend_from_slice(&32u16.to_le_bytes());
        ico.extend_from_slice(&(data.len() as u32).to_le_bytes());
        ico.extend_from_slice(&offset.to_le_bytes());
        offset += data.len() as u32;
    }
    for (_, data) in &images {
        ico.extend_from_slice(data);
    }
    std::fs::write(&ico_path, &ico).expect("write ico");

    let rc_path = out.join("app.rc");
    let res_path = out.join("app.res");
    std::fs::write(&rc_path, "1 ICON \"app.ico\"\r\n").expect("write rc");

    let status = Command::new("windres")
        .current_dir(&out)
        .args(["-i", "app.rc", "-o", "app.res", "--output-format=coff"])
        .status()
        .expect("windres não encontrado no PATH (MinGW)");
    assert!(status.success(), "windres falhou ao embutir o ícone no EXE");
    println!("cargo:rustc-link-arg={}", res_path.display());
}

fn encode_dib(img: &RgbaImage) -> Vec<u8> {
    let w = img.width();
    let h = img.height();
    let xor_stride = w * 4;
    let and_stride = ((w + 31) / 32) * 4;
    let xor_size = xor_stride * h;
    let and_size = and_stride * h;

    let mut dib = Vec::with_capacity(40 + (xor_size + and_size) as usize);
    dib.extend_from_slice(&40u32.to_le_bytes());
    dib.extend_from_slice(&(w as i32).to_le_bytes());
    dib.extend_from_slice(&((h * 2) as i32).to_le_bytes());
    dib.extend_from_slice(&1u16.to_le_bytes());
    dib.extend_from_slice(&32u16.to_le_bytes());
    dib.extend_from_slice(&0u32.to_le_bytes());
    dib.extend_from_slice(&(xor_size + and_size).to_le_bytes());
    dib.extend_from_slice(&[0u8; 16]);

    for y in (0..h).rev() {
        for x in 0..w {
            let [r, g, b, a] = img.get_pixel(x, y).0;
            dib.push(b);
            dib.push(g);
            dib.push(r);
            dib.push(a);
        }
    }
    for y in (0..h).rev() {
        let mut row = vec![0u8; and_stride as usize];
        for x in 0..w {
            if img.get_pixel(x, y).0[3] < 128 {
                row[(x / 8) as usize] |= 1 << (7 - (x % 8));
            }
        }
        dib.extend_from_slice(&row);
    }
    dib
}

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
