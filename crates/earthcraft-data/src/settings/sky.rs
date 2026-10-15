use serde::Serialize;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct SkyPreferences {
    pub enabled: bool,
    pub cinematic: bool,
}
impl Default for SkyPreferences {
    fn default() -> Self {
        Self {
            enabled: true,
            cinematic: false,
        }
    }
}
impl SkyPreferences {
    pub(super) fn apply(&mut self, value: &serde_json::Value) {
        if let Some(enabled) = value.get("enabled").and_then(|v| v.as_bool()) {
            self.enabled = enabled;
        }
        if let Some(cinematic) = value.get("cinematic").and_then(|v| v.as_bool()) {
            self.cinematic = cinematic;
        }
    }
}
