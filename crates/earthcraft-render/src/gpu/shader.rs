//! Embed shared rendering constants into the portable WGSL shader.
use crate::tuning;
use earthcraft_core::{EARTH_RADIUS_M, VERTICAL_FOV_HALF_TAN};

pub(super) fn source() -> String {
    let color = |v: glam::Vec4| format!("vec3<f32>({:?}, {:?}, {:?})", v.x, v.y, v.z);
    let mut source = include_str!("shaders/overview.wgsl")
        .replace("__MISSING__", &color(tuning::MISSING_TILE_COLOR))
        .replace("__RIM__", &color(tuning::RIM_COLOR));
    for (name, value) in [
        ("FOV", VERTICAL_FOV_HALF_TAN),
        ("RADIUS", EARTH_RADIUS_M),
        ("MERCATOR", tuning::WEB_MERCATOR_MAX_LATITUDE_DEG),
        ("LOD_BIAS", tuning::LOD_SHARPNESS_BIAS),
        ("LOD_LAT_COS", tuning::MIN_LOD_LATITUDE_COSINE),
        ("LOD_INC_COS", tuning::MIN_LOD_INCIDENCE_COSINE),
        ("MIN_BLEND", tuning::MIN_ZOOM_BLEND_FRACTION as f64),
        ("TWILIGHT_OFFSET", tuning::TWILIGHT_OFFSET),
        ("TWILIGHT_WIDTH", tuning::TWILIGHT_WIDTH),
        ("NIGHT_BRIGHTNESS", tuning::NIGHT_BRIGHTNESS as f64),
        ("DAY_RANGE", tuning::DAY_BRIGHTNESS_RANGE as f64),
        ("NIGHT_BLUE", tuning::NIGHT_BLUE_TINT as f64),
        ("RIM_BLEND", tuning::RIM_BLEND as f64),
    ] {
        source = source.replace(&format!("__{name}__"), &format!("{:?}", value as f32));
    }
    source
}
