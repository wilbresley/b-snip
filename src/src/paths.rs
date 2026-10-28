use std::path::{Path, PathBuf};

use crate::config::INI_NAME;

pub const PORTABLE_FLAG: &str = "portable.txt";
pub const DB_NAME: &str = "expander.db";
pub const SENTINEL_EXPANDER: &str = "open_expander.flag";
pub const APP_NAME: &str = "b-snip";
const APP_FOLDER: &str = APP_NAME;
const LEGACY_APP_FOLDER: &str = "b-abcABC";

pub fn exe_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."))
}

pub fn user_data_dir() -> PathBuf {
    user_data_dir_from(std::env::var_os("LOCALAPPDATA").map(PathBuf::from))
}

pub fn user_data_dir_from(local_appdata: Option<PathBuf>) -> PathBuf {
    local_appdata
        .unwrap_or_else(|| PathBuf::from("."))
        .join(APP_FOLDER)
}

pub fn portable_flag_path() -> PathBuf {
    exe_dir().join(PORTABLE_FLAG)
}

pub fn is_portable() -> bool {
    portable_flag_path().is_file()
}

/// Pasta de dados conforme o modo. Cria o diretório se faltar.
pub fn data_dir() -> PathBuf {
    let dir = resolve_data_dir(&exe_dir(), &user_data_dir(), is_portable());
    let _ = std::fs::create_dir_all(&dir);
    dir
}

pub fn resolve_data_dir(exe_dir: &Path, user_dir: &Path, portable: bool) -> PathBuf {
    if portable {
        exe_dir.to_path_buf()
    } else {
        user_dir.to_path_buf()
    }
}

pub fn ini_path() -> PathBuf {
    data_dir().join(INI_NAME)
}

pub fn db_path() -> PathBuf {
    data_dir().join(DB_NAME)
}

pub fn legacy_ini_beside_exe() -> PathBuf {
    exe_dir().join(INI_NAME)
}

/// Primeira execução no perfil: herda a pasta antiga `b-abcABC` e o INI ao lado do EXE.
pub fn migrate_legacy_ini_if_needed() {
    if is_portable() {
        return;
    }
    migrate_legacy_user_folder();
    let dest = ini_path();
    if dest.exists() {
        return;
    }
    let src = legacy_ini_beside_exe();
    if src.exists() && src != dest {
        if let Some(parent) = dest.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::copy(&src, &dest);
    }
}

fn migrate_legacy_user_folder() {
    let dest = user_data_dir();
    if dest.join(INI_NAME).is_file() || dest.join(DB_NAME).is_file() {
        return;
    }
    let Some(base) = std::env::var_os("LOCALAPPDATA").map(PathBuf::from) else {
        return;
    };
    let src = base.join(LEGACY_APP_FOLDER);
    if src.is_dir() && src != dest {
        let _ = copy_data_dir(&src, &dest);
    }
}

pub fn set_portable(on: bool) -> Result<(), String> {
    let flag = portable_flag_path();
    if on {
        std::fs::write(&flag, "1\r\n").map_err(|_| {
            "Não deu para criar o arquivo ao lado do programa (falta permissão). Os dados continuam na pasta deste usuário.".to_string()
        })
    } else if flag.exists() {
        std::fs::remove_file(&flag).map_err(|_| {
            "Não deu para remover o modo portátil (falta permissão).".to_string()
        })
    } else {
        Ok(())
    }
}

pub fn copy_data_dir(from: &Path, to: &Path) -> Result<(), String> {
    if from == to {
        return Ok(());
    }
    std::fs::create_dir_all(to).map_err(|e| format!("Não deu para criar a pasta destino: {e}"))?;
    for name in [
        INI_NAME,
        DB_NAME,
        "expander.db-wal",
        "expander.db-shm",
        "expander.db-journal",
    ] {
        let src = from.join(name);
        if src.is_file() {
            std::fs::copy(&src, to.join(name))
                .map_err(|e| format!("Não deu para copiar {name}: {e}"))?;
        }
    }
    Ok(())
}

pub fn write_open_expander_sentinel() {
    let _ = std::fs::write(data_dir().join(SENTINEL_EXPANDER), "1");
}

pub fn take_open_expander_sentinel() -> bool {
    let p = data_dir().join(SENTINEL_EXPANDER);
    if p.exists() {
        let _ = std::fs::remove_file(&p);
        true
    } else {
        false
    }
}

#[allow(dead_code)]
pub fn data_files_exist(dir: &Path) -> bool {
    dir.join(INI_NAME).is_file() || dir.join(DB_NAME).is_file()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn portable_grava_ao_lado_do_exe() {
        let exe = PathBuf::from(r"D:\app");
        let user = PathBuf::from(r"C:\Users\a\AppData\Local\b-snip");
        assert_eq!(resolve_data_dir(&exe, &user, true), exe);
        assert_eq!(resolve_data_dir(&exe, &user, false), user);
    }

    #[test]
    fn user_dir_usa_localappdata() {
        let base = PathBuf::from(r"C:\Users\wbc\AppData\Local");
        assert_eq!(
            user_data_dir_from(Some(base)),
            PathBuf::from(r"C:\Users\wbc\AppData\Local").join(APP_NAME)
        );
        assert_eq!(APP_FOLDER, APP_NAME);
    }
}
