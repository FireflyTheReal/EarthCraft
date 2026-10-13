# EarthCraft contributor and agent rules

Read `docs/ARCHITECTURE.md` and `docs/CONTRIBUTING.md` before structural changes.
Portuguese architecture notes are in `docs/ARCHITECTURE.pt-BR.md`.
User instructions and the requested scope take precedence over these defaults.

## Ownership and dependencies

- `apps/earthcraft` owns desktop startup, egui widgets, input, application state,
  texture upload, native dialogs, native desktop menus, and background-result polling.
  Native menu callbacks enqueue commands and wake egui; apply actions and open
  dialogs only from the app update. Keep menu objects on the window thread,
  localize captions in both catalogs, and reuse the existing file services.
  Workspace code forbids unsafe Rust. Native menus are macOS-only; Windows and
  Linux use the app's sidebar controls.
- `crates/earthcraft-core` owns geometry, camera operations, language selection,
  embedded localization, and placemark values. It may use `emath` for screen geometry; keep network,
  filesystem, egui widgets, and windowing out of it.
- `crates/earthcraft-data` owns tile services, search, boundary loading, and local
  persistence. It depends on core. Report results or invoke the injected repaint
  callback; do not add egui/eframe dependencies.
- `crates/earthcraft-render` owns rasterization, frame coordination, geographic
  overlay projection/visibility/occupancy and tile request prioritization.
  It depends on core and data, never on the desktop application.
- Dependencies flow downward. Do not create cycles, import app code into libraries,
  or add a crate just to shorten a file. Use modules for related implementation.

## Making changes

- For behavior-preserving changes, preserve features, UI strings, defaults, keyboard/mouse
  behavior, persistence paths/formats, service URLs, timeouts, and rendering order.
  Feature requests may change behavior explicitly; document the difference.
- Put logic in the owning crate and surface drawing in the matching app UI module.
  Pure geographic visibility, overlay projection, ordered label candidates and
  rectangle occupancy belong in render, with `emath` screen coordinates and no
  egui dependency. The app measures fonts and creates/draws styled egui shapes.
  Camera flight interpolation belongs in core; local suggestion ranking/merging
  belongs in data. The bounded frame coordinator belongs in render and receives
  a repaint callback. UI event timing, input delivery, texture ownership and
  native dialogs remain app responsibilities. Audit the owner before adding
  non-widget algorithms to an app module.
  Keep body/flight models and camera-control math in core; body meshes, weather
  compositing, ruler/aircraft projection and raster compatibility policy in render.
  Keep live imagery refresh/fetch state and portable planet documents in data.
  Frontends own textures, widgets, OS commands and native dialogs. Expose byte/image
  APIs where another frontend can supply its own file or storage integration;
  moving logic into crates alone does not imply WebAssembly support.
  Keep `main.rs` limited to startup. Avoid catch-all `utils` or `common` modules.
- Keep implementation private. Expose only APIs needed across modules/crates;
  use `pub(super)`/`pub(crate)` where possible. Keep synchronization internals private.
- Prefer named structures when tuples or positional arguments hide meaning.
  Add abstractions only for a concrete dependency boundary or multiple real uses.
- Use workspace dependency versions and shared package metadata. Keep `Cargo.lock`
  tracked. Do not update unrelated dependencies during unrelated changes.
- Import unchanged dependency types, traits, and functions directly from their
  owning package. Do not re-export them through EarthCraft crates or hide them
  behind equivalent aliases. Declare a direct workspace dependency when needed.
  Re-exporting EarthCraft-owned APIs from private implementation modules is fine;
  domain aliases must express an application concept rather than rename a dependency.
- Rust 1.89 is the declared minimum. Avoid newer language/library APIs unless
  intentionally updating the minimum and its CI job together.
- Write application-owned error messages and diagnostic logs in English, including
  error statuses displayed when the UI language is Portuguese. Normal UI labels
  and success messages remain localized. Preserve external names and source errors.
- Keep new error handling explicit and user-facing where appropriate. Do not add
  blanket lint allowances or swallow new errors just to make checks pass.
- Write explanatory code comments in English, followed by Portuguese if desired.
- Name policy limits, thresholds, timing, units, colors, and non-obvious coefficients.
  Include units in names (`_KM`, `_DEG`, `_SECS`, `_PX`, `_POINTS`). Keep constants
  with their owning module; share only values with the same meaning. Keep values
  and floating-point evaluation order unchanged when naming constants.
- Leave obvious algebraic values (zero, one, halves, vector indices) inline unless
  they encode a format or policy. Avoid a global bag of unrelated constants.
- Use rustfmt. A module approaching 400 lines is a review signal, not a quota:
  split by responsibility, keeping related code and tests together.

## Library and resource practices

- Use glam for vector/matrix/rotation operations. Keep camera and geographic
  geometry in `f64` (`DVec*`, `DMat*`, `DQuat`); use SIMD `Vec4` through tile
  sampling, blending, shading and final RGB clamping. Use `dcamera` view/projection
  constructors for the double-precision camera. Preserve right-handed world axes,
  Y-up/-Z-forward view space, screen Y inversion, field of view and horizon picking.
  Cache camera transforms per basis; do not invert matrices per raster pixel.
  Choose types for actual operations rather than replacing every scalar or tuple.
  Check precision and singular-case behavior before replacing geographic formulas
  with approximate interpolation or rotation helpers.
  Egui screen geometry stays in `emath`; do not add conversions without a use.
- Use `wide` for independent f64 pixel lanes in CPU ray construction and sphere
  intersection. Preserve scalar evaluation order and geographic precision; keep
  short final batches from shading padded pixels or generating tile requests.
  Compare pixels and missing-tile priorities against pre-change fixtures before
  expanding batching to other formulas. Avoid unmeasured approximate math.
- Use safe bytemuck casts for byte-backed pixel layouts. RGBA arrays have byte
  alignment; a byte buffer is not automatically aligned for a SIMD vector. Keep
  channel order and padding explicit. Do not add unsafe casts or unsafe POD impls.
- Keep structured `earthcraft_data::Error` causes until the presentation boundary.
  Serde settings/service models must retain tolerant field defaults and longitude-first
  coordinate arrays. Use GeoRust `kml` for KML decoding/writing; the private XML
  preparation pass skips empty Points and prevents truncated-container EOF loops.
  Keep marker coordinate order, fallback names and legacy bare-coordinate/outer
  fragment imports. Do not reintroduce a parallel handwritten KML parser.
  Treat geographic libraries as reference implementations for differential tests
  and benchmarks when useful; add runtime dependencies for demonstrated needs.
- The app owns one `ServiceRuntime`. Inject its handle and repaint callbacks into
  data; use pooled reqwest clients and async I/O. Keep CPU rasterization on Rayon,
  large decoding/projection on bounded blocking workers, and UI access on egui's
  thread. Completion must wake the UI, including the last background job.
- Run KML I/O and screenshot encoding on the bounded data-owned file service.
  Capture save/export snapshots at submission, preserve dropped-file ordering,
  wake the UI on both success and failure, and apply results only in the app.
  Native dialogs remain app-owned; never wait on a file job in egui.
  Retain displayed RGBA through shared immutable ownership for screenshots;
  never copy the entire frame on submission or recycle a buffer with readers.
- Keep whole-frame rasterization on the render-owned coordinator with the
  Rayon pool. Bound pending/completed frames; coalesce pending camera requests.
  Reject incompatible provider/style/size results and align overlays with the
  displayed frame's camera. Keep texture upload on egui's thread; never wait there
  for a raster job or create a pool per frame.
  Prepare backend CPU images and camera-matched overlay geometry before frame
  publication. Keep backend types in the app-provided preparation callback;
  render must not depend on egui. Preserve unchanged-view geometry reuse.
- Keep overlay caching scoped to one displayed view. Invalidate on boundary
  patches, camera/projection, viewport, DPI, visible layers, language and map
  text scale. Keep ruler and placemark edits immediate. Screen-space label
  indexing must preserve stable priority/rank order, touching-edge rejection,
  padding and the 500-label limit; do not use spatial traversal order as priority.
  Build boundary indexes, cull/project geometry and sort label candidates off
  the UI thread. Coalesce stationary overlay jobs and reject stale views/data.
  Keep font layout, final occupancy and shapes on the UI thread. Publish geometry
  with moving raster frames so coalescing cannot starve overlays during motion.
  Merge boundary patches and build immutable name snapshots in data's processor;
  UI application of a result should swap shared snapshots, not clone/dedupe datasets.
  Coalesce settings/language writes on the persistence worker, report failures,
  and flush pending writes only at final shutdown. Preserve filenames and JSON.
- Bound decoded tiles by pixel bytes, not only tile counts. The Moka `sync` cache
  uses a 256 MiB best-effort weighted target and LRU eviction; brief overshoot is
  expected, so do not describe it as a hard RAM ceiling. Do not wrap ready-cache
  reads in an application mutex or force maintenance on every frame. Request
  admission and retry metadata remain separately synchronized. Preserve retry policy
  and disk fallback. Stream large GeoJSON feature collections; share immutable
  coordinate lines and request municipal data only when its layers are visible.
  Account for temporary references separately when measuring.
- Persist imagery through MBTiles and the single tokio-rusqlite worker. Keep
  provider/release/encoded-format identities separate, convert XYZ rows to TMS,
  retain legacy-file fallback/migration and respect the disk epoch on writeback.
  The 128 MiB disk target counts managed file bytes, including SQLite overhead;
  reclaim pages when evicting and prefer coarse overview retention. Publish tiles
  before optional persistence. Keep SQLite out of UI/pixel loops. Clear only owned
  tile directories, preserve other data and report actual usage. R-tree/FTS5
  belong to a separate auxiliary catalog, not imagery's exact-key hot path.
- Keep startup overview warming bounded and subordinate to visible requests. Use
  the cache budget and disk loader; avoid pinning a second unaccounted
  tile set. Respect provider prefetch policies; the public OSM service stays on
  demand. Measure CPU frame time separately from network readiness and GUI work.
  Renderer batch-local tile handles must be released after their batch, rather
  than accumulating a persistent duplicate cache.
- Keep loaded-name prefix indexes in data. Build normalized FST snapshots off
  the UI thread and coalesce pending revisions. Preserve both languages, Unicode
  folding, rank/kind/name-byte ordering, stable source ties and remote merging.
  Never use lexical trie traversal as result priority. Keep a scan fallback while
  rebuilding and do not add a duplicate SQLite text index without a new workload.
- UI translations live in `crates/earthcraft-core/locales/{en,pt-BR}.ftl`.
  Add the same semantic ID and named variables to both catalogs. Use `Lang::text`
  for static labels and `Lang::format` for whole messages; do not assemble translated
  fragments in Rust. Preserve number precision and whitespace when changing message handling.
  Name IDs after the feature and message role, such as `settings-title`,
  `search-input-placeholder`, or `wayback-fallback-help`. Keep them concise and
  stable when wording changes. Do not generate IDs from translated sentences,
  repeat the feature name (`settings-settings`), or truncate prose into a key.
  Share an ID only when the meaning and usage match, not just the displayed text.
  See `docs/LOCALIZATION.md` for fallback, fixtures and contributor examples.

- GPU acceleration is optional: the CPU renderer remains available. Selection is opt-in
  (`EARTHCRAFT_RENDERER=auto|gpu`; CPU by default). Keep adapter initialization,
  texture uploads, compute dispatch, waits and readback on the frame coordinator.
  Retain a CPU fallback on initialization/readback/device errors. Close and exact-grid
  views use the double-precision CPU path; GPU sky/horizon/tile seams use precise
  CPU repair. Check GPU memory bounds, software adapters, device loss and render
  parity before widening hardware support. See `docs/GPU_RENDERER.md`.
  Changing egui's presentation backend alone does not accelerate rasterization.
- Performance claims need workload-specific evidence. Opt-in profiling uses a
  bounded background recorder; never write timing logs on UI/pixel paths, mix
  raster resolutions, or equate app-side texture submission with GPU execution.
  Test both default and profiling builds. Keep captures and temporary caches out
  of Git; retain concise aggregate evidence and report dropped events/outliers.

## Verification and reporting

Run from the repository root:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo build --workspace --locked
```

- Test production behavior, never a copied implementation. Keep automated tests
  independent of public map services, local user settings, and caches.
- Preserve the original-renderer fixtures; do not regenerate expected output merely
  to make a failing regression test pass. See the fixture README for provenance.
- For UI/input/render changes, also run the desktop smoke checklist in
  `docs/CONTRIBUTING.md`. Compiler success does not verify interactive behavior.
- Keep changes reviewable and explain boundaries, behavior changes, tests, and
  limitations. Never claim a platform, live service, packaging step, or GUI check
  passed unless it actually ran.
- Do not commit generated `target/` output, local settings, or unrelated user work.
  Stage explicit file groups. Commit/push/open PRs only when requested.
