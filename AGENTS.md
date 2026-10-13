# EarthCraft contributor and agent rules

Read `docs/ARCHITECTURE.md` and `docs/CONTRIBUTING.md` before structural changes.
Portuguese architecture notes are in `docs/ARCHITECTURE.pt-BR.md`.
User instructions and the requested scope take precedence over these defaults.

## Ownership and dependencies

- `apps/earthcraft` owns desktop startup, egui widgets, input, application state,
  texture upload, native dialogs, and background-result polling.
- `crates/earthcraft-core` owns geometry, camera operations, language selection,
  embedded localization, and placemark values. It may use `emath` for screen geometry; keep network,
  filesystem, egui widgets, and windowing out of it.
- `crates/earthcraft-data` owns tile services, search, boundary loading, and local
  persistence. It depends on core. Report results or invoke the injected repaint
  callback; do not add egui/eframe dependencies.
- `crates/earthcraft-render` owns CPU rasterization and tile request prioritization.
  It depends on core and data, never on the desktop application.
- Dependencies flow downward. Do not create cycles, import app code into libraries,
  or add a crate just to shorten a file. Use modules for related implementation.

## Making changes

- For refactoring tasks, preserve features, UI strings, defaults, keyboard/mouse
  behavior, persistence paths/formats, service URLs, timeouts, and rendering order.
  Feature requests may change behavior explicitly; document the difference.
- Put logic in the owning crate and surface drawing in the matching app UI module.
  Keep `main.rs` limited to startup. Avoid catch-all `utils` or `common` modules.
- Keep implementation private. Expose only APIs needed across modules/crates;
  use `pub(super)`/`pub(crate)` where possible. Keep synchronization internals private.
- Prefer named structures when tuples or positional arguments hide meaning.
  Add abstractions only for a concrete dependency boundary or multiple real uses.
- Use workspace dependency versions and shared package metadata. Keep `Cargo.lock`
  tracked. Do not update unrelated dependencies during a refactor.
- Rust 1.88 is the declared minimum. Avoid newer language/library APIs unless
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
  and floating-point evaluation order unchanged during a naming refactor.
- Leave obvious algebraic values (zero, one, halves, vector indices) inline unless
  they encode a format or policy. Avoid a global bag of unrelated constants.
- Use rustfmt. A module approaching 400 lines is a review signal, not a quota:
  split by responsibility, keeping related code and tests together.

## Library and resource practices

- Use glam for vector/matrix/rotation operations. Keep camera and geographic
  geometry in `f64` (`DVec*`, `DMat*`, `DQuat`); use SIMD `Vec4` for color work.
  Choose types for actual operations rather than replacing every scalar or tuple.
  Egui screen geometry stays in `emath`; do not add conversions without a use.
- Use safe bytemuck casts for byte-backed pixel layouts. RGBA arrays have byte
  alignment; a byte buffer is not automatically aligned for a SIMD vector. Keep
  channel order and padding explicit. Do not add unsafe casts or unsafe POD impls.
- Keep structured `earthcraft_data::Error` causes until the presentation boundary.
  Serde settings/service models must retain tolerant field defaults and existing
  coordinate order. quick-xml supports the existing KML Point subset.
- The app owns one `ServiceRuntime`. Inject its handle and repaint callbacks into
  data; use pooled reqwest clients and async I/O. Keep CPU rasterization on Rayon,
  large decoding/projection on bounded blocking workers, and UI access on egui's
  thread. Completion must wake the UI, including the last background job.
- Bound retained decoded tile bytes, not only tile counts. Preserve retry policy
  and disk fallback. Stream large GeoJSON feature collections; share immutable
  coordinate lines and request municipal data only when its layers are visible.
  Account for temporary references and GPU textures separately when measuring.
- UI translations live in `crates/earthcraft-core/locales/{en,pt-BR}.ftl`.
  Add the same semantic ID and named variables to both catalogs. Use `Lang::text`
  for static labels and `Lang::format` for whole messages; do not assemble translated
  fragments in Rust. Preserve number precision and whitespace during refactors.
  Name IDs after the feature and message role, such as `settings-title`,
  `search-input-placeholder`, or `wayback-fallback-help`. Keep them concise and
  stable when wording changes. Do not generate IDs from translated sentences,
  repeat the feature name (`settings-settings`), or truncate prose into a key.
  Share an ID only when the meaning and usage match, not just the displayed text.
  See `docs/LOCALIZATION.md` for fallback, fixtures and contributor examples.

## Verification and reporting

Run from the repository root:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo build --workspace --locked
```

- Test production behavior, never a copied implementation. Keep automated tests
  independent of public map services, local user settings, and existing caches.
- Preserve the original-renderer fixtures; do not regenerate expected output merely
  to make a failing refactor test pass. See the fixture README for provenance.
- For UI/input/render changes, also run the desktop smoke checklist in
  `docs/CONTRIBUTING.md`. Compiler success does not verify interactive behavior.
- Keep changes reviewable and explain boundaries, behavior changes, tests, and
  limitations. Never claim a platform, live service, packaging step, or GUI check
  passed unless it actually ran.
- Do not commit generated `target/` output, local settings, or unrelated user work.
  Stage explicit file groups. Commit/push/open PRs only when requested.
