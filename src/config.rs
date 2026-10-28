use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub const INI_NAME: &str = "ClipboardExpander.ini";
#[allow(dead_code)]
pub const MAX_QUEUE: usize = 100;
pub const MIN_EXPANDER_ROWS: usize = 3;
pub const MAX_EXPANDER_ROWS: usize = 100;

#[derive(Clone, Copy, Debug, Default)]
pub struct WindowPos {
    pub queue: Option<(f32, f32)>,
    pub transform: Option<(f32, f32)>,
    pub config: Option<(f32, f32)>,
}

#[derive(Clone, Copy, Debug)]
pub struct Features {
    pub copy_queue: bool,
    pub expander: bool,
    pub transform: bool,
    pub loop_on: bool,
    pub sound: bool,
    pub clear_on_last: bool,
}

impl Default for Features {
    fn default() -> Self {
        Self {
            copy_queue: true,
            expander: true,
            transform: true,
            loop_on: true,
            sound: true,
            clear_on_last: false,
        }
    }
}

pub struct AppConfig {
    pub slots: Vec<String>,
    pub triggers: Vec<String>,
    pub window_pos: WindowPos,
    pub welcome_shown: bool,
    pub features: Features,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            slots: vec![String::new(); MIN_EXPANDER_ROWS],
            triggers: vec![String::new(); MIN_EXPANDER_ROWS],
            window_pos: WindowPos::default(),
            welcome_shown: false,
            features: Features::default(),
        }
    }
}

pub fn ini_path() -> PathBuf {
    crate::paths::ini_path()
}

pub fn load(path: &Path) -> AppConfig {
    let text = std::fs::read_to_string(path).unwrap_or_default();
    let map = parse_ini(&text);
    let mut cfg = AppConfig::default();

    let (slots, triggers) = load_expander_rows(&map);
    cfg.slots = slots;
    cfg.triggers = triggers;

    cfg.window_pos.queue = read_xy(&map, "Queue");
    cfg.window_pos.transform = read_xy(&map, "Transform");
    cfg.window_pos.config = read_xy(&map, "Config");

    cfg.welcome_shown = map
        .get(&("General".into(), "WelcomeShown".into()))
        .map(|v| v != "0" && !v.is_empty())
        .unwrap_or(false);

    cfg.features.copy_queue = read_bool(&map, "Features", "CopyQueue", true);
    cfg.features.expander = read_bool(&map, "Features", "Expander", true);
    cfg.features.transform = read_bool(&map, "Features", "Transform", true);
    cfg.features.loop_on = read_bool(&map, "Features", "Loop", true);
    cfg.features.sound = read_bool(&map, "Features", "Sound", true);
    cfg.features.clear_on_last = read_bool(&map, "Features", "ClearOnLast", false);
    if cfg.features.loop_on {
        cfg.features.clear_on_last = false;
    }

    cfg
}

fn load_expander_rows(map: &HashMap<(String, String), String>) -> (Vec<String>, Vec<String>) {
    let mut max_n = 0u32;
    for ((sec, key), _) in map {
        if sec != "Slots" && sec != "Triggers" {
            continue;
        }
        if let Ok(n) = key.parse::<u32>() {
            let n = if n == 0 { 10 } else { n };
            if (1..=MAX_EXPANDER_ROWS as u32).contains(&n) {
                max_n = max_n.max(n);
            }
        }
    }

    if max_n == 0 {
        return (
            vec![String::new(); MIN_EXPANDER_ROWS],
            vec![String::new(); MIN_EXPANDER_ROWS],
        );
    }

    let mut slots = Vec::with_capacity(max_n as usize);
    let mut trigs = Vec::with_capacity(max_n as usize);
    for n in 1..=max_n {
        let key = expander_ini_key(n, map);
        slots.push(
            map.get(&("Slots".into(), key.clone()))
                .cloned()
                .unwrap_or_default(),
        );
        trigs.push(
            map.get(&("Triggers".into(), key))
                .cloned()
                .unwrap_or_default(),
        );
    }

    while slots.len() > MIN_EXPANDER_ROWS
        && is_unused_expander_row(
            slots.last().map(|s| s.as_str()).unwrap_or(""),
            trigs.last().map(|s| s.as_str()).unwrap_or(""),
        )
    {
        slots.pop();
        trigs.pop();
    }
    while slots.len() < MIN_EXPANDER_ROWS {
        slots.push(String::new());
        trigs.push(String::new());
    }
    (slots, trigs)
}

/// Linha vazia ou leftover do expansor antigo (`@wbc1`…`@wbc0` sem texto).
fn is_unused_expander_row(slot: &str, trig: &str) -> bool {
    if !slot.trim().is_empty() {
        return false;
    }
    let t = trig.trim();
    if t.is_empty() {
        return true;
    }
    t.starts_with("@wbc") && t.chars().skip(4).all(|c| c.is_ascii_digit())
}

/// INI antigo: o 10º item era a chave `0`. O novo usa `10`.
fn expander_ini_key(n: u32, map: &HashMap<(String, String), String>) -> String {
    if n == 10 {
        let ten = "10".to_string();
        if map.contains_key(&("Slots".into(), ten.clone()))
            || map.contains_key(&("Triggers".into(), ten.clone()))
        {
            return ten;
        }
        if map.contains_key(&("Slots".into(), "0".into()))
            || map.contains_key(&("Triggers".into(), "0".into()))
        {
            return "0".into();
        }
    }
    n.to_string()
}

pub fn save_all(
    path: &Path,
    state_slots: &[String],
    state_triggers: &[String],
    pos: WindowPos,
    welcome_shown: bool,
    features: Features,
) {
    let n = state_slots.len().max(state_triggers.len()).min(MAX_EXPANDER_ROWS);
    let mut out = String::new();
    out.push_str("[General]\r\n");
    out.push_str(&format!("WelcomeShown={}\r\n\r\n", if welcome_shown { "1" } else { "0" }));

    out.push_str("[WindowPositions]\r\n");
    write_xy(&mut out, "Queue", pos.queue);
    write_xy(&mut out, "Transform", pos.transform);
    write_xy(&mut out, "Config", pos.config);
    out.push('\n');

    out.push_str("[Slots]\r\n");
    for i in 0..n {
        let val = state_slots.get(i).map(|s| s.as_str()).unwrap_or("");
        out.push_str(&format!("{}={}\r\n", i + 1, escape_ini(val)));
    }
    out.push('\n');

    out.push_str("[Triggers]\r\n");
    for i in 0..n {
        let val = state_triggers.get(i).map(|s| s.as_str()).unwrap_or("");
        out.push_str(&format!("{}={}\r\n", i + 1, escape_ini(val)));
    }
    out.push('\n');

    out.push_str("[Features]\r\n");
    out.push_str(&format!("CopyQueue={}\r\n", if features.copy_queue { "1" } else { "0" }));
    out.push_str(&format!("Expander={}\r\n", if features.expander { "1" } else { "0" }));
    out.push_str(&format!("Transform={}\r\n", if features.transform { "1" } else { "0" }));
    out.push_str(&format!("Loop={}\r\n", if features.loop_on { "1" } else { "0" }));
    out.push_str(&format!("Sound={}\r\n", if features.sound { "1" } else { "0" }));
    out.push_str(&format!(
        "ClearOnLast={}\r\n",
        if features.clear_on_last { "1" } else { "0" }
    ));

    let _ = std::fs::write(path, out);
}

pub fn mark_welcome_shown(path: &Path) {
    let mut cfg = load(path);
    cfg.welcome_shown = true;
    save_all(
        path,
        &cfg.slots,
        &cfg.triggers,
        cfg.window_pos,
        true,
        cfg.features,
    );
}

fn read_bool(map: &HashMap<(String, String), String>, section: &str, key: &str, default: bool) -> bool {
    map.get(&(section.into(), key.into()))
        .map(|v| v != "0" && !v.is_empty())
        .unwrap_or(default)
}

fn read_xy(map: &HashMap<(String, String), String>, prefix: &str) -> Option<(f32, f32)> {
    let x = map.get(&("WindowPositions".into(), format!("{prefix}X")))?;
    let y = map.get(&("WindowPositions".into(), format!("{prefix}Y")))?;
    let x: f32 = x.parse().ok()?;
    let y: f32 = y.parse().ok()?;
    Some((x, y))
}

fn write_xy(out: &mut String, prefix: &str, pos: Option<(f32, f32)>) {
    if let Some((x, y)) = pos {
        out.push_str(&format!("{prefix}X={}\r\n", x.round() as i32));
        out.push_str(&format!("{prefix}Y={}\r\n", y.round() as i32));
    }
}

fn escape_ini(s: &str) -> String {
    s.replace('\r', "").replace('\n', " ")
}

fn parse_ini(text: &str) -> HashMap<(String, String), String> {
    let mut map = HashMap::new();
    let mut section = String::new();
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with(';') || line.starts_with('#') {
            continue;
        }
        if let Some(inner) = line.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
            section = inner.trim().to_string();
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            map.insert((section.clone(), k.trim().to_string()), v.to_string());
        }
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn parse_ini_sections() {
        let map = parse_ini("[Features]\r\nCopyQueue=1\r\nLoop=0\r\n");
        assert_eq!(
            map.get(&("Features".into(), "CopyQueue".into())).unwrap(),
            "1"
        );
        assert!(!read_bool(&map, "Features", "Loop", true));
        assert!(read_bool(&map, "Features", "CopyQueue", false));
    }

    #[test]
    fn trim_wbc_placeholder_rows() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("babc-ini-{stamp}.ini"));
        fs::write(
            &path,
            "[Slots]\r\n1=mail@x.com\r\n2=\r\n3=\r\n[Triggers]\r\n1=!mail\r\n2=@wbc2\r\n3=@wbc3\r\n",
        )
        .unwrap();
        let cfg = load(&path);
        let _ = fs::remove_file(&path);
        assert_eq!(cfg.triggers.len(), MIN_EXPANDER_ROWS);
        assert_eq!(cfg.triggers[0], "!mail");
        assert_eq!(cfg.slots[0], "mail@x.com");
    }

    #[test]
    fn loop_ligado_desliga_clear_on_last() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("babc-ini-xor-{stamp}.ini"));
        fs::write(
            &path,
            "[Features]\r\nLoop=1\r\nClearOnLast=1\r\n",
        )
        .unwrap();
        let cfg = load(&path);
        let _ = fs::remove_file(&path);
        assert!(cfg.features.loop_on);
        assert!(!cfg.features.clear_on_last);
    }
}
