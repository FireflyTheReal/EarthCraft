# Library options

These are proposed follow-up refactors, not new dependencies in this change.
Adopt a library when it removes concrete maintenance work, and test the existing
behavior at the boundary before replacing handwritten code. Keep workspace
versions centralized and verify Rust 1.88 compatibility for the chosen release.

| Library | Current maintenance burden | Proposed use |
| --- | --- | --- |
| [Serde derives](https://serde.rs/derive.html) | Manual field extraction from `serde_json::Value` in settings and service responses | Typed settings and response structs with explicit defaults and conversions |
| [quick-xml](https://docs.rs/quick-xml/) | Handwritten XML escaping, tag slicing and KML extraction | XML reader/writer for the existing KML subset |
| [thiserror](https://docs.rs/thiserror/) | Service errors become strings immediately, losing structured causes | Domain error enums in data, converted to the existing localized status text at the app boundary |
| [glam DVec3](https://docs.rs/glam/latest/glam/f64/struct.DVec3.html) | Handwritten vector arithmetic and normalization in `V3` | Double-precision vector operations in core and render |

Serde and quick-xml address the largest parsing burden. `serde_json` already uses
Serde internally, but the project does not yet declare Serde derives directly.
Use explicit compatibility logic: the current settings loader accepts valid
fields even if another field has the wrong type, retains defaults for missing or
empty paths, and clamps scales. A plain derived struct can reject the entire
object and would change that behavior. Search responses also mix string numbers,
optional fields and provider-specific shapes.

For KML, test the currently supported Point coordinates, longitude-first order,
escaping, CDATA, fallback names and malformed-input behavior. An XML library may
accept more documents or reject documents the old slicing parser accepted;
review that separately from this structural refactor.

`thiserror` provides error derives, but the meaningful change is choosing useful
variants and preserving source errors. Keep localized UI text in the app and
preserve existing messages during a mechanical migration. Avoid adding both
`thiserror` and `anyhow` without a concrete need for each.

`glam::DVec3` supplies dot/cross/normalization operations. Use the f64 vector type,
retain the existing coordinate conventions, and run camera and original-raster
regression tests. Library normalization and arithmetic may change floating-point
rounding; do not silently update expected renderer output to accommodate it.

The existing synchronous `ureq`, Rayon and egui/eframe stack already fits the
application. A new async runtime or ECS is not needed to perform these focused
cleanups.
