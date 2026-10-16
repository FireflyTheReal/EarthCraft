//! Earth-fixed directions to the NASA map's ICRF/J2000 frame.
use glam::{DMat3, DVec2, DVec3};
use std::f64::consts::{PI, TAU};
const UNIX_EPOCH_JULIAN_DATE: f64 = 2_440_587.5;
const SECONDS_PER_DAY: f64 = 86_400.0;
#[derive(Clone, Copy, Debug)]
pub struct CelestialFrame {
    celestial_from_earth_fixed: DMat3,
}
#[derive(Clone, Copy, Debug)]
pub struct CelestialTimeError(pub i32);
impl std::fmt::Display for CelestialTimeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "Celestial UTC conversion failed (SOFA status {})",
            self.0
        )
    }
}
impl std::error::Error for CelestialTimeError {}
impl CelestialFrame {
    /// UTC is used as UT1 and polar motion is zero: suitable for a 4K backdrop,
    /// not an astrometric observation. TT uses SOFA's embedded leap-second table.
    pub fn at_unix(unix_secs: f64) -> Result<Self, CelestialTimeError> {
        if !unix_secs.is_finite() {
            return Err(CelestialTimeError(-1));
        }
        let days = unix_secs / SECONDS_PER_DAY;
        let (tai1, tai2) =
            sofars::ts::utctai(UNIX_EPOCH_JULIAN_DATE, days).map_err(CelestialTimeError)?;
        let (tt1, tt2) = sofars::ts::taitt(tai1, tai2).map_err(CelestialTimeError)?;
        Ok(Self::from_times(tt1, tt2, UNIX_EPOCH_JULIAN_DATE, days))
    }
    fn from_times(tt1: f64, tt2: f64, ut1: f64, ut2: f64) -> Self {
        let rows = sofars::pnp::c2t06a(tt1, tt2, ut1, ut2, 0.0, 0.0);
        // SOFA returns rows of celestial-to-terrestrial. Interpreting those rows
        // as glam columns gives its transpose, terrestrial-to-celestial.
        Self {
            celestial_from_earth_fixed: DMat3::from_cols_array_2d(&rows),
        }
    }
    pub fn direction(self, earth_fixed: DVec3) -> DVec3 {
        self.celestial_from_earth_fixed * earth_fixed
    }
}
/// The published map centers RA 0h and increases RA leftward; declination is up.
pub fn equatorial_uv(direction: DVec3) -> DVec2 {
    let ra = direction.y.atan2(direction.x);
    let dec = direction.z.atan2(direction.x.hypot(direction.y));
    DVec2::new(
        (0.5 - ra / TAU).rem_euclid(1.0),
        (0.5 - dec / PI).clamp(0.0, 1.0),
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn map_ra_is_leftward_and_poles_are_not_mirrored() {
        assert_eq!(equatorial_uv(DVec3::X), DVec2::new(0.5, 0.5));
        assert_eq!(equatorial_uv(DVec3::Y), DVec2::new(0.25, 0.5));
        assert_eq!(equatorial_uv(-DVec3::Y), DVec2::new(0.75, 0.5));
        assert_eq!(equatorial_uv(DVec3::Z).y, 0.0);
        assert_eq!(equatorial_uv(-DVec3::Z).y, 1.0);
        let left = equatorial_uv(DVec3::new(-1.0, 1e-12, 0.0));
        let right = equatorial_uv(DVec3::new(-1.0, -1e-12, 0.0));
        assert!((left.x + 1.0 - right.x).abs() < 1e-10);
    }
    #[test]
    fn sofa_reference_orientation_and_inertial_directions() {
        // SOFA c2t06a reference epoch, TT=UT1=MJD 53736, no polar motion.
        let frame = CelestialFrame::from_times(2_400_000.5, 53736.0, 2_400_000.5, 53736.0);
        let x = frame.direction(DVec3::X);
        assert!((x.x - -0.18103321283058973).abs() < 1e-8);
        assert!((x.y - 0.9834769806938592).abs() < 1e-8);
        let now = CelestialFrame::at_unix(1_760_000_000.0).unwrap();
        let later = CelestialFrame::at_unix(1_760_021_600.0).unwrap();
        assert!(now.direction(DVec3::X).dot(later.direction(DVec3::X)).abs() < 0.01);
        let inertial = DVec3::new(0.2, 0.5, 0.8).normalize();
        let earth_ray = now.celestial_from_earth_fixed.transpose() * inertial;
        assert!(now.direction(earth_ray).abs_diff_eq(inertial, 1e-12));
        assert!(CelestialFrame::at_unix(f64::NAN).is_err());
    }
}
