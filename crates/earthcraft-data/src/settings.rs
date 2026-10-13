use crate::paths::{cache_dir, default_out_dir};
use earthcraft_core::language::Lang;
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

#[derive(Clone, PartialEq)]
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
impl Settings {
    pub fn load() -> Self {
        let mut st = Settings::default();
        if let Some(v) = std::fs::read(settings_file())
            .ok()
            .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
        {
            if let Some(x) = v["screenshot_dir"].as_str().filter(|x| !x.is_empty()) {
                st.screenshot_dir = x.into();
            }
            if let Some(x) = v["kml_dir"].as_str().filter(|x| !x.is_empty()) {
                st.kml_dir = x.into();
            }
            if let Some(x) = v["ui_scale"].as_f64() {
                st.ui_scale = (x as f32).clamp(MIN_UI_SCALE, MAX_UI_SCALE);
            }
            if let Some(x) = v["map_scale"].as_f64() {
                st.map_scale = (x as f32).clamp(MIN_MAP_SCALE, MAX_MAP_SCALE);
            }
        }
        st
    }
    pub fn save(&self) {
        let _ = std::fs::create_dir_all(cache_dir());
        let v = serde_json::json!({
            "screenshot_dir": self.screenshot_dir,
            "kml_dir": self.kml_dir,
            "ui_scale": self.ui_scale,
            "map_scale": self.map_scale,
        });
        let _ = std::fs::write(settings_file(), v.to_string());
    }
}
