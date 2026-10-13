pub use fluent_bundle::FluentValue;

// ───────────────────────────── Languages ─────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Lang {
    En,
    PtBr,
}
impl Lang {
    pub const ALL: [Lang; 2] = [Lang::En, Lang::PtBr];
    pub fn code(self) -> &'static str {
        match self {
            Lang::En => "en",
            Lang::PtBr => "pt-BR",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Lang::En => "English",
            Lang::PtBr => "Português (Brasil)",
        }
    }
    /// An embedded, cached label with English fallback.
    pub fn text(self, id: &str) -> &str {
        super::localization::text(self, id)
    }
    /// Format a whole translated message; use string arguments for exact legacy numbers.
    pub fn format(self, id: &str, args: &[(&str, FluentValue<'_>)]) -> String {
        super::localization::format(self, id, args)
    }
}

/// Lower-cases and strips common Portuguese/Latin accents, for name de-duplication.
pub fn fold_name(s: &str) -> String {
    s.chars()
        .map(|c| c.to_lowercase().next().unwrap_or(c))
        .map(|c| match c {
            'á' | 'à' | 'â' | 'ã' | 'ä' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'í' | 'ì' | 'î' | 'ï' => 'i',
            'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o',
            'ú' | 'ù' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            c => c,
        })
        .filter(|c| c.is_alphanumeric())
        .collect()
}
