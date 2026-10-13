use fluent_bundle::FluentValue;

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

/// Canonically decomposes and removes combining accents for name matching.
/// This is a comparison key, not a transliteration or a displayed place name.
pub fn fold_name(s: &str) -> String {
    use unicode_normalization::{char::is_combining_mark, UnicodeNormalization};
    s.nfd()
        .flat_map(char::to_lowercase)
        .filter(|c| !is_combining_mark(*c) && c.is_alphanumeric())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn names_match_across_canonical_accents_and_case_without_transliterating_scripts() {
        for (name, expected) in [
            ("São Paulo", "saopaulo"),
            ("SA\u{303}O PAULO", "saopaulo"),
            ("ÁÀÂÃÄ ÉÈÊË ÍÌÎÏ ÓÒÔÕÖ ÚÙÛÜ Ç", "aaaaaeeeeiiiiooooouuuuc"),
            ("İstanbul", "istanbul"),
            ("Thành phố Huế", "thanhphohue"),
            ("Αθήνα", "αθηνα"),
            ("東京", "東京"),
            ("Москва", "москва"),
        ] {
            assert_eq!(fold_name(name), expected, "{name}");
        }
        assert_eq!(fold_name("Huế"), fold_name("Hue\u{302}\u{301}"));
    }
}
