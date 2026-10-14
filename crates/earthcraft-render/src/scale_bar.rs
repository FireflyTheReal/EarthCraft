//! Scale-bar distance selection, independent of drawing.
use earthcraft_core::{units::DistanceUnits, METERS_PER_KM, VERTICAL_FOV_HALF_TAN};
const MAX_ALTITUDE_KM: f64 = 3000.0;
const TARGET_WIDTH_POINTS: f64 = 120.0;
pub struct ScaleBar {
    pub distance_m: f64,
    pub width_points: f32,
}
impl ScaleBar {
    pub fn label(&self) -> String {
        if self.distance_m >= METERS_PER_KM {
            format!("{} km", self.distance_m / METERS_PER_KM)
        } else {
            format!("{} m", self.distance_m)
        }
    }
}
pub fn scale_bar(altitude_km: f64, height_points: f32) -> Option<ScaleBar> {
    scale_bar_with_units(altitude_km, height_points, DistanceUnits::Metric)
}
/// Select a round displayed distance in the requested unit system.
pub fn scale_bar_with_units(
    altitude_km: f64,
    height_points: f32,
    units: DistanceUnits,
) -> Option<ScaleBar> {
    if altitude_km >= MAX_ALTITUDE_KM {
        return None;
    }
    let mpp = altitude_km * METERS_PER_KM * 2.0 * VERTICAL_FOV_HALF_TAN / height_points as f64;
    let unit_m = units.scale_unit_meters(mpp * TARGET_WIDTH_POINTS);
    let target = mpp * TARGET_WIDTH_POINTS / unit_m;
    let pow = 10f64.powf(target.log10().floor());
    let distance_m = [5.0, 2.0, 1.0]
        .iter()
        .map(|m| m * pow)
        .find(|v| *v <= target)
        .unwrap_or(pow);
    Some(ScaleBar {
        distance_m: distance_m * unit_m,
        width_points: (distance_m * unit_m / mpp) as f32,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn imperial_scales_use_round_distances_at_bounded_screen_widths() {
        for altitude in [0.15, 1.0, 10.0, 100.0, 1000.0, 2999.0] {
            let scale = scale_bar_with_units(altitude, 800.0, DistanceUnits::Imperial).unwrap();
            assert!(scale.width_points > 0.0 && scale.width_points <= TARGET_WIDTH_POINTS as f32);
            let label = DistanceUnits::Imperial.scale_label(scale.distance_m);
            assert!(label.ends_with(" ft") || label.ends_with(" mi"), "{label}");
        }
    }
    #[test]
    fn familiar_distance_and_visibility_cutoff() {
        let scale = scale_bar(1.0, 800.0).unwrap();
        assert_eq!(scale.distance_m, 100.0);
        assert_eq!(scale.label(), "100 m");
        assert!(scale.width_points > 0.0 && scale.width_points <= 120.0);
        assert!(scale_bar(3000.0, 800.0).is_none());
    }
}
