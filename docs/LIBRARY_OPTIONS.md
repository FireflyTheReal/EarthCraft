# Library choices

Versions are centralized in the workspace manifest and resolved in `Cargo.lock`.
Each migration has a separate commit; supporting dependencies share a commit when
they implement the same boundary. Keep Rust 1.88 compatibility and avoid unrelated
updates. All original raster fixtures remain unchanged.

| Library | Current use | Compatibility constraint |
| --- | --- | --- |
| Serde | Tolerant settings and Nominatim response models; feature-at-a-time GeoJSON projection | Keep valid fields when siblings have wrong types; preserve defaults, scale clamping and coordinate order |
| kml + quick-xml | GeoRust KML reader/writer with a private XML preparation pass | Skip empty Points, balance incomplete outer envelopes, wrap legacy bare coordinates and retain localized names/longitude-first marker positions; no parallel legacy parser |
| thiserror | Typed data errors with source causes | Convert errors to user-facing text at presentation boundaries |
| glam | `DVec3` geometry, `dcamera` view/perspective constructors, `DMat2` panning and `DQuat` rotations; SIMD colors throughout the renderer | Keep f64 geographic precision and the original camera/raster output; retain emath for egui geometry |
| bytemuck | Safe zero-copy views of RGBA byte storage | Cast to byte-aligned pixel arrays, not aligned SIMD vectors; no unsafe code |
| Tokio + reqwest | App-owned async runtime, pooled HTTP clients and bounded service work | Preserve provider policy, timeout values, retries, disk fallback and UI completion delivery; CPU rendering remains on Rayon |
| Moka | Concurrent decoded-tile storage with byte weights and LRU eviction | 256 MiB is a best-effort target; keep request admission/retries separate and do not force frame-by-frame maintenance |
| Fluent | Embedded English/Portuguese catalogs with cached labels and named message arguments | Preserve wording and number precision; test syntax, parity, resolution and fallback |
| rstar | R*-tree of accepted screen-space label rectangles | Preserve ordered admission, inclusive edge intersection and the 500-label limit; index only the current overlay rebuild, without changing geographic visibility |

Do not introduce matrices, affine transforms or typed integer vectors just because
glam exposes them. Use an operation/type where it removes handwritten math or
expresses an actual domain value. The globe uses right-handed double-precision
`dcamera` constructors while retaining its field of view, analytic ray/sphere
intersection and horizon visibility. Geographic conversion, haversine distance
and great-circle interpolation remain explicit: glam's `DVec3::slerp` rescales
lengths and uses different fallbacks for nearly identical/opposite vectors. Cursor rotation keeps
the existing axis/angle threshold rather than changing its small-angle behavior
with `from_rotation_arc`. Bytemuck does not make arbitrary buffers SIMD-aligned.

The data crate streams large boundary inputs and shares immutable coordinate
lines. Decoded tiles have a best-effort weighted target with LRU eviction; expensive Brazilian
municipal geometry is loaded on demand. These are resource-policy changes with
deterministic tests, separate from the library migrations.

See [architecture](ARCHITECTURE.md), [localization](LOCALIZATION.md) and
[contributing](CONTRIBUTING.md) for the ownership boundaries and verification rules.

Moka uses its `sync` feature because Rayon needs immediate cache reads. UUID is
locked to 1.26.0 to preserve Rust 1.88 compatibility; newer UUID releases resolved
by the local stable toolchain require Rust 1.89. Review transitive MSRVs when
updating the lockfile. No extra UUID API or dependency is exposed by EarthCraft.

See [geospatial evaluation](GEOSPATIAL.md) for measured `geo` comparisons and
the proposed roles of geozero, geo-index and Mapbox Vector Tiles. Those libraries
are not runtime dependencies. Library reference code and fixtures can validate
our approach without replacing a faster or better-fitting implementation.
