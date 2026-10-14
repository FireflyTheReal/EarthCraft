//! True-north headings and initial great-circle bearings, shared by frontends.
use crate::{language::Lang, math::ll_to_v3};
use glam::DVec3;

const UNDEFINED_BEARING_EPSILON: f64 = 1e-12;
const COMPASS_SECTOR_DEG: f64 = 45.0;
const DIRECTION_IDS: [&str; 8] = [
    "compass-north",
    "compass-northeast",
    "compass-east",
    "compass-southeast",
    "compass-south",
    "compass-southwest",
    "compass-west",
    "compass-northwest",
];

/// Degrees clockwise from true north in [0, 360).
pub fn normalize_heading(heading_deg: f64) -> f64 {
    let heading = heading_deg.rem_euclid(360.0);
    if heading == 0.0 { 0.0 } else { heading }
}

pub fn heading_text(heading_deg: f64, lang: Lang) -> String {
    // Normalize after rounding so the readout never says 360° or disagrees at a sector edge.
    let heading = normalize_heading(heading_deg.round());
    let sector = ((heading / COMPASS_SECTOR_DEG).round() as usize) % DIRECTION_IDS.len();
    lang.format(
        "compass-heading",
        &[
            ("degrees", format!("{heading:.0}").into()),
            ("direction", lang.text(DIRECTION_IDS[sector]).into()),
        ],
    )
}

/// Initial great-circle bearing from `start` to `end`, in latitude/longitude degrees.
/// Coincident/antipodal points and a start at either pole have no unique bearing.
pub fn initial_bearing(start: (f64, f64), end: (f64, f64)) -> Option<f64> {
    if ![start.0, start.1, end.0, end.1]
        .iter()
        .all(|v| v.is_finite())
        || start.0.abs() >= 90.0
        || end.0.abs() > 90.0
    {
        return None;
    }
    let (lat, lon) = (start.0.to_radians(), start.1.to_radians());
    let east = DVec3::new(-lon.sin(), lon.cos(), 0.0);
    let north = DVec3::new(-lat.sin() * lon.cos(), -lat.sin() * lon.sin(), lat.cos());
    let target = ll_to_v3(end.0, end.1);
    let (e, n) = (target.dot(east), target.dot(north));
    if e.hypot(n) <= UNDEFINED_BEARING_EPSILON {
        return None;
    }
    Some(normalize_heading(e.atan2(n).to_degrees()))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bearings_cover_cardinals_dateline_and_undefined_routes() {
        for (start, end, expected) in [
            ((0.0, 0.0), (10.0, 0.0), 0.0),
            ((0.0, 0.0), (0.0, 10.0), 90.0),
            ((0.0, 0.0), (-10.0, 0.0), 180.0),
            ((0.0, 0.0), (0.0, -10.0), 270.0),
            ((0.0, 179.0), (0.0, -179.0), 90.0),
            ((0.0, -179.0), (0.0, 179.0), 270.0),
        ] {
            assert!((initial_bearing(start, end).unwrap() - expected).abs() < 1e-9);
        }
        for (start, end) in [
            ((0.0, 0.0), (0.0, 0.0)),
            ((0.0, 0.0), (0.0, 180.0)),
            ((90.0, 0.0), (80.0, 0.0)),
            ((f64::NAN, 0.0), (0.0, 0.0)),
        ] {
            assert!(initial_bearing(start, end).is_none());
        }
    }
    #[test]
    fn headings_wrap_round_and_localize() {
        for (heading, expected) in [
            (0.0, "0° N"),
            (90.0, "90° E"),
            (-90.0, "270° W"),
            (359.9, "0° N"),
            (405.0, "45° NE"),
        ] {
            assert_eq!(heading_text(heading, Lang::En), expected);
        }
        assert_eq!(heading_text(270.0, Lang::PtBr), "270° O");
        assert!(!normalize_heading(-0.0).is_sign_negative());
    }
}
