# Library choices

Versions are centralized in the workspace manifest and resolved in `Cargo.lock`.
Supporting dependencies implement concrete package boundaries. Keep Rust 1.89
compatibility, review dependency changes separately, and retain regression fixtures.

| Library | Current use | Compatibility constraint |
| --- | --- | --- |
| Serde | Tolerant settings and Nominatim response models; feature-at-a-time GeoJSON projection | Keep valid fields when siblings have wrong types; preserve defaults, scale clamping and coordinate order |
| kml + quick-xml | GeoRust KML reader/writer with a private XML preparation pass | Skip empty Points, balance incomplete outer envelopes, wrap legacy bare coordinates and retain localized names/longitude-first marker positions; no parallel legacy parser |
| thiserror | Typed data errors with source causes | Convert errors to user-facing text at presentation boundaries |
| glam | `DVec3` geometry, `dcamera` view/perspective constructors, `DMat2` panning and `DQuat` rotations; SIMD colors throughout the renderer | Keep f64 geographic precision and camera/raster contracts; retain emath for egui geometry |
| bytemuck | Safe zero-copy views of RGBA byte storage | Cast to byte-aligned pixel arrays, not aligned SIMD vectors; no unsafe code |
| wide | Four independent f64 ray lanes for camera rotation, normalization and sphere intersection | Rust 1.89 minimum; preserve geographic precision, regression fixture tolerances and missing-tile priorities; padded lanes never shade or request tiles |
| Tokio + reqwest | App-owned async runtime, pooled HTTP clients and bounded service work | Preserve provider policy, timeout values, retries, disk fallback and UI completion delivery; CPU rendering remains on Rayon |
| Moka | Concurrent decoded-tile storage with byte weights and LRU eviction | 256 MiB is a best-effort target; keep request admission/retries separate and do not force frame-by-frame maintenance |
| Fluent | Embedded English/Portuguese catalogs with cached labels and named message arguments | Preserve wording and number precision; test syntax, parity, resolution and fallback |
| wgpu + pollster | Optional background GPU overview rasterization and device initialization | Preserve CPU fallback, bound GPU residency, validate shader/precision and keep execution/readback off egui |
| Naga (tests) | Headless WGSL validation and uniform-layout checks | Match wgpu's shader version; native adapter tests remain separate |
| geo-index | Packed immutable boundary-line visibility index | Conservative horizon/frustum and dateline/pole tests before projection |
| tokio-rusqlite + rusqlite | SQLite-backed MBTiles persistence | Separate providers/releases/formats, TMS rows, managed-byte eviction and epoch-safe clearing |
| fst + unicode-normalization | Loaded-name prefix indexes and international matching | Preserve ranking, original display names and snapshot generations |
| rstar | R*-tree of accepted screen-space label rectangles | Preserve ordered admission, inclusive edge intersection and the 500-label limit; index only the current overlay rebuild, without changing geographic visibility |

Do not introduce matrices, affine transforms or typed integer vectors just because
glam exposes them. Use an operation/type where it removes handwritten math or
expresses an actual domain value. The globe uses right-handed double-precision
`dcamera` constructors while retaining its field of view, analytic ray/sphere
intersection and horizon visibility. Geographic conversion, haversine distance
and great-circle interpolation remain explicit: glam's `DVec3::slerp` rescales
lengths and uses different fallbacks for nearly identical/opposite vectors. Cursor rotation keeps
the axis/angle threshold rather than changing its small-angle behavior
with `from_rotation_arc`. Bytemuck does not make arbitrary buffers SIMD-aligned.

The data crate streams large boundary inputs and shares immutable coordinate
lines. Decoded tiles have a best-effort weighted target with LRU eviction; expensive Brazilian
municipal geometry is loaded on demand. Deterministic tests cover these resource policies.

See [architecture](ARCHITECTURE.md), [localization](LOCALIZATION.md) and
[contributing](CONTRIBUTING.md) for the ownership boundaries and verification rules.

Moka uses its `sync` feature because Rayon needs immediate cache reads. Review transitive MSRVs when updating the lockfile;
EarthCraft does not expose a UUID API.

See [geospatial evaluation](GEOSPATIAL.md) for measured `geo` comparisons and
runtime geo-index usage and evaluation of geozero and Mapbox Vector Tiles.
Geozero and MVT are not runtime dependencies. Library reference code and fixtures can validate
our approach without replacing a faster or better-fitting implementation.
