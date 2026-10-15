//! Stored desktop layout; invalid fields retain their last valid preference.
use serde::Serialize;

pub const DEFAULT_SIDEBAR_WIDTH_POINTS: f32 = 240.0;
pub const MIN_SIDEBAR_WIDTH_POINTS: f32 = 210.0;
pub const MAX_SIDEBAR_WIDTH_POINTS: f32 = 600.0;
pub const DEFAULT_PINNED_PLACES_HEIGHT_POINTS: f32 = 300.0;
pub const MIN_PINNED_PLACES_HEIGHT_POINTS: f32 = 180.0;
pub const MAX_PINNED_PLACES_HEIGHT_POINTS: f32 = 1000.0;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SidebarPanel {
    #[default]
    Explore,
    Layers,
    Places,
}
impl SidebarPanel {
    pub const ALL: [Self; 3] = [Self::Explore, Self::Layers, Self::Places];
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct SidebarLayout {
    pub panel: SidebarPanel,
    pub open: bool,
    pub places_pinned: bool,
    pub width_points: f32,
    pub pinned_places_height_points: f32,
}
impl Default for SidebarLayout {
    fn default() -> Self {
        Self {
            panel: SidebarPanel::Explore,
            open: true,
            places_pinned: false,
            width_points: DEFAULT_SIDEBAR_WIDTH_POINTS,
            pinned_places_height_points: DEFAULT_PINNED_PLACES_HEIGHT_POINTS,
        }
    }
}
impl SidebarLayout {
    pub(super) fn apply_json(&mut self, value: &serde_json::Value) {
        if let Some(panel) = value.get("panel").and_then(|v| v.as_str()) {
            self.panel = match panel {
                "explore" => SidebarPanel::Explore,
                "layers" => SidebarPanel::Layers,
                "places" => SidebarPanel::Places,
                _ => self.panel,
            };
        }
        for (name, target) in [
            ("open", &mut self.open),
            ("places_pinned", &mut self.places_pinned),
        ] {
            if let Some(value) = value.get(name).and_then(|v| v.as_bool()) {
                *target = value;
            }
        }
        for (name, target, min, max) in [
            (
                "width_points",
                &mut self.width_points,
                MIN_SIDEBAR_WIDTH_POINTS,
                MAX_SIDEBAR_WIDTH_POINTS,
            ),
            (
                "pinned_places_height_points",
                &mut self.pinned_places_height_points,
                MIN_PINNED_PLACES_HEIGHT_POINTS,
                MAX_PINNED_PLACES_HEIGHT_POINTS,
            ),
        ] {
            if let Some(value) = value
                .get(name)
                .and_then(|v| v.as_f64())
                .filter(|v| v.is_finite())
            {
                *target = value.clamp(f64::from(min), f64::from(max)) as f32;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::Settings;

    #[test]
    fn sidebar_layout_roundtrips_and_preserves_old_or_invalid_preferences() {
        for panel in SidebarPanel::ALL {
            let layout = SidebarLayout {
                panel,
                open: false,
                places_pinned: true,
                width_points: 345.0,
                pinned_places_height_points: 420.0,
            };
            let settings = Settings {
                sidebar: layout.clone(),
                ..Settings::default()
            };
            let mut restored = Settings::default();
            restored
                .apply_json(&serde_json::to_vec(&settings).unwrap())
                .unwrap();
            assert_eq!(restored.sidebar, layout);
            for bytes in [br#"{}"#.as_slice(), br#"{"sidebar":null}"#, br#"{"sidebar":42}"#, br#"{"sidebar":{"panel":"future","open":"yes","rail_visible":42,"places_pinned":[],"width_points":"wide","pinned_places_height_points":null}}"#] {
                restored.apply_json(bytes).unwrap();
                assert_eq!(restored.sidebar, layout);
            }
        }
        let mut restored = Settings::default();
        restored
            .apply_json(br#"{"sidebar":{"width_points":1e200,"pinned_places_height_points":-10}}"#)
            .unwrap();
        assert_eq!(restored.sidebar.width_points, MAX_SIDEBAR_WIDTH_POINTS);
        assert_eq!(
            restored.sidebar.pinned_places_height_points,
            MIN_PINNED_PLACES_HEIGHT_POINTS
        );
        restored.apply_json(br#"{"sidebar":{"panel":"layers","open":false,"rail_visible":"bad","places_pinned":true}}"#).unwrap();
        assert_eq!(restored.sidebar.panel, SidebarPanel::Layers);
        assert!(!restored.sidebar.open);
        assert!(
            serde_json::to_value(&restored.sidebar)
                .unwrap()
                .get("rail_visible")
                .is_none()
        );
        assert!(restored.sidebar.places_pinned);
    }
}
