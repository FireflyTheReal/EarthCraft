//! Pixel-rendering parameters
// Tile resolution, projection limits, and level-of-detail tuning.
pub(super) const TILE_SIZE_PX: f64 = 256.0;
pub(super) const WEB_MERCATOR_MAX_LATITUDE_DEG: f64 = 85.051_128_78;
pub(super) const MIN_LOD_LATITUDE_COSINE: f64 = 0.02;
pub(super) const MIN_LOD_INCIDENCE_COSINE: f64 = 0.2;
pub(super) const LOD_SHARPNESS_BIAS: f64 = 0.5;
pub(super) const MIN_ZOOM_BLEND_FRACTION: f32 = 0.01;
pub(super) const MISSING_TILE_COLOR: [f32; 3] = [0.04, 0.09, 0.18];
// Grid visibility and day/night transition.
pub(super) const GRID_LINE_WIDTH_PX: f64 = 0.8;
pub(super) const GRID_COLOR: [f32; 3] = [1.0, 1.0, 0.6];
pub(super) const GRID_BLEND: f32 = 0.55;
pub(super) const TWILIGHT_OFFSET: f64 = 0.12;
pub(super) const TWILIGHT_WIDTH: f64 = 0.30;
pub(super) const NIGHT_BRIGHTNESS: f32 = 0.10;
pub(super) const DAY_BRIGHTNESS_RANGE: f32 = 0.90;
pub(super) const NIGHT_BLUE_TINT: f32 = 0.015;
// Surface rim and atmospheric shell.
pub(super) const RIM_FALLOFF_POWER: i32 = 4;
pub(super) const RIM_BLEND: f32 = 0.75;
pub(super) const RIM_COLOR: [f32; 3] = [0.35, 0.60, 1.0];
pub(super) const ATMOSPHERE_RADIUS_RATIO: f64 = 1.07;
pub(super) const ATMOSPHERE_HEIGHT_RATIO: f64 = 0.07;
pub(super) const ATMOSPHERE_COLOR: [f32; 3] = [0.30, 0.55, 1.0];
pub(super) const ATMOSPHERE_BLEND: f32 = 0.9;
// Deterministic star field and 8-bit RGBA output.
pub(super) const SPACE_COLOR: [f32; 3] = [0.0, 0.0, 0.015];
pub(super) const STAR_GRID_RESOLUTION: f64 = 900.0;
pub(super) const STAR_OCCURRENCE_MODULUS: u64 = 1400;
pub(super) const STAR_MIN_BRIGHTNESS: f32 = 0.45;
pub(super) const STAR_BRIGHTNESS_DIVISOR: f32 = 480.0;
pub(super) const STAR_BRIGHTNESS_SHIFT: u32 = 20;
pub(super) const STAR_BRIGHTNESS_MASK: u64 = 255;
pub(super) const RGBA_CHANNELS: usize = 4;
pub(super) const COLOR_CHANNEL_MAX: f32 = 255.0;
pub(super) const OPAQUE_ALPHA: u8 = 255;
