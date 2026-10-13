use crate::paths::{cache_dir, default_out_dir};
use earthcraft_core::language::Lang;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
pub const MIN_UI_SCALE: f32 = 0.7;
pub const MAX_UI_SCALE: f32 = 2.5;
pub const MIN_MAP_SCALE: f32 = 0.6;
pub const MAX_MAP_SCALE: f32 = 3.0;

fn lang_file() -> PathBuf {
    cache_dir().join("lang.txt")
}
pub fn load_lang() -> Lang {
    match std::fs::read_to_string(lang_file())
        .ok()
        .as_deref()
        .map(str::trim)
    {
        Some("pt-BR") => Lang::PtBr,
        _ => Lang::En, // English is the default
    }
}
pub fn save_lang(l: Lang) {
    let _ = std::fs::create_dir_all(cache_dir());
    let _ = std::fs::write(lang_file(), l.code());
}

#[derive(Clone, PartialEq, Serialize)]
pub struct Settings {
    pub screenshot_dir: String,
    pub kml_dir: String,
    pub ui_scale: f32,  // text size of buttons/panels
    pub map_scale: f32, // text size of map labels
}
impl Default for Settings {
    fn default() -> Self {
        Settings {
            screenshot_dir: default_out_dir("Pictures"),
            kml_dir: default_out_dir("Documents"),
            ui_scale: 1.0,
            map_scale: 1.0,
        }
    }
}
fn settings_file() -> PathBuf {
    cache_dir().join("settings.json")
}
#[derive(Default, Deserialize)]
struct SettingsInput {
    #[serde(default, deserialize_with = "crate::json_fields::string")]
    screenshot_dir: Option<String>,
    #[serde(default, deserialize_with = "crate::json_fields::string")]
    kml_dir: Option<String>,
    #[serde(default, deserialize_with = "crate::json_fields::number")]
    ui_scale: Option<f64>,
    #[serde(default, deserialize_with = "crate::json_fields::number")]
    map_scale: Option<f64>,
}
impl Settings {
    fn apply_json(&mut self, bytes: &[u8]) -> Result<(), crate::Error> {
        let input: SettingsInput = serde_json::from_slice(bytes)?;
        if let Some(path) = input.screenshot_dir.filter(|s| !s.is_empty()) {
            self.screenshot_dir = path;
        }
        if let Some(path) = input.kml_dir.filter(|s| !s.is_empty()) {
            self.kml_dir = path;
        }
        if let Some(scale) = input.ui_scale {
            self.ui_scale = (scale as f32).clamp(MIN_UI_SCALE, MAX_UI_SCALE);
        }
        if let Some(scale) = input.map_scale {
            self.map_scale = (scale as f32).clamp(MIN_MAP_SCALE, MAX_MAP_SCALE);
        }
        Ok(())
    }
    pub fn load() -> Self {
        let mut settings = Self::default();
        if let Ok(bytes) = std::fs::read(settings_file()) {
            let _ = settings.apply_json(&bytes);
        }
        settings
    }
    pub fn save(&self) {
        if let Ok(bytes) = serde_json::to_vec(self) {
            let _ = std::fs::create_dir_all(cache_dir());
            let _ = std::fs::write(settings_file(), bytes);
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn partial_invalid_settings_preserve_valid_fields_and_clamping() {
        let mut settings = Settings {
            screenshot_dir: "pictures".into(),
            kml_dir: "documents".into(),
            ui_scale: 1.0,
            map_scale: 1.0,
        };
        settings
            .apply_json(
                br#"{"screenshot_dir":123,"kml_dir":"custom","ui_scale":9,"map_scale":"wrong"}"#,
            )
            .unwrap();
        assert_eq!(settings.screenshot_dir, "pictures");
        assert_eq!(settings.kml_dir, "custom");
        assert_eq!(settings.ui_scale, MAX_UI_SCALE);
        assert_eq!(settings.map_scale, 1.0);
        settings
            .apply_json(br#"{"kml_dir":"","ui_scale":-5,"map_scale":9}"#)
            .unwrap();
        assert_eq!(settings.kml_dir, "custom");
        assert_eq!(settings.ui_scale, MIN_UI_SCALE);
        assert_eq!(settings.map_scale, MAX_MAP_SCALE);
        let json: serde_json::Value =
            serde_json::from_slice(&serde_json::to_vec(&settings).unwrap()).unwrap();
        assert_eq!(json["kml_dir"], "custom");
    }
}
