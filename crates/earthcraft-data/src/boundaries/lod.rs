//! Detail level for the world overlays, chosen from camera altitude.
//!
//! Coarse detail loads first and is replaced when the camera moves closer.
//! Altitudes are the camera's distance above the surface in kilometres.

/// Whole-globe views keep the bundled low detail.
pub(super) const LOW_DETAIL_MAX_ALTITUDE_KM: f64 = 10_000.0;
/// Continental views use medium detail below this altitude.
pub(super) const MEDIUM_DETAIL_MAX_ALTITUDE_KM: f64 = 1_500.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum DetailLevel {
    Low = 1,
    Medium = 2,
    High = 3,
}

impl DetailLevel {
    pub(super) fn for_altitude(altitude_km: f64) -> Self {
        if !altitude_km.is_finite() || altitude_km > LOW_DETAIL_MAX_ALTITUDE_KM {
            Self::Low
        } else if altitude_km > MEDIUM_DETAIL_MAX_ALTITUDE_KM {
            Self::Medium
        } else {
            Self::High
        }
    }

    pub(super) fn from_code(code: u8) -> Self {
        match code {
            3 => Self::High,
            2 => Self::Medium,
            _ => Self::Low,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn altitude_selects_a_monotonic_detail_level() {
        assert_eq!(DetailLevel::for_altitude(20_000.0), DetailLevel::Low);
        assert_eq!(DetailLevel::for_altitude(10_001.0), DetailLevel::Low);
        assert_eq!(DetailLevel::for_altitude(10_000.0), DetailLevel::Medium);
        assert_eq!(DetailLevel::for_altitude(1_501.0), DetailLevel::Medium);
        assert_eq!(DetailLevel::for_altitude(1_500.0), DetailLevel::High);
        assert_eq!(DetailLevel::for_altitude(5.0), DetailLevel::High);
        assert_eq!(DetailLevel::for_altitude(f64::NAN), DetailLevel::Low);
        assert!(DetailLevel::Low < DetailLevel::Medium);
        assert!(DetailLevel::Medium < DetailLevel::High);
    }

    #[test]
    fn codes_round_trip() {
        for level in [DetailLevel::Low, DetailLevel::Medium, DetailLevel::High] {
            assert_eq!(DetailLevel::from_code(level as u8), level);
        }
        assert_eq!(DetailLevel::from_code(0), DetailLevel::Low);
        assert_eq!(DetailLevel::from_code(9), DetailLevel::Low);
    }
}
