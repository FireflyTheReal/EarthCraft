# Aircraft projection fixture

`aircraft-original.bin` was captured from the production
`earthcraft-render/src/aircraft.rs` at upstream commit
`e9f193b` (unchanged projector from `eef7d33`) before replacing its callers.
It was produced on macOS ARM64 with the workspace's locked dependencies.

The fixture uses 20,000 records with hexadecimal index IDs, latitude
`index % 160 - 80`, longitude `index % 360 - 180`, heading 90 degrees,
440 knots, observation time 1000, and projection time 1010. The viewport is
800 by 600 logical points at map scale 1. Camera `(lat, lon, heading, altitude km)`:
`(0, 0, 25, 14000)`, `(30, 179.8, 145, 1000)`, `(89.9, -150, 90, 500)`.

Each case stores the visible count and up to eight evenly spaced sample rows
as little-endian u32 values. Each row contains the source index followed by
14 f32 components: position, four icon vertices, and two trail endpoints.
The regression permits 0.0001 logical points of floating-point variation.
Do not regenerate this baseline to accommodate a failed geometry test.
