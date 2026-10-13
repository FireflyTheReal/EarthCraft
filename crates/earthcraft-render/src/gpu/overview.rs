//! Full GPU overview ray casting; bounded exact-coordinate tile lookup.
use super::*;
use crate::tuning;
use earthcraft_core::{EARTH_RADIUS_M, VERTICAL_FOV_HALF_TAN};
use earthcraft_data::tiles::{make_key, unkey, Lookup};
const MIN_GPU_ALTITUDE_KM: f64 = 1000.0;
const MAX_TRACKED_ZOOM: u8 = 10;
const TILE_TABLE_SIZE: usize = 2048;
const MAX_RASTER_PIXELS: usize = 2000 * 2000;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct View {
    camera: [f32; 4],
    right: [f32; 4],
    up: [f32; 4],
    forward: [f32; 4],
    sun: [f32; 4],
    raster: [u32; 4],
    effects: [u32; 4],
    limits: [u32; 4],
}
pub(super) struct Context<'a> {
    pub device: &'a wgpu::Device,
    pub queue: &'a wgpu::Queue,
    pub atlas: &'a wgpu::TextureView,
}
pub(super) struct Frame {
    pub view: RenderedView,
    pub sampled: Vec<TileKey>,
}
pub(super) struct Overview {
    pipeline: wgpu::ComputePipeline,
    uniform: wgpu::Buffer,
    table: wgpu::Buffer,
    output: wgpu::Buffer,
    needs: wgpu::Buffer,
    staging: wgpu::Buffer,
    pixels: usize,
    dependency_count: usize,
    empty: Vec<u32>,
}
fn buffer(
    device: &wgpu::Device,
    label: &str,
    size: usize,
    usage: wgpu::BufferUsages,
) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size: size as u64,
        usage,
        mapped_at_creation: false,
    })
}
impl Overview {
    pub(super) fn new(device: &wgpu::Device) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("EarthCraft overview ray casting"),
            source: wgpu::ShaderSource::Wgsl(super::shader::source().into()),
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("EarthCraft overview renderer"),
            layout: None,
            module: &shader,
            entry_point: "main",
        });
        let storage = wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_DST
            | wgpu::BufferUsages::COPY_SRC;
        Self {
            pipeline,
            uniform: buffer(
                device,
                "EarthCraft camera",
                std::mem::size_of::<View>(),
                wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            ),
            table: buffer(
                device,
                "EarthCraft tile table",
                TILE_TABLE_SIZE * 16,
                storage,
            ),
            output: buffer(device, "EarthCraft overview pixels", 4, storage),
            needs: buffer(device, "EarthCraft overview dependencies", 8, storage),
            staging: buffer(
                device,
                "EarthCraft overview readback",
                12,
                wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            ),
            pixels: 1,
            dependency_count: 1,
            empty: vec![u32::MAX; 2],
        }
    }
    fn reserve(&mut self, device: &wgpu::Device, pixels: usize, dependencies: usize) {
        if pixels <= self.pixels && dependencies <= self.dependency_count {
            return;
        }
        self.pixels = self.pixels.max(pixels);
        self.dependency_count = self.dependency_count.max(dependencies);
        let storage = wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_DST
            | wgpu::BufferUsages::COPY_SRC;
        self.output = buffer(
            device,
            "EarthCraft overview pixels",
            self.pixels * 4,
            storage,
        );
        self.needs = buffer(
            device,
            "EarthCraft overview dependencies",
            (self.dependency_count + 1) * 4,
            storage,
        );
        self.staging = buffer(
            device,
            "EarthCraft overview readback",
            (self.pixels + self.dependency_count + 1) * 4,
            wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        );
        self.empty.resize(self.dependency_count + 1, u32::MAX);
    }
    pub(super) fn render(
        &mut self,
        context: Context<'_>,
        request: &RenderRequest,
        slots: &HashMap<TileKey, u32>,
        lookup: &(impl Fn(TileKey) -> Lookup + Sync),
        mut pixels: Vec<u8>,
    ) -> Result<Frame, (String, Vec<u8>, bool)> {
        let Context {
            device,
            queue,
            atlas,
        } = context;
        let Some(max_zoom) = supported_zoom(request) else {
            return Err(("View requires the precise CPU renderer (close altitude, exact grid, or raster limits)".into(),pixels,false));
        };
        let count = request.width * request.height;
        let dependencies = ((1usize << (2 * (max_zoom as usize + 1))) - 1) / 3;
        self.reserve(device, count, dependencies);
        let rotation = request.basis.rotation_from_view();
        let v4 = |vector: glam::DVec3, w| [vector.x as f32, vector.y as f32, vector.z as f32, w];
        let view = View {
            camera: v4(request.basis.c, request.basis.aspect_ratio() as f32),
            right: v4(rotation.x_axis, 0.0),
            up: v4(rotation.y_axis, 0.0),
            forward: v4(rotation.z_axis, 0.0),
            sun: v4(request.cfg.sun.unwrap_or_default(), 0.0),
            raster: [
                request.width as u32,
                request.height as u32,
                u32::from(request.cfg.provider.is_imagery()),
                request.cfg.provider.max_zoom() as u32,
            ],
            effects: [
                u32::from(request.cfg.sun.is_some()),
                u32::from(request.cfg.atmo),
                0,
                0,
            ],
            limits: [
                max_zoom as u32,
                dependencies as u32,
                (TILE_TABLE_SIZE - 1) as u32,
                0,
            ],
        };
        let mut table = vec![[u32::MAX; 4]; TILE_TABLE_SIZE];
        for (&key, &layer) in slots {
            let (_, z, x, y, _) = unkey(key);
            let mut index = tile_hash(z as u32, x, y);
            while table[index][0] != u32::MAX {
                index = (index + 1) & (TILE_TABLE_SIZE - 1);
            }
            table[index] = [z as u32, x, y, layer];
        }
        queue.write_buffer(&self.uniform, 0, bytemuck::bytes_of(&view));
        queue.write_buffer(&self.table, 0, bytemuck::cast_slice(&table));
        queue.write_buffer(
            &self.needs,
            0,
            bytemuck::cast_slice(&self.empty[..dependencies + 1]),
        );
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("EarthCraft overview view"),
            layout: &self.pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: binding(&self.output, (count * 4) as u64),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(atlas),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: self.table.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: binding(&self.needs, ((dependencies + 1) * 4) as u64),
                },
            ],
        });
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("EarthCraft GPU overview"),
        });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("EarthCraft overview rays and shading"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &group, &[]);
            pass.dispatch_workgroups((count as u32).div_ceil(WORKGROUP_SIZE), 1, 1);
        }
        encoder.copy_buffer_to_buffer(&self.output, 0, &self.staging, 0, (count * 4) as u64);
        encoder.copy_buffer_to_buffer(
            &self.needs,
            0,
            &self.staging,
            (count * 4) as u64,
            ((dependencies + 1) * 4) as u64,
        );
        queue.submit(Some(encoder.finish()));
        let slice = self
            .staging
            .slice(..((count + dependencies + 1) * 4) as u64);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = tx.send(result);
        });
        device.poll(wgpu::Maintain::Wait);
        if let Err(error) = rx
            .recv()
            .map_err(|error| error.to_string())
            .and_then(|result| result.map_err(|error| error.to_string()))
        {
            return Err((error, pixels, true));
        }
        let mut required = Vec::new();
        {
            let mapped = slice.get_mapped_range();
            pixels.resize(count * 4, 0);
            pixels.copy_from_slice(&mapped[..count * 4]);
            let needs: &[u32] = bytemuck::cast_slice(&mapped[count * 4..]);
            if needs[dependencies] != u32::MAX {
                drop(mapped);
                self.staging.unmap();
                return Err((
                    "GPU LOD exceeded its checked dependency bounds".into(),
                    pixels,
                    false,
                ));
            }
            for z in 0..=max_zoom {
                let start = ((1usize << (2 * z as usize)) - 1) / 3;
                let n = 1usize << z;
                for (i, &dist) in needs[start..start + n * n].iter().enumerate() {
                    if dist != u32::MAX {
                        required.push((
                            make_key(
                                request.cfg.provider,
                                z,
                                (i % n) as u32,
                                (i / n) as u32,
                                request.cfg.rel,
                            ),
                            f32::from_bits(dist),
                        ));
                    }
                }
            }
        }
        self.staging.unmap();
        let sampled = required.iter().map(|entry| entry.0).collect();
        let mut required_map: HashMap<_, _> = required
            .into_iter()
            .filter(|(key, _)| !slots.contains_key(key))
            .collect();
        for (key, distance) in super::repair::pixels(request, &mut pixels, lookup) {
            let distance = distance.unwrap_or(f32::MAX);
            required_map
                .entry(key)
                .and_modify(|previous| *previous = previous.min(distance))
                .or_insert(distance);
        }
        let mut required: Vec<_> = required_map.into_iter().collect();
        required.sort_by(|a, b| unkey(a.0).1.cmp(&unkey(b.0).1).then(a.1.total_cmp(&b.1)));
        let waiting: Vec<_> = required.iter().map(|entry| entry.0).collect();
        let requests = required
            .into_iter()
            .filter(|entry| matches!(lookup(entry.0), Lookup::Absent))
            .map(|entry| entry.0)
            .collect();
        Ok(Frame {
            view: RenderedView {
                pixels,
                waiting,
                requests,
            },
            sampled,
        })
    }
}
pub(super) fn supported_zoom(request: &RenderRequest) -> Option<u8> {
    let count = request.width.checked_mul(request.height)?;
    if request.width == 0
        || request.height == 0
        || count > MAX_RASTER_PIXELS
        || request.cam.alt < MIN_GPU_ALTITUDE_KM
        || !request.cam.alt.is_finite()
        || request.cfg.grid
    {
        return None;
    }
    let pa = 2.0 * VERTICAL_FOV_HALF_TAN / request.height as f64;
    let max_lod = ((2.0 * std::f64::consts::PI * EARTH_RADIUS_M)
        / (tuning::TILE_SIZE_PX * request.cam.alt * earthcraft_core::METERS_PER_KM * pa))
        .log2()
        + tuning::LOD_SHARPNESS_BIAS;
    let max_zoom = (max_lod.ceil().max(if request.cfg.provider.is_imagery() {
        1.0
    } else {
        0.0
    }) as u8)
        .min(request.cfg.provider.max_zoom());
    (max_zoom <= MAX_TRACKED_ZOOM).then_some(max_zoom)
}
fn tile_hash(z: u32, x: u32, y: u32) -> usize {
    (z.wrapping_mul(73856093) ^ x.wrapping_mul(19349663) ^ y.wrapping_mul(83492791)) as usize
        & (TILE_TABLE_SIZE - 1)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shader_validates_without_an_adapter_and_uniform_layout_matches() {
        let module = naga::front::wgsl::parse_str(&super::super::shader::source())
            .expect("valid embedded WGSL");
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::empty(),
        )
        .validate(&module)
        .expect("portable shader operations");
        assert_eq!(std::mem::size_of::<View>(), 128);
        assert_eq!(std::mem::offset_of!(View, raster), 80);
        assert_eq!(std::mem::offset_of!(View, effects), 96);
        assert_eq!(std::mem::offset_of!(View, limits), 112);
        let shader_view = module
            .types
            .iter()
            .find(|(_, ty)| ty.name.as_deref() == Some("View"))
            .unwrap()
            .1;
        let naga::TypeInner::Struct { members, span } = &shader_view.inner else {
            panic!("View must remain a uniform struct");
        };
        assert_eq!(*span as usize, std::mem::size_of::<View>());
        assert_eq!(
            members[5].offset as usize,
            std::mem::offset_of!(View, raster)
        );
    }
}
