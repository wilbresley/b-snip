use rusqlite::{params, Connection, OptionalExtension};

use crate::config::MIN_EXPANDER_ROWS;
use crate::paths;
use crate::payload::ClipPayload;

#[allow(dead_code)]
pub struct SnippetMeta {
    pub trigger: String,
    pub name: String,
    pub bytes: i64,
    pub has_rich: bool,
}

fn open() -> Result<Connection, String> {
    let path = paths::db_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let conn = Connection::open(&path).map_err(|e| e.to_string())?;
    conn.execute_batch(
        "PRAGMA journal_mode=WAL;
         CREATE TABLE IF NOT EXISTS snippets (
            sort INTEGER PRIMARY KEY,
            trigger TEXT NOT NULL DEFAULT '',
            name TEXT NOT NULL DEFAULT '',
            unicode TEXT NOT NULL DEFAULT '',
            html BLOB,
            rtf BLOB,
            png BLOB,
            dib BLOB,
            bytes INTEGER NOT NULL DEFAULT 0
         );",
    )
    .map_err(|e| e.to_string())?;
    Ok(conn)
}

pub fn load_meta() -> Vec<SnippetMeta> {
    let Ok(conn) = open() else {
        return Vec::new();
    };
    let Ok(mut stmt) = conn.prepare(
        "SELECT trigger, name, bytes,
                (html IS NOT NULL AND length(html)>0)
             OR (rtf IS NOT NULL AND length(rtf)>0)
             OR (png IS NOT NULL AND length(png)>0)
             OR (dib IS NOT NULL AND length(dib)>0)
         FROM snippets ORDER BY sort",
    ) else {
        return Vec::new();
    };
    stmt.query_map([], |row| {
        Ok(SnippetMeta {
            trigger: row.get(0)?,
            name: row.get(1)?,
            bytes: row.get(2)?,
            has_rich: row.get::<_, i64>(3)? != 0,
        })
    })
    .ok()
    .map(|rows| rows.filter_map(|r| r.ok()).collect())
    .unwrap_or_default()
}

pub fn get_payload(sort: i64) -> Option<ClipPayload> {
    let conn = open().ok()?;
    conn.query_row(
        "SELECT unicode, html, rtf, png, dib FROM snippets WHERE sort=?1",
        params![sort],
        |row| {
            Ok(ClipPayload {
                unicode: row.get::<_, String>(0).unwrap_or_default(),
                html: blob(row, 1),
                rtf: blob(row, 2),
                png: blob(row, 3),
                dib: blob(row, 4),
            })
        },
    )
    .optional()
    .ok()
    .flatten()
}

fn blob(row: &rusqlite::Row<'_>, idx: usize) -> Option<Vec<u8>> {
    row.get::<_, Option<Vec<u8>>>(idx)
        .ok()
        .flatten()
        .filter(|v| !v.is_empty())
}

pub fn set_payload(sort: i64, trigger: &str, name: &str, payload: &ClipPayload) -> Result<(), String> {
    let conn = open()?;
    let bytes = payload.byte_size() as i64;
    conn.execute(
        "INSERT INTO snippets (sort, trigger, name, unicode, html, rtf, png, dib, bytes)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)
         ON CONFLICT(sort) DO UPDATE SET
            trigger=excluded.trigger,
            name=excluded.name,
            unicode=excluded.unicode,
            html=excluded.html,
            rtf=excluded.rtf,
            png=excluded.png,
            dib=excluded.dib,
            bytes=excluded.bytes",
        params![
            sort,
            trigger,
            name,
            payload.unicode,
            payload.html.as_deref(),
            payload.rtf.as_deref(),
            payload.png.as_deref(),
            payload.dib.as_deref(),
            bytes
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// Atualiza gatilho/nome e alinha a quantidade de linhas. Blobs das linhas que
/// continuam existentes são preservados.
pub fn sync_rows(triggers: &[String], names: &[String]) -> Result<(), String> {
    let n = triggers.len().max(names.len());
    let conn = open()?;
    conn.execute("BEGIN", []).map_err(|e| e.to_string())?;
    let _ = conn.execute("DELETE FROM snippets WHERE sort >= ?1", params![n as i64]);
    for i in 0..n {
        let trig = triggers.get(i).map(|s| s.as_str()).unwrap_or("");
        let name = names.get(i).map(|s| s.as_str()).unwrap_or("");
        conn.execute(
            "INSERT INTO snippets (sort, trigger, name, unicode, bytes)
             VALUES (?1,?2,?3,'',0)
             ON CONFLICT(sort) DO UPDATE SET trigger=excluded.trigger, name=excluded.name",
            params![i as i64, trig, name],
        )
        .map_err(|e| e.to_string())?;
    }
    conn.execute("COMMIT", []).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn replace_all(rows: &[(String, String, ClipPayload)]) -> Result<(), String> {
    let conn = open()?;
    conn.execute_batch("BEGIN; DELETE FROM snippets;")
        .map_err(|e| e.to_string())?;
    for (i, (trig, name, payload)) in rows.iter().enumerate() {
        let bytes = payload.byte_size() as i64;
        conn.execute(
            "INSERT INTO snippets (sort, trigger, name, unicode, html, rtf, png, dib, bytes)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
            params![
                i as i64,
                trig,
                name,
                payload.unicode,
                payload.html.as_deref(),
                payload.rtf.as_deref(),
                payload.png.as_deref(),
                payload.dib.as_deref(),
                bytes
            ],
        )
        .map_err(|e| e.to_string())?;
    }
    conn.execute("COMMIT", []).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn import_from_ini_if_empty(triggers: &[String], slots: &[String]) {
    if !load_meta().is_empty() {
        return;
    }
    let n = triggers.len().max(slots.len()).max(MIN_EXPANDER_ROWS);
    let mut t = triggers.to_vec();
    let mut s = slots.to_vec();
    t.resize(n, String::new());
    s.resize(n, String::new());
    let _ = sync_rows(&t, &s);
    for i in 0..n {
        if s[i].is_empty() {
            continue;
        }
        let p = ClipPayload::from_unicode(s[i].clone());
        let _ = set_payload(i as i64, &t[i], &s[i], &p);
    }
}

#[cfg(test)]
mod tests {
    use crate::payload::ClipPayload;

    fn isolated_db() {
        // Os testes de store usam o db_path real; aqui só validamos roundtrip
        // de payload em memória via ClipPayload.
        let mut p = ClipPayload::from_unicode("olá");
        p.html = Some(b"<b>ola</b>".to_vec());
        assert!(p.has_rich());
        assert!(p.byte_size() > 3);
    }

    #[test]
    fn payload_rico_nao_e_so_texto() {
        isolated_db();
    }
}
