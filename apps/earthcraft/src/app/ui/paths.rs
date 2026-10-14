//! Compact path labels shared by file statuses and sidebar folders.
/// Retain recognizable trailing components and hide machine-specific parents.
pub(super) fn short_folder(path: &str) -> String {
    let home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" });
    compact_path(
        path,
        home.as_deref().and_then(|home| home.to_str()),
        cfg!(windows),
    )
}
fn compact_path(path: &str, home: Option<&str>, windows: bool) -> String {
    if let Some(home) = home.filter(|home| !home.is_empty()) {
        let normalized_path = if windows {
            path.replace('\\', "/")
        } else {
            path.to_string()
        };
        let normalized_home = if windows {
            home.replace('\\', "/")
        } else {
            home.to_string()
        };
        let home = normalized_home.trim_end_matches('/');
        let prefix = normalized_path.get(..home.len());
        if prefix.is_some_and(|prefix| {
            if windows {
                prefix.eq_ignore_ascii_case(home)
            } else {
                prefix == home
            }
        }) {
            let rest = &normalized_path[home.len()..];
            if rest.is_empty() || rest.starts_with('/') {
                return if windows {
                    format!("%USERPROFILE%{}", rest.replace('/', "\\"))
                } else {
                    format!("~{rest}")
                };
            }
        }
    }
    const VISIBLE_PATH_COMPONENTS: usize = 2;
    // Accept both separators so imported Windows paths also display sensibly.
    let components: Vec<_> = path
        .split(['/', '\\'])
        .filter(|part| !part.is_empty())
        .collect();
    if components.len() <= VISIBLE_PATH_COMPONENTS {
        return path.to_string();
    }
    format!(
        ".../{}",
        components[components.len() - VISIBLE_PATH_COMPONENTS..].join("/")
    )
}
/// On narrow toolbars keep the filename end visible, rather than clipping it away.
pub(super) fn fit_tail(ui: &eframe::egui::Ui, text: &str, width_points: f32) -> String {
    use eframe::egui::{Color32, TextStyle};
    let font = TextStyle::Body.resolve(ui.style());
    let fits = |text: &str| {
        ui.fonts_mut(|fonts| {
            fonts
                .layout_no_wrap(text.to_owned(), font.clone(), Color32::WHITE)
                .size()
                .x
        }) <= width_points
    };
    if fits(text) {
        return text.to_owned();
    }
    let boundaries: Vec<_> = text
        .char_indices()
        .map(|(index, _)| index)
        .chain(std::iter::once(text.len()))
        .collect();
    let mut low = 0;
    let mut high = boundaries.len() - 1;
    while low < high {
        let mid = (low + high) / 2;
        if fits(&format!("...{}", &text[boundaries[mid]..])) {
            high = mid;
        } else {
            low = mid + 1;
        }
    }
    format!("...{}", &text[boundaries[low]..])
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn narrow_labels_keep_the_filename_end_and_fit_the_available_width() {
        use eframe::egui;
        let ctx = egui::Context::default();
        let _ = crate::app::test_ui::run(&ctx, egui::RawInput::default(), |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                let text = "1 placemark imported. — ~/Downloads/東京.kml";
                assert_eq!(fit_tail(ui, text, 1000.0), text);
                let fitted = fit_tail(ui, text, 110.0);
                assert!(fitted.starts_with("..."));
                assert!(fitted.ends_with("東京.kml"));
                let width = ui.fonts_mut(|fonts| {
                    fonts
                        .layout_no_wrap(
                            fitted,
                            egui::TextStyle::Body.resolve(ui.style()),
                            egui::Color32::WHITE,
                        )
                        .size()
                        .x
                });
                assert!(width <= 110.0);
            });
        });
    }
    #[test]
    fn home_abbreviation_requires_a_complete_directory_prefix() {
        assert_eq!(
            compact_path("/Users/ana/Pictures/EarthCraft", Some("/Users/ana"), false),
            "~/Pictures/EarthCraft"
        );
        assert_eq!(compact_path("/Users/ana", Some("/Users/ana"), false), "~");
        assert_eq!(
            compact_path(
                "/Users/anabel/Pictures/EarthCraft",
                Some("/Users/ana"),
                false
            ),
            ".../Pictures/EarthCraft"
        );
        assert_eq!(
            compact_path(
                r"C:\Users\Ana\Pictures\EarthCraft",
                Some(r"c:\users\ana"),
                true
            ),
            r"%USERPROFILE%\Pictures\EarthCraft"
        );
    }
    #[test]
    fn preserves_filename_and_unicode_without_expanding_short_paths() {
        assert_eq!(
            compact_path("/Users/name/Downloads/São Paulo.kml", None, false),
            ".../Downloads/São Paulo.kml"
        );
        assert_eq!(
            compact_path(r"C:\Users\name\Downloads\places.kml", None, true),
            ".../Downloads/places.kml"
        );
        assert_eq!(short_folder("places.kml"), "places.kml");
        assert_eq!(short_folder(""), "");
    }
}
