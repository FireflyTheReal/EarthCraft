// Overview geometry uses f32 only after the host checks the altitude/LOD range.
// Close views use the shared f64 planner. CPU repairs the exact sky/horizon.
struct View {
    camera: vec4<f32>,
    right: vec4<f32>, up: vec4<f32>, forward: vec4<f32>,
    sun: vec4<f32>,
    raster: vec4<u32>, // width, height, imagery, provider max zoom
    effects: vec4<u32>, // sunlight, atmosphere, reserved, reserved
    limits: vec4<u32>, // tracked max zoom, dependency count, table mask, unused
}
@group(0) @binding(0) var<uniform> view: View;
@group(0) @binding(1) var<storage, read_write> output: array<u32>;
@group(0) @binding(2) var atlas: texture_2d_array<f32>;
@group(0) @binding(3) var<storage, read> tiles: array<vec4<u32>>;
@group(0) @binding(4) var<storage, read_write> needs: array<atomic<u32>>;
const FOV_HALF_TAN: f32 = __FOV__;
const EARTH_RADIUS_M: f32 = __RADIUS__;
const MERCATOR_LIMIT_DEG: f32 = __MERCATOR__;
const TILE_SEAM_PRECISION_MARGIN: f32 = 0.0001;
const MERCATOR_EDGE_EPSILON: f32 = 0.00000011920928955078125;
const HORIZON_PRECISION_MARGIN: f32 = 0.01;
const PI: f32 = 3.141592653589793;
const MISSING: vec3<f32> = __MISSING__;
const RIM: vec3<f32> = __RIM__;
const LOD_BIAS: f32 = __LOD_BIAS__;
const LOD_LAT_COS: f32 = __LOD_LAT_COS__;
const LOD_INC_COS: f32 = __LOD_INC_COS__;
const MIN_BLEND: f32 = __MIN_BLEND__;
const TWILIGHT_OFFSET: f32 = __TWILIGHT_OFFSET__;
const TWILIGHT_WIDTH: f32 = __TWILIGHT_WIDTH__;
const NIGHT_BRIGHTNESS: f32 = __NIGHT_BRIGHTNESS__;
const DAY_RANGE: f32 = __DAY_RANGE__;
const NIGHT_BLUE: f32 = __NIGHT_BLUE__;
const RIM_BLEND: f32 = __RIM_BLEND__;
fn blend(a: vec3<f32>, b: vec3<f32>, t: f32) -> vec3<f32> { return a * (1.0 - t) + b * t; }
fn tile_hash(z: u32, x: u32, y: u32) -> u32 { return (z * 73856093u ^ x * 19349663u ^ y * 83492791u) & view.limits.z; }
fn find_tile(z: u32, x: u32, y: u32) -> i32 {
    var index = tile_hash(z, x, y);
    for(var probe = 0u; probe <= view.limits.z; probe++) {
        let tile = tiles[index];
        if tile.x == 0xffffffffu { return -1; }
        if all(tile.xyz == vec3<u32>(z,x,y)) { return i32(tile.w); }
        index = (index + 1u) & view.limits.z;
    }
    return -1;
}
fn sample_tile(uv: vec2<f32>, layer: i32) -> vec3<f32> {
    let dimensions = vec2<i32>(textureDimensions(atlas));
    let p = max(uv * vec2<f32>(dimensions) - vec2<f32>(0.5), vec2<f32>(0.0));
    let low = min(vec2<i32>(p), dimensions - vec2<i32>(1));
    let high = min(low + vec2<i32>(1), dimensions - vec2<i32>(1));
    let f = min(p - vec2<f32>(low), vec2<f32>(1.0));
    let a = textureLoad(atlas, low, layer, 0).rgb * 255.0;
    let b = textureLoad(atlas, vec2<i32>(high.x, low.y), layer, 0).rgb * 255.0;
    let c = textureLoad(atlas, vec2<i32>(low.x, high.y), layer, 0).rgb * 255.0;
    let d = textureLoad(atlas, high, layer, 0).rgb * 255.0;
    return blend(blend(a,b,f.x),blend(c,d,f.x),f.y) / 255.0;
}
fn chain(zoom: u32, uv: vec2<f32>, distance: f32) -> vec3<f32> {
    var z = i32(zoom);
    loop {
        let n = 1u << u32(z);
        // Preserve the CPU sampler's signed floor before saturating key coordinates.
        let coordinate = min(floor(uv * f32(n)), vec2<f32>(f32(n-1u)));
        let coord = vec2<u32>(max(coordinate,vec2<f32>(0.0)));
        // Track sampled keys as well as misses, so residency follows this view
        // instead of growing with the entire decoded cache.
        if u32(z) <= view.limits.x {
            let index = ((1u << (2u * u32(z))) - 1u) / 3u + coord.y * n + coord.x;
            let priority = bitcast<u32>(distance);
            if priority < atomicLoad(&needs[index]) { atomicMin(&needs[index],priority); }
        } else { atomicStore(&needs[view.limits.y],1u); }
        let layer = find_tile(u32(z),coord.x,coord.y);
        if layer >= 0 { return sample_tile(uv * f32(n) - coordinate,layer); }
        z -= 1;
        if z < 0 { break; }
    }
    return MISSING;
}
@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let i = id.x;
    let width = view.raster.x; let height = view.raster.y;
    if i >= width * height { return; }
    let x = i % width; let y = i / width;
    let nx = (f32(x)+0.5) / f32(width) * 2.0 - 1.0;
    let ny = 1.0 - (f32(y)+0.5) / f32(height) * 2.0;
    let dir = normalize(view.right.xyz * (nx * view.camera.w * FOV_HALF_TAN) + view.up.xyz * (ny * FOV_HALF_TAN) - view.forward.xyz);
    let camera = view.camera.xyz;
    let bb = dot(camera,dir); let cc = dot(camera,camera);
    let disc = bb * bb - (cc - 1.0);
    if disc < HORIZON_PRECISION_MARGIN || bb >= 0.0 { output[i] = 0u; return; }
    let t = -bb - sqrt(disc);
    let point = camera + dir * t;
    let incidence = max(-dot(point,dir),0.0);
    let lat = asin(clamp(point.z,-1.0,1.0));
    let lon = atan2(point.y,point.x);
    let mpp = t * EARTH_RADIUS_M * (2.0 * FOV_HALF_TAN / f32(height)) / sqrt(max(incidence,LOD_INC_COS));
    let lod = log2(2.0 * PI * EARTH_RADIUS_M * max(cos(lat),LOD_LAT_COS) / (256.0 * mpp)) + LOD_BIAS;
    var zl = ceil(lod); var fraction = 0.0;
    if view.raster.z != 0u { zl = floor(lod); fraction = lod - zl; }
    let z = u32(clamp(zl,0.0,f32(view.raster.w)));
    if z >= view.raster.w { fraction = 0.0; }
    let latc = clamp(lat,-MERCATOR_LIMIT_DEG * PI/180.0,MERCATOR_LIMIT_DEG * PI/180.0);
    let s = sin(latc);
    var v = 0.5 - log((1.0+s)/(1.0-s))/(4.0*PI);
    // The f64 Mercator clamp lands just beyond [0,1]. Reproduce its
    // tile-local edge choice instead of silently changing polar imagery.
    if lat >= MERCATOR_LIMIT_DEG*PI/180.0 { v = -MERCATOR_EDGE_EPSILON; }
    if lat <= -MERCATOR_LIMIT_DEG*PI/180.0 { v = 1.0+MERCATOR_EDGE_EPSILON; }
    let uv = vec2<f32>(fract((lon+PI)/(2.0*PI)), v);
    // f32 coordinate rounding must not choose the opposite side of a tile
    // seam. Repair these narrow strips with the shared precise CPU sampler.
    let seam_zoom = z + select(0u,1u,fraction > MIN_BLEND);
    let scaled_uv = uv*f32(1u << seam_zoom);
    if any(abs(scaled_uv-round(scaled_uv)) < vec2<f32>(TILE_SEAM_PRECISION_MARGIN)) { output[i] = 0u; return; }
    let offset = vec2<f32>(f32(x)-f32(width)/2.0,f32(y)-f32(height)/2.0);
    let distance = dot(offset,offset);
    var color = chain(z,uv,distance);
    if fraction > MIN_BLEND { color = blend(color,chain(z+1u,uv,distance),fraction); }
    if view.effects.x != 0u {
        var k = clamp((dot(point,view.sun.xyz)+TWILIGHT_OFFSET)/TWILIGHT_WIDTH,0.0,1.0);
        k = k*k*(3.0-2.0*k);
        color = color*(NIGHT_BRIGHTNESS+DAY_RANGE*k) + vec3<f32>(0.0,0.0,NIGHT_BLUE*(1.0-k));
    }
    if view.effects.y != 0u { let rim_base = 1.0-incidence; let rim_squared = rim_base*rim_base; let rim = rim_squared*rim_squared*RIM_BLEND; color = blend(color,RIM,rim); }
    let rgb = vec3<u32>(clamp(color,vec3<f32>(0.0),vec3<f32>(1.0))*255.0);
    output[i] = rgb.r | (rgb.g<<8u) | (rgb.b<<16u) | (255u<<24u);
}
