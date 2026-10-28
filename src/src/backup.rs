use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;

use zip::write::SimpleFileOptions;
use zip::ZipArchive;
use zip::ZipWriter;

use crate::config::INI_NAME;
use crate::paths::{self, DB_NAME};

const ZIP_INI: &str = "ClipboardExpander.ini";
const ZIP_DB: &str = "expander.db";

pub fn export_zip(dest: &Path) -> Result<(), String> {
    let dir = paths::data_dir();
    let file = File::create(dest).map_err(|e| format!("Não deu para criar o backup: {e}"))?;
    let mut zip = ZipWriter::new(file);
    let opts = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    for (name, disk) in [
        (ZIP_INI, dir.join(INI_NAME)),
        (ZIP_DB, dir.join(DB_NAME)),
    ] {
        if !disk.is_file() {
            continue;
        }
        zip.start_file(name, opts)
            .map_err(|e| format!("Backup: {e}"))?;
        let bytes = std::fs::read(&disk).map_err(|e| format!("Backup: {e}"))?;
        zip.write_all(&bytes).map_err(|e| format!("Backup: {e}"))?;
    }
    zip.finish().map_err(|e| format!("Backup: {e}"))?;
    Ok(())
}

pub fn import_zip(src: &Path) -> Result<(), String> {
    let dir = paths::data_dir();
    std::fs::create_dir_all(&dir).map_err(|e| format!("Importar: {e}"))?;
    let file = File::open(src).map_err(|e| format!("Não deu para abrir o backup: {e}"))?;
    let mut zip = ZipArchive::new(file).map_err(|e| format!("Arquivo de backup inválido: {e}"))?;
    let mut got = false;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).map_err(|e| format!("Importar: {e}"))?;
        let name = entry.name().replace('\\', "/");
        let dest = if name.ends_with(ZIP_INI) || name == ZIP_INI {
            dir.join(INI_NAME)
        } else if name.ends_with(ZIP_DB) || name == ZIP_DB {
            dir.join(DB_NAME)
        } else {
            continue;
        };
        let mut buf = Vec::new();
        entry
            .read_to_end(&mut buf)
            .map_err(|e| format!("Importar: {e}"))?;
        std::fs::write(&dest, buf).map_err(|e| format!("Importar: {e}"))?;
        got = true;
    }
    if !got {
        return Err("O arquivo não contém config nem banco do expansor.".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn zip_roundtrip_nomes() {
        let dir = std::env::temp_dir().join("b-snip-zip-test");
        let _ = std::fs::create_dir_all(&dir);
        let zip_path = dir.join("t.zip");
        let mut z = ZipWriter::new(File::create(&zip_path).unwrap());
        let opts = SimpleFileOptions::default();
        z.start_file(ZIP_INI, opts).unwrap();
        z.write_all(b"[General]\r\nWelcomeShown=1\r\n").unwrap();
        z.finish().unwrap();

        let mut zip = ZipArchive::new(File::open(&zip_path).unwrap()).unwrap();
        assert!(zip.len() >= 1);
        let names: Vec<String> = (0..zip.len())
            .map(|i| zip.by_index(i).unwrap().name().to_string())
            .collect();
        assert!(names.iter().any(|n| n.ends_with(ZIP_INI)));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
