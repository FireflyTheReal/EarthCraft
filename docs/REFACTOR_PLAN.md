# Library and memory refactor checkpoints

Branch: `refactor/libraries-and-memory`, based on the workspace refactor PR.
The migrations preserve features, controls, persistence formats and original
renderer fixtures. Resource policy now bounds retained tile bytes and defers
expensive municipal loading until relevant layers are visible.

| Commit | Change | Verification |
| --- | --- | --- |
| `3325288` | glam double-precision vectors and SIMD color blending | Camera tests and original raster fixtures |
| `79a5e0b` | bytemuck zero-copy RGBA views | Renderer regression and strict Clippy |
| `7e8e951` | thiserror structured service failures | Error source/display test and workspace check |
| `028a90a` | Tolerant Serde settings/search models | Partial-invalid settings, defaults, clamping and invalid response rows |
| `2bd950c` | glam camera matrices, quaternion rotations and SIMD tile sampling | Camera invariants and unchanged original raster fixtures |
| `e379663` | quick-xml KML reading/writing | Roundtrip, CDATA, entities, fallback names and incomplete envelopes |
| `970d528` | Tokio/reqwest async services | Local HTTP response caps, errors, retries and cache fallback |
| `4303123` | Tile byte budget, shared lines, deferred municipal geometry and streaming GeoJSON | LRU accounting, oversized/failed tiles, lazy state transitions and streaming parsing |
| `f73e13e` | Embedded Fluent catalogs and whole-message arguments | Original wording/output, syntax, key/variable parity, references and fallback |
| `7ee7719` | Boundary completion repaint and active loading label | Completion/cancellation wake-up test and focused boundary tests |
| `9d399cb` | glam camera constructors and SIMD colors throughout rendering | Projection/picking invariants, tile sampling and original raster fixtures |

Each library checkpoint ran focused tests and strict workspace Clippy. Supporting
libraries are grouped when they implement one boundary; separate glam expansion
has its own commit. The early disk-space blocker was resolved before proceeding.

## Final local validation

- `cargo fmt --all -- --check`: passed.
- `cargo clippy --workspace --all-targets --offline --locked -- -D warnings`: passed.
- `cargo test --workspace --offline --locked`: all 37 tests passed after the Moka migration; two manual benchmarks are ignored by default.
- `cargo build --workspace --offline --locked`: passed on macOS ARM64.
- Final desktop executable launched without startup errors; imagery, borders, labels
  and geographic orientation were inspected in its native window.
- Native-target dependency metadata declares no Rust minimum above 1.88. Rust
  1.88 is not installed locally; minimum-toolchain and other-platform builds
  remain CI validation.

Local builds used `CARGO_PROFILE_DEV_DEBUG=0` and
`CARGO_PROFILE_DEV_INCREMENTAL=false`, retaining the existing optimized dev
profile. Cargo reports the existing transitive `block 0.1.6` future-compatibility
warning. Full manual controls, file dialogs, language switching and live-service
checks remain the contributor smoke checklist; launch alone does not verify them.

## Further glam audit

The camera now uses double-precision `dcamera` view and infinite perspective
constructors. RGB stays in SIMD `Vec4` through sampling, shading and output
clamping, with `lerp_monotonic` for blending. Four additional tests cover screen
pick/projection agreement across headings, poles, aspect ratios and altitude
limits; cardinal orientation; horizon/ray invariants; and tile interpolation.
Original raster fixtures remain unchanged. Great-circle interpolation and
axis/angle rotation retain their existing near-singular behavior.

## Moka migration

`moka::sync::Cache` stores shared decoded tiles with byte weights, the existing
packed-key hasher and explicit LRU eviction. The 128 MiB limit is now a best-effort
target with temporary overshoot, as accepted for this migration. Ready-cache reads
bypass the request mutex; loading/failure metadata preserves deduplication,
48 in-flight requests, 15-second retry timing and closed-queue accounting.
Tests cover eviction after maintenance, byte weighting, retries, concurrent
request deduplication and cached reads while request coordination is locked.
UUID is locked to 1.26.0 so the newly active dependency graph stays compatible
with the declared Rust 1.88 minimum.

Local optimized-dev measurements on macOS ARM64:

| Workload | Previous cache | Moka |
| --- | --- | --- |
| Cached hits, one thread, median of five rounds | 79.04 million/s | 14.08 million/s |
| Cached hits, eight threads, median of five rounds | 8.81 million/s | 4.46 million/s |
| Warm 512×384 CPU frame, eight Rayon threads, median of five rounds of twenty frames | 3.471 ms | 2.173 ms |

The lookup benchmark uses 256 warm 64×64 tiles and 50,000 lookups per thread.
The frame benchmark uses 341 two-pixel disk fixtures and production rendering,
including local per-row tile reuse, grid, atmosphere and sunlight. Neither includes
network, eviction pressure, GUI upload or live geographic overlays. Lookup-only
throughput regressed while this CPU-frame workload improved; these observations
are not a general FPS guarantee. No timing assertions were added. Commands and
benchmark limitations are in [contributing](CONTRIBUTING.md).

## Memory observation

One fresh launch after the memory migration reported RSS of 199,632 KiB (about
195 MiB) through `ps`. This is a startup observation, not a controlled benchmark
against the earlier 812.2 MB Activity Monitor screenshot. Different memory metrics,
settings, elapsed time and enabled layers cannot establish a percentage reduction.
The current decoded-tile capacity target is 128 MiB and may overshoot temporarily;
geometry, request bodies, temporary
renderer references and GPU resources remain separate allocations. Use the
comparison procedure in [contributing](CONTRIBUTING.md) for a repeatable measurement.
