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
