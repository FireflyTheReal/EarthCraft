# Library choices

Versions are centralized in the workspace manifest and resolved in `Cargo.lock`.
Each migration has a separate commit; supporting dependencies share a commit when
they implement the same boundary. Keep Rust 1.88 compatibility and avoid unrelated
updates. All original raster fixtures remain unchanged.

| Library | Current use | Compatibility constraint |
| --- | --- | --- |
| Serde | Tolerant settings and Nominatim response models; feature-at-a-time GeoJSON projection | Keep valid fields when siblings have wrong types; preserve defaults, scale clamping and coordinate order |
| quick-xml | KML event reader and writer | Preserve the existing Point subset, CDATA, entities, fallback names and incomplete-envelope compatibility |
| thiserror | Typed data errors with source causes | Convert errors to user-facing text at presentation boundaries |
| glam | `DVec3` globe geometry, `DMat3` camera basis, `DMat2` panning and `DQuat` rotations; SIMD color interpolation | Keep f64 geographic precision and the original camera/raster output; retain emath for egui geometry |
| bytemuck | Safe zero-copy views of RGBA byte storage | Cast to byte-aligned pixel arrays, not aligned SIMD vectors; no unsafe code |
| Tokio + reqwest | App-owned async runtime, pooled HTTP clients and bounded service work | Preserve provider policy, timeout values, retries, disk fallback and UI completion delivery; CPU rendering remains on Rayon |
| Fluent | Embedded English/Portuguese catalogs with cached labels and named message arguments | Preserve wording and number precision; test syntax, parity, resolution and fallback |

Do not introduce matrices, affine transforms or typed integer vectors just because
glam exposes them. Use an operation/type where it removes handwritten math or
expresses an actual domain value. The globe's ray/sphere model needs its existing
camera conventions; a generic perspective constructor would require a deliberate
projection change. Bytemuck does not make arbitrary buffers SIMD-aligned.

The data crate streams large boundary inputs and shares immutable coordinate
lines. Decoded tiles have a byte budget with LRU eviction; expensive Brazilian
municipal geometry is loaded on demand. These are resource-policy changes with
deterministic tests, separate from the library migrations.

See [architecture](ARCHITECTURE.md), [localization](LOCALIZATION.md) and
[contributing](CONTRIBUTING.md) for the ownership boundaries and verification rules.
