pub const QUEUE_ITEM_MAX: usize = 2 * 1024 * 1024;
pub const QUEUE_TOTAL_MAX: usize = 32 * 1024 * 1024;
pub const EXPANDER_ITEM_MAX: usize = 8 * 1024 * 1024;
pub const WARN_RATIO: f32 = 0.80;
pub const MAX_QUEUE_ITEMS: usize = 100;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ClipPayload {
    pub unicode: String,
    pub html: Option<Vec<u8>>,
    pub rtf: Option<Vec<u8>>,
    pub png: Option<Vec<u8>>,
    pub dib: Option<Vec<u8>>,
}

impl ClipPayload {
    pub fn from_unicode(text: impl Into<String>) -> Self {
        Self {
            unicode: text.into(),
            ..Default::default()
        }
    }

    pub fn byte_size(&self) -> usize {
        self.unicode.len()
            + self.html.as_ref().map(|v| v.len()).unwrap_or(0)
            + self.rtf.as_ref().map(|v| v.len()).unwrap_or(0)
            + self.png.as_ref().map(|v| v.len()).unwrap_or(0)
            + self.dib.as_ref().map(|v| v.len()).unwrap_or(0)
    }

    pub fn is_empty(&self) -> bool {
        self.unicode.trim().is_empty()
            && self.html.as_ref().is_none_or(|v| v.is_empty())
            && self.rtf.as_ref().is_none_or(|v| v.is_empty())
            && self.png.as_ref().is_none_or(|v| v.is_empty())
            && self.dib.as_ref().is_none_or(|v| v.is_empty())
    }

    pub fn has_rich(&self) -> bool {
        self.html.as_ref().is_some_and(|v| !v.is_empty())
            || self.rtf.as_ref().is_some_and(|v| !v.is_empty())
            || self.png.as_ref().is_some_and(|v| !v.is_empty())
            || self.dib.as_ref().is_some_and(|v| !v.is_empty())
    }

    pub fn same_as(&self, other: &Self) -> bool {
        self.unicode == other.unicode
            && self.html == other.html
            && self.rtf == other.rtf
            && self.png == other.png
            && self.dib == other.dib
    }

    pub fn flags_label(&self) -> String {
        let mut parts = Vec::new();
        if self.html.as_ref().is_some_and(|v| !v.is_empty()) {
            parts.push("HTML");
        }
        if self.rtf.as_ref().is_some_and(|v| !v.is_empty()) {
            parts.push("RTF");
        }
        if self.png.as_ref().is_some_and(|v| !v.is_empty())
            || self.dib.as_ref().is_some_and(|v| !v.is_empty())
        {
            parts.push("imagem");
        }
        if parts.is_empty() {
            "texto".into()
        } else {
            parts.join(" + ")
        }
    }

    /// Unicode que parece HTML vira CF_HTML, para o expansor não gravar só as tags.
    pub fn promote_html(&mut self) {
        if self.html.as_ref().is_none_or(|v| v.is_empty())
            && crate::text::looks_like_html(&self.unicode)
        {
            self.html = Some(crate::text::wrap_cf_html(&self.unicode));
        }
    }

    pub fn mapped(&self, map: impl Fn(&str) -> String + Copy) -> Self {
        let mut out = self.clone();
        out.unicode = map(&self.unicode);
        if let Some(html) = &self.html {
            out.html = Some(crate::text::map_cf_html(html, map));
        }
        if let Some(rtf) = &self.rtf {
            let s = String::from_utf8_lossy(rtf);
            out.rtf = Some(crate::text::map_rtf_visible(&s, map).into_bytes());
        }
        out
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SizeDecision {
    Accept,
    AcceptWarn,
    ItemTooBig,
    TotalTooBig,
    QueueFull,
}

pub fn check_queue_size(item_bytes: usize, total_after: usize, count_after: usize) -> SizeDecision {
    if count_after > MAX_QUEUE_ITEMS {
        return SizeDecision::QueueFull;
    }
    if item_bytes > QUEUE_ITEM_MAX {
        return SizeDecision::ItemTooBig;
    }
    if total_after > QUEUE_TOTAL_MAX {
        return SizeDecision::TotalTooBig;
    }
    let warn_item = (QUEUE_ITEM_MAX as f32 * WARN_RATIO) as usize;
    let warn_total = (QUEUE_TOTAL_MAX as f32 * WARN_RATIO) as usize;
    if item_bytes >= warn_item || total_after >= warn_total {
        SizeDecision::AcceptWarn
    } else {
        SizeDecision::Accept
    }
}

pub fn check_expander_size(item_bytes: usize) -> SizeDecision {
    if item_bytes > EXPANDER_ITEM_MAX {
        SizeDecision::ItemTooBig
    } else if item_bytes >= (EXPANDER_ITEM_MAX as f32 * WARN_RATIO) as usize {
        SizeDecision::AcceptWarn
    } else {
        SizeDecision::Accept
    }
}

pub fn format_mb(bytes: usize) -> String {
    let mb = bytes as f64 / (1024.0 * 1024.0);
    if mb < 0.1 {
        format!("{:.0} KB", bytes as f64 / 1024.0)
    } else {
        format!("{mb:.1} MB")
    }
}

pub const QUEUE_FULL_MSG: &str = "Fila cheia (100 itens). Nada novo entra.\nLimpe a lista: botão direito no ícone → Limpar fila.\nAtalho: Ctrl+Alt+R.";

pub fn queue_item_limit_msg(bytes: usize) -> String {
    format!(
        "Item grande demais ({}/{}).",
        format_mb(bytes),
        format_mb(QUEUE_ITEM_MAX)
    )
}

pub fn queue_total_limit_msg(bytes: usize) -> String {
    format!(
        "Fila grande demais ({}/{}).",
        format_mb(bytes),
        format_mb(QUEUE_TOTAL_MAX)
    )
}

pub fn queue_warn_msg(bytes: usize, total: usize) -> String {
    format!(
        "Item grande: {} / {}.\nFila: {} / {}.",
        format_mb(bytes),
        format_mb(QUEUE_ITEM_MAX),
        format_mb(total),
        format_mb(QUEUE_TOTAL_MAX)
    )
}

pub fn expander_limit_msg(bytes: usize) -> String {
    format!(
        "Conteúdo grande demais ({}/{}).",
        format_mb(bytes),
        format_mb(EXPANDER_ITEM_MAX)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn byte_size_soma_formatos() {
        let mut p = ClipPayload::from_unicode("abc");
        p.html = Some(vec![0; 10]);
        p.png = Some(vec![0; 7]);
        assert_eq!(p.byte_size(), 3 + 10 + 7);
        assert!(p.has_rich());
    }

    #[test]
    fn fila_rejeita_item_acima_de_2mb() {
        let d = check_queue_size(QUEUE_ITEM_MAX + 1, QUEUE_ITEM_MAX + 1, 1);
        assert_eq!(d, SizeDecision::ItemTooBig);
    }

    #[test]
    fn fila_rejeita_total_acima_de_32mb() {
        let d = check_queue_size(1024, QUEUE_TOTAL_MAX + 1, 10);
        assert_eq!(d, SizeDecision::TotalTooBig);
    }

    #[test]
    fn fila_rejeita_101_itens() {
        assert_eq!(
            check_queue_size(10, 100, 101),
            SizeDecision::QueueFull
        );
        assert!(QUEUE_FULL_MSG.contains("Limpe a lista"));
        assert!(QUEUE_FULL_MSG.contains("Ctrl+Alt+R"));
    }

    #[test]
    fn fila_avisa_em_80_por_cento() {
        let item = (QUEUE_ITEM_MAX as f32 * 0.85) as usize;
        assert_eq!(check_queue_size(item, item, 1), SizeDecision::AcceptWarn);
        assert_eq!(check_queue_size(100, 100, 1), SizeDecision::Accept);
    }

    #[test]
    fn expansor_teto_8mb() {
        assert_eq!(
            check_expander_size(EXPANDER_ITEM_MAX + 1),
            SizeDecision::ItemTooBig
        );
        assert_eq!(check_expander_size(100), SizeDecision::Accept);
    }

    #[test]
    fn empty_sem_unicode_nem_blob() {
        assert!(ClipPayload::default().is_empty());
        assert!(!ClipPayload::from_unicode(" x ").is_empty());
    }

    #[test]
    fn promote_html_quando_unicode_e_tag() {
        let mut p = ClipPayload::from_unicode("<p><strong>Oi</strong></p>");
        p.promote_html();
        assert!(p.has_rich());
        let html = String::from_utf8(p.html.unwrap()).unwrap();
        assert!(html.contains("StartHTML:"));
        assert!(html.contains("<strong>Oi</strong>"));
    }

    #[test]
    fn mapped_sobe_so_texto_html() {
        let mut p = ClipPayload::from_unicode("abc");
        p.html = Some(crate::text::wrap_cf_html("<p>abc</p>"));
        let up = p.mapped(crate::text::str_upper);
        assert_eq!(up.unicode, "ABC");
        let html = String::from_utf8(up.html.unwrap()).unwrap();
        assert!(html.contains("<p>ABC</p>"));
        assert!(html.contains("StartHTML:"));
    }
}
