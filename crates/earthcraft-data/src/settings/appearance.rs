//! Persisted UI appearance and installed-font family preferences.
use serde::Serialize;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ThemePreference {
    System,
    #[default]
    Dark,
    Light,
}
impl ThemePreference {
    pub const ALL: [Self; 3] = [Self::System, Self::Dark, Self::Light];
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct AppearancePreferences {
    pub theme: ThemePreference,
    pub ui_font: String,
    pub map_font: String,
}
impl AppearancePreferences {
    pub(super) fn apply(&mut self, value: &serde_json::Value) {
        if let Some(theme) = value.get("theme").and_then(|v| v.as_str()) {
            self.theme = match theme {
                "system" => ThemePreference::System,
                "dark" => ThemePreference::Dark,
                "light" => ThemePreference::Light,
                _ => self.theme,
            };
        }
        for (key, target) in [
            ("ui_font", &mut self.ui_font),
            ("map_font", &mut self.map_font),
        ] {
            if let Some(font) = value.get(key).and_then(|v| v.as_str()) {
                *target = font.to_owned();
            }
        }
    }
}
