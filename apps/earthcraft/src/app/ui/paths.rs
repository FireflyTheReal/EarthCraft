//! Compact path labels shared by file statuses and sidebar folders.
pub(super) fn file_path_input(
    ui: &mut eframe::egui::Ui,
    path: &mut String,
) -> eframe::egui::Response {
    let home = if cfg!(unix) {
        std::env::var("HOME").ok()
    } else {
        None
    };
    let mut display = editable_path(path, home.as_deref());
    let response = ui.add(
        eframe::egui::TextEdit::singleline(&mut display)
            .id(eframe::egui::Id::new("places-files-path"))
            .desired_width(f32::INFINITY),
    );
    if response.changed() {
        *path = expanded_path(&display, home.as_deref());
    }
    response.on_hover_text(path.as_str())
}

fn editable_path(path: &str, home: Option<&str>) -> String {
    if let Some(home) = home.filter(|home| !home.is_empty()) {
        let home = home.trim_end_matches('/');
        if path == home {
            return "~".into();
        }
        if let Some(rest) = path
            .strip_prefix(home)
            .and_then(|rest| rest.strip_prefix('/'))
        {
            return format!("~/{rest}");
        }
    }
    path.into()
}

fn expanded_path(path: &str, home: Option<&str>) -> String {
    if let Some(home) = home.filter(|home| !home.is_empty()) {
        if path == "~" {
            return home.into();
        }
        if let Some(rest) = path.strip_prefix("~/") {
            return format!("{}/{rest}", home.trim_end_matches('/'));
        }
    }
    path.into()
}

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
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn editable_home_paths_preserve_full_paths_and_only_expand_current_home() {
        for home in ["/Users/ana", "/home/ana"] {
            let path = format!("{home}/Downloads/São Paulo.kmz");
            let display = editable_path(&path, Some(home));
            assert_eq!(display, "~/Downloads/São Paulo.kmz");
            assert_eq!(expanded_path(&display, Some(home)), path);
            assert_eq!(
                expanded_path("~/Downloads/edited.kml", Some(home)),
                format!("{home}/Downloads/edited.kml")
            );
            assert_eq!(
                expanded_path("~other/places.kml", Some(home)),
                "~other/places.kml"
            );
            assert_eq!(
                editable_path(&format!("{home}bel/places.kml"), Some(home)),
                format!("{home}bel/places.kml")
            );
            assert_eq!(
                expanded_path(&editable_path(home, Some(home)), Some(home)),
                home
            );
        }
        assert_eq!(
            editable_path("/tmp/places.kml", Some("/home/ana")),
            "/tmp/places.kml"
        );
        assert_eq!(editable_path("places.kml", None), "places.kml");
        assert_eq!(
            expanded_path("~/Downloads/places.kml", None),
            "~/Downloads/places.kml"
        );
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
