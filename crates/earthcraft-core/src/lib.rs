//! Globe geometry, camera operations, language selection, and placemark values.
//! No network, filesystem, windowing, or widget dependencies.
pub mod camera;
pub mod language;
pub mod math;
pub mod placemark;
/// Mean Earth radius used by the original globe model, in kilometers.
pub const EARTH_RADIUS_KM: f64 = 6371.0088;
pub const METERS_PER_KM: f64 = 1000.0;
pub const EARTH_RADIUS_M: f64 = EARTH_RADIUS_KM * METERS_PER_KM;
/// Tangent of half the 45-degree vertical field of view: tan(22.5 degrees).
pub const VERTICAL_FOV_HALF_TAN: f64 = 0.414_213_562_373_095_1;
pub const MIN_CAMERA_ALTITUDE_KM: f64 = 0.15;
pub const MAX_CAMERA_ALTITUDE_KM: f64 = 40_000.0;
/// Existing approximation used for screen/grid distances; keep separate from search's 111.2 km/degree.
pub const METERS_PER_LATITUDE_DEGREE: f64 = 111_195.0;
/// Stop short of the poles so camera orientation remains well-defined.
pub const MAX_CAMERA_LATITUDE_DEG: f64 = 89.9;
