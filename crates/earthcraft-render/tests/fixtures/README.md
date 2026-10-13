# Original raster fixtures

These four 64 × 48 RGBA frames were captured by calling `render_row` in the
unmodified `src/main.rs` at upstream commit `ae3b8f19e1942835403b4ff5bae435ea3795737b`.
They were generated on macOS ARM64 with Rust 1.99.0 before replacing the renderer.
Each `.rgba` file is raw row-major RGBA8, 12,288 bytes, with no image header.

The camera is latitude -15°, longitude -50°, heading 25°, altitude 14,000 km.
The provider is Satellite, release 0, with a 10° grid step where enabled.

| File | Grid | Atmosphere | Sun | Cache |
| --- | --- | --- | --- | --- |
| `plain.rgba` | off | off | none | empty |
| `atmosphere.rgba` | off | on | none | empty |
| `grid-sun.rgba` | on | on | latitude 20°, longitude 45° | empty |
| `loaded.rgba` | on | on | latitude 20°, longitude 45° | one level-0 tile |

`tile.png` is the loaded 2 × 2 tile: red/green on its first row and blue/white on
its second, all fully opaque. The original renderer sampled those same pixels.
No tile downloads or user caches were involved.

The renderer test compares every output channel, permitting one quantization
step for floating-point differences between platforms. Missing tile requests
must still be reported. These fixtures are regression evidence, not generated
build output. Do not overwrite them to accommodate an unexplained failure.
For an intentional rendering change, review the image difference and update
fixtures separately with the source commit, camera/configuration and rationale.

## Ray-batching boundary fixtures

The additional `batches/` snapshots were captured from EarthCraft commit
`c31bfa8` in an isolated temporary source checkout before enabling wide ray
batching. The capture called the unchanged production `render_pixels`; it did
not copy the pixel algorithm or generate expectations from the new SIMD code.
Original upstream fixtures above remain unchanged. Captured on macOS ARM64,
Rust 1.99.0, optimized dev profile without debug information/incremental output.

All cases use grid/atmosphere/sunlight, a 10° grid, sun at (20°, 45°), release 7,
and the existing 2×2 `tile.png` available only at level 0 for the chosen provider.
Everything else is absent. No user caches, services or requests are involved.

| Name | Size | Provider | Latitude | Longitude | Heading | Altitude km |
| --- | --- | --- | --- | --- | --- | --- |
| odd-satellite | 65×49 | Satellite | -15 | -50 | 25 | 14000 |
| polar-streets | 67×51 | Streets | 89.9 | 179.9 | -130 | 1000 |
| dateline-topo | 63×47 | Topo | -40 | 179.9 | 70 | 500 |
| narrow-wayback | 3×9 | Wayback | 0 | -180 | 180 | 30000 |
| one-pixel-column | 1×7 | Satellite | -15 | -50 | 25 | 14000 |
| low-altitude | 7×5 | Satellite | 89.9 | -179.9 | -20 | 0.15 |

Each `.rgba` is row-major RGBA8 (`width × height × 4` bytes), with the same
one-channel quantization tolerance as the original fixtures. Each `.misses` is
a list sorted by packed tile key: little-endian `u64` key followed by `f32`
nearest-pixel squared distance, 12 bytes per entry. Missing keys/priorities are
compared exactly, including short row and eight-row batch tails. Do not overwrite
these snapshots to accommodate a batching regression.
