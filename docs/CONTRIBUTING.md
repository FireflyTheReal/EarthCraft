# Contributing

Start with [the architecture](ARCHITECTURE.md) ([português](ARCHITECTURE.pt-BR.md))
and [agent rules](../AGENTS.md). Keep feature work separate from mechanical moves
so reviewers can distinguish behavior changes from file organization.

## Development

Install Rust 1.88 or newer through rustup. The locked dependency graph includes
packages requiring 1.88; the old README's 1.82 requirement was no longer accurate.
The workspace declares 1.88 and CI includes that toolchain. Stable is suitable for
day-to-day development; do not use APIs newer than the declared minimum.

On Linux, install build prerequisites before compiling:

```sh
sudo apt install build-essential pkg-config libgtk-3-dev libxkbcommon-dev libwayland-dev libx11-dev
```

From the repository root:

```sh
cargo run --release
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo build --workspace --locked
```

The default member is the desktop app. Plain `cargo test` does not run every
library's unit tests: use `cargo test --workspace`. For focused changes:

```sh
cargo test -p earthcraft-core
cargo test -p earthcraft-data
cargo test -p earthcraft-render
cargo run -p earthcraft
```

Keep `Cargo.lock` in version control. Normal verification uses `--locked` to
catch unintended dependency resolution changes. Intentional dependency updates
should be reviewed separately, including compatibility with Rust 1.88.
CI formats, lints, tests, and builds on Linux/macOS/Windows, plus Linux with the
minimum Rust version. GitHub Actions are pinned to commits; update the pins
explicitly when updating CI tooling.

## Tests and behavior preservation

Camera tests call the same methods as the app. Boundary and search tests use
local JSON/strings. KML tests use isolated temporary files. Raster regression
fixtures were captured from the original implementation, including empty-cache
and loaded-tile rendering; see [fixture provenance](../crates/earthcraft-render/tests/fixtures/README.md).
Tests require no public map service or saved user configuration. HTTP fixtures
bind an ephemeral loopback port; they exercise response limits, retries and disk
cache behavior. Fluent tests verify catalog syntax, matching keys/variables and
exact original UI output. See [localization](LOCALIZATION.md) before editing text.

Prefer tests of observable behavior and invariants. Avoid copies of production
algorithms. For a behavior-preserving refactor, retain constants, coordinate
order, persistence formats and UI strings. Do not regenerate regression fixtures
unless a deliberately approved rendering change explains the difference.

The automated tests do not validate live services, GUI event delivery, native
file dialogs, or packaging. Run the following manual checks for affected areas:

- Launch from the root and verify the icon, initial view and panels.
- Orbit, zoom under the cursor, rotate with right-drag/Shift, use arrow keys,
  compass/north reset, navigation widgets, double-click and fly-to.
- With background rendering, open Settings during continuous rotation/flight;
  verify input remains responsive, new imagery keeps arriving and borders/labels
  stay aligned. Stop input and check full-resolution recovery. Rapidly switch
  providers/effects and resize to check obsolete results, then close while busy.
- At the initial distance, let the satellite overview warm, then rotate to the
  opposite hemisphere. Check that overview imagery is available, close zoom
  still loads detail, and other layers remain on demand. Compare cold and warm
  disk-cache startup separately; do not promise immediate cold-network readiness.
- In an isolated cache directory, use Settings → Clear on-disk cache. Check
  progress/completion, retained imagery in memory, preservation of settings and
  boundary caches, and that old queued downloads do not restore deleted files.
  Check the usage readout beside the button before clearing and its refreshed
  value afterward; it counts encoded tile files, excluding filesystem overhead.
  Verify that fresh visible misses and a later startup can cache tiles again.
- Switch satellite/streets/topography/historical imagery; verify loading,
  release selection, fallback and attribution.
- Search by coordinates and address; select suggestions with mouse and keyboard.
- Toggle borders, city/metropolitan layers, labels, grid, atmosphere and sunlight.
- Create placemarks; save/open/drop KML; export a screenshot and use the ruler.
- Switch English/Portuguese, change UI/map scales, then restart to verify persistence.

Report which checks actually ran and which platforms/services remain unverified.
For packaging, use the app manifest/package, e.g. a package-selecting command
supported by your packaging tool or run it from `apps/earthcraft`. Release
artifacts still go to the root `target/release`. The pre-existing missing
`icon.icns` and `packaging/earthcraft.desktop` assets must be supplied before
validating bundle/deb output; this refactor does not invent replacements.

## Resource-constrained builds

For a small disk or a quick local check, reduce generated debug information and
incremental output without changing the shared development profile:

```sh
CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_DEV_INCREMENTAL=false cargo test --workspace --locked
```

On PowerShell, set those names with `$env:CARGO_PROFILE_DEV_DEBUG="0"` and
`$env:CARGO_PROFILE_DEV_INCREMENTAL="false"`, then run Cargo normally.
`cargo clean --profile dev` removes rebuildable development output. Never delete
user caches, screenshots, KML, or unrelated files to make a build fit.

## Review checklist

Explain the concrete behavior affected, why the owning module is correct, and
what ran. Keep imports explicit and public APIs small. Prefer a module to a new
crate when code shares the same responsibility and dependency requirements.
Update architecture docs when ownership or dependency direction changes. Update
both READMEs when changing user-facing setup or feature descriptions. Stage
explicit files and keep generated output out of commits.

## Library refactors and memory measurements

Use separate commits for each library migration; group dependencies only when
needed for one boundary (for example Tokio/reqwest or Fluent's supporting crates).
Verify the affected invariants before committing, then run workspace checks.
Use glam's double precision for geometry and SIMD float vectors for colors;
avoid alignment assumptions when using bytemuck with pixel buffers.

For memory comparisons, use the same build profile, viewport, settings, cache
state, enabled layers and elapsed startup time. Record which memory metric was
used. Resident memory and Activity Monitor's memory column are not interchangeable.
Moka's 256 MiB target is weighted by decoded tile bytes and enforced best-effort;
brief overshoot is expected. It does not limit total application memory, and its
weighted-size estimate can lag inserts/evictions. Keep retry/deduplication tests
independent of cache maintenance, and do not add per-frame eviction flushing.
Municipal geometry is deferred rather than removed. Exercise enabling both
municipal and metro layers after startup, and verify that loading labels finish.

For a focused cache lookup comparison (optimized dev profile):

```sh
CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_DEV_INCREMENTAL=false cargo test -p earthcraft-data cached_hit_throughput --locked -- --ignored --nocapture
```

This ignored manual benchmark reports median cached-hit throughput over five
rounds with 256 warm tiles and one/eight threads. It calls production lookup code;
there are no timing assertions. It excludes misses, downloads, eviction pressure,
full rendering and texture upload. Do not equate its numbers with application FPS.

To compare complete warm CPU frames using isolated disk fixtures:

```sh
CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_DEV_INCREMENTAL=false RAYON_NUM_THREADS=8 cargo test -p earthcraft-render warm_cpu_frame_time --locked -- --ignored --nocapture --test-threads=1
```

Run this benchmark alone: it temporarily selects a private cache directory before
creating worker threads, then restores the process environment and removes that
directory. It renders 512×384 frames with grid, sunlight and atmosphere, using 341
warm two-pixel fixture tiles. Startup warming uses production admission and disk
workers, and rotations across hemispheres, the dateline and poles assert that
overview frames request no new tiles. Five rounds of two hundred frames include production ray
casting, sampling, allocation and tile lookup; they exclude GUI upload, overlays,
network and eviction pressure. Neither benchmark is a substitute for live-app
frame-time and memory measurements.
