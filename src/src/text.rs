#[allow(dead_code)]
pub fn normalize_text(s: &str) -> String {
    let mut t = s.replace("\r\n", "\n").replace('\r', "\n");
    while t.contains("\n\n") {
        t = t.replace("\n\n", "\n");
    }
    t.trim().to_string()
}

/// Equivalente ao StrTitle do AutoHotkey: primeira letra de cada palavra em maiúscula.
pub fn str_title(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut new_word = true;
    for ch in s.chars() {
        if ch.is_whitespace() {
            out.push(ch);
            new_word = true;
        } else if new_word {
            for up in ch.to_uppercase() {
                out.push(up);
            }
            new_word = false;
        } else {
            for lo in ch.to_lowercase() {
                out.push(lo);
            }
        }
    }
    out
}

pub fn str_upper(s: &str) -> String {
    s.to_uppercase()
}

/// Passos de cursor para reselecionar o texto colado.
/// `\r\n` no clipboard é uma quebra só no editor (VS Code, Word, etc.).
#[allow(dead_code)]
pub fn caret_units(s: &str) -> usize {
    s.replace("\r\n", "\n").replace('\r', "\n").chars().count()
}

pub fn visible_text(unicode: &str) -> String {
    if looks_like_html(unicode) {
        strip_tags(unicode)
    } else {
        unicode.to_string()
    }
}

pub fn strip_tags(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_tag = false;
    for ch in s.chars() {
        if ch == '<' {
            in_tag = true;
            continue;
        }
        if ch == '>' {
            in_tag = false;
            continue;
        }
        if !in_tag {
            out.push(ch);
        }
    }
    out
}

pub fn str_lower(s: &str) -> String {
    s.to_lowercase()
}

pub fn looks_like_html(s: &str) -> bool {
    let t = s.trim_start();
    t.starts_with('<')
        && (t.contains("<html")
            || t.contains("<HTML")
            || t.contains("<p")
            || t.contains("<P")
            || t.contains("<div")
            || t.contains("<span")
            || t.contains("<strong")
            || t.contains("<br")
            || t.contains("<img")
            || t.contains("<table"))
}

/// Envelope CF_HTML do Windows para um fragmento (tags ou texto).
pub fn wrap_cf_html(fragment: &str) -> Vec<u8> {
    let inner = format!("<html><body><!--StartFragment-->{fragment}<!--EndFragment--></body></html>");
    let dummy = format!(
        "Version:0.9\r\nStartHTML:{:010}\r\nEndHTML:{:010}\r\nStartFragment:{:010}\r\nEndFragment:{:010}\r\n",
        0, 0, 0, 0
    );
    let start_html = dummy.len();
    let frag_token = "<!--StartFragment-->";
    let end_token = "<!--EndFragment-->";
    let start_fragment = start_html + inner.find(frag_token).unwrap_or(0) + frag_token.len();
    let end_fragment = start_html + inner.find(end_token).unwrap_or(inner.len());
    let end_html = start_html + inner.len();
    let header = format!(
        "Version:0.9\r\nStartHTML:{:010}\r\nEndHTML:{:010}\r\nStartFragment:{:010}\r\nEndFragment:{:010}\r\n",
        start_html, end_html, start_fragment, end_fragment
    );
    let mut out = String::with_capacity(header.len() + inner.len());
    out.push_str(&header);
    out.push_str(&inner);
    out.into_bytes()
}

/// Aplica `map` só no texto visível (fora de tags e de &entidades;).
pub fn map_html_visible(s: &str, map: impl Fn(&str) -> String) -> String {
    let mut out = String::with_capacity(s.len());
    let mut buf = String::new();
    let mut in_tag = false;
    let mut in_entity = false;
    for ch in s.chars() {
        if in_tag {
            out.push(ch);
            if ch == '>' {
                in_tag = false;
            }
            continue;
        }
        if in_entity {
            out.push(ch);
            if ch == ';' || ch.is_whitespace() {
                in_entity = false;
            }
            continue;
        }
        if ch == '<' {
            flush_buf(&mut buf, &mut out, &map);
            in_tag = true;
            out.push(ch);
            continue;
        }
        if ch == '&' {
            flush_buf(&mut buf, &mut out, &map);
            in_entity = true;
            out.push(ch);
            continue;
        }
        buf.push(ch);
    }
    flush_buf(&mut buf, &mut out, &map);
    out
}

fn flush_buf(buf: &mut String, out: &mut String, map: &impl Fn(&str) -> String) {
    if !buf.is_empty() {
        out.push_str(&map(buf));
        buf.clear();
    }
}

pub fn map_cf_html(bytes: &[u8], map: impl Fn(&str) -> String) -> Vec<u8> {
    let s = String::from_utf8_lossy(bytes);
    let idx = s
        .find("<html")
        .or_else(|| s.find("<HTML"))
        .or_else(|| s.find('<'));
    if let Some(i) = idx {
        let (head, rest) = s.split_at(i);
        format!("{head}{}", map_html_visible(rest, map)).into_bytes()
    } else {
        map_html_visible(&s, map).into_bytes()
    }
}

/// Texto RTF fora de `{ } \` — o bastante para maiúscula sem comer comandos.
pub fn map_rtf_visible(s: &str, map: impl Fn(&str) -> String) -> String {
    let mut out = String::with_capacity(s.len());
    let mut buf = String::new();
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let ch = chars[i];
        if ch == '\\' {
            flush_buf(&mut buf, &mut out, &map);
            out.push(ch);
            i += 1;
            while i < chars.len() && (chars[i].is_ascii_alphabetic() || chars[i] == '\'') {
                out.push(chars[i]);
                i += 1;
            }
            if i < chars.len() && chars[i] == ' ' {
                out.push(' ');
                i += 1;
            }
            continue;
        }
        if ch == '{' || ch == '}' {
            flush_buf(&mut buf, &mut out, &map);
            out.push(ch);
            i += 1;
            continue;
        }
        buf.push(ch);
        i += 1;
    }
    flush_buf(&mut buf, &mut out, &map);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn html_visivel_nao_mexe_tag() {
        let s = "<p><strong>abc</strong></p>";
        let out = map_html_visible(s, str_upper);
        assert_eq!(out, "<p><strong>ABC</strong></p>");
    }

    #[test]
    fn html_pula_entidade() {
        let s = "a&nbsp;b";
        assert_eq!(map_html_visible(s, str_upper), "A&nbsp;B");
    }

    #[test]
    fn looks_like_html_detecta_p() {
        assert!(looks_like_html("  <p><strong>Oi</strong></p>"));
        assert!(!looks_like_html("olá mundo"));
    }

    #[test]
    fn wrap_cf_html_tem_offsets() {
        let b = wrap_cf_html("<p>x</p>");
        let s = String::from_utf8(b).unwrap();
        assert!(s.contains("StartHTML:"));
        assert!(s.contains("<!--StartFragment--><p>x</p><!--EndFragment-->"));
    }

    #[test]
    fn visible_text_tira_tags() {
        assert_eq!(visible_text("<p>Oi <strong>mundo</strong></p>"), "Oi mundo");
        assert_eq!(visible_text("olá"), "olá");
    }
}

pub fn today_br() -> String {
    use windows::Win32::System::SystemInformation::GetLocalTime;
    let st = unsafe { GetLocalTime() };
    format!("{:02}/{:02}/{:04}", st.wDay, st.wMonth, st.wYear)
}
