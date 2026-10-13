# GPU renderer plan

Status: design requirement only. EarthCraft still ray-casts the globe on the CPU.
The current egui presentation backend and a future GPU globe renderer are separate
choices; changing the UI backend alone does not move globe computation to the GPU.

## Optional acceleration

Keep the CPU renderer available in every normal desktop build. Automatic selection
should use a GPU path only after adapter/device creation and required capabilities
and limits succeed. Missing adapters, unsupported requirements or initialization
errors must select the CPU globe path with an English diagnostic rather than
preventing startup. Provide a way to force CPU rendering for diagnostics and
reliable CPU/GPU comparisons. None of that selection is implemented yet.

Wgpu exposes fallible [adapter selection](https://docs.rs/wgpu/latest/wgpu/struct.Instance.html#method.request_adapter)
and [device creation](https://docs.rs/wgpu/latest/wgpu/struct.Adapter.html#method.request_device).
Check shader precision and rendering parity as well as API availability. Keep
double-precision camera/picking contracts; do not require every adapter to support
our existing CPU numeric representation. Device-loss handling also needs an
explicit tested recovery/fallback strategy; it is not supplied by retaining CPU code.
CPU globe rendering still needs a functioning graphics driver for the native UI.

## Implementation checkpoints

1. Retain the opt-in native CPU baseline and original raster fixtures.
2. Introduce an optional GPU path alongside CPU, with explicit backend selection,
   capability checks and failure-path tests. Keep ownership in render; egui
   presentation stays in the app.
3. Port globe shading/sampling, with precision checks at altitude extremes, poles,
   dateline and horizon. Preserve providers, LOD/fallback, effects and displayed
   camera alignment. Keep tile request planning off shader readback paths.
4. Bound GPU tile residency separately from the decoded RAM and encoded disk
   budgets. Preserve visible-request priority and global overview warming.
5. Validate screenshot output, resizing, runtime failures and CPU-forced operation.
   Repeat the same native workloads and validate Windows/macOS/Linux before the
   planned 0.1.0 release decision.

CPU profiling supports this direction but also shows overlay/index work that GPU
rasterization will not automatically accelerate. See [the baseline](PERFORMANCE_BASELINE.md).
