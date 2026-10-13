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

## Startup overview and renderer batching

Satellite zooms 0–4 now warm globally in the background while the app is idle at
the initial 14,000 km distance or farther away. The 341 standard tiles use about
85 MiB decoded within the shared Moka target, now raised to 256 MiB to retain
more detailed imagery after overview warming. Visible requests are admitted
first; warming keeps total in-flight work below two before admission, pauses
during interaction/closer zoom/other layers, and skips ready, loading or recently
failed tiles. Other providers remain on demand. Cold startup still downloads
progressively; higher-detail viewing and eviction can require additional loads.

CPU rendering shares tile handles within eight-row Rayon batches and performs
one local map lookup per sample. Handles are released at batch completion. Misses
retain their nearest pixel distance, and the redundant second request-deduplication
set is removed. Original pixel fixtures remain unchanged. New tests cover bounded
warmup, visible-request priority, whole-world coverage, skipped states and local
lookup reuse. The isolated disk-fixture benchmark also verifies that overview
rotations across both hemispheres, the dateline and poles request no new tiles.

Three alternating comparisons used the same optimized dev profile, eight Rayon
threads, 512×384 frame, 341 two-pixel fixtures, and five rounds of 200 frames:

| Comparison | Previous per-row cache | Eight-row batches |
| --- | --- | --- |
| 1 | 3.270 ms | 3.151 ms |
| 2 | 3.129 ms | 2.931 ms |
| 3 | 3.532 ms | 2.599 ms |

Values are median CPU-frame times, not application FPS. Individual samples were
noisy (roughly 2.3–5.7 ms); the consistent direction suggests a modest improvement,
not a guaranteed percentage. The benchmark excludes network readiness, GUI
upload, geographic overlays and eviction pressure. The increased round length
supersedes the earlier 20-frame procedure, not its historical observations.

## Memory observation

One fresh launch after the memory migration reported RSS of 199,632 KiB (about
195 MiB) through `ps`. This is a startup observation, not a controlled benchmark
against the earlier 812.2 MB Activity Monitor screenshot. Different memory metrics,
settings, elapsed time and enabled layers cannot establish a percentage reduction.
The current decoded-tile capacity target is 256 MiB and may overshoot temporarily;
geometry, request bodies, temporary
renderer references and GPU resources remain separate allocations. Use the
comparison procedure in [contributing](CONTRIBUTING.md) for a repeatable measurement.

## Next performance stages

### Current remaining order

The post-SIMD review and geographic-index discussion refine the remaining order:

1. **Completed:** Align globe picking with the displayed frame's camera, matching the existing
   overlay alignment. Cover input while a newer camera frame is still rendering.
2. **Completed:** Isolate expensive boundary parsing and tile decoding so long feature jobs do
   not occupy all blocking capacity needed by interactive tile I/O. Measure queue
   wait and responsiveness with both workloads active.
3. **Completed:** Avoid raster rebuilds for unrelated tile completions and overlay-only changes.
4. **Completed:** Reuse frame buffers and texture storage while preserving screenshot ownership.
5. **Completed:** Improve stale-view tile request scheduling without changing retry, admission
   limits or coarse-tile fallback.
6. **Completed:** Use `geo-index` for a static boundary-chunk index and conservative geographic
   visibility culling before vertex conversion/projection. Benchmark construction,
   memory and moving-view queries against a bulk-loaded `rstar` index and the
   existing traversal. Keep mutable `rstar` for priority-ordered label occupancy.
   Preserve visible segments across the horizon, poles and dateline; rejecting
   chunks solely because their endpoints are invisible is insufficient.

`geo-index` now implements the static boundary initiative. A separate k-d tree
is not planned without a concrete point-query workload. Unicode normalization is complete.
MBTiles, conditional DashMap evaluation and the GPU/version stages remain below.
Keep each initiative in its own logical commit.

Commit and inspect the overview/batching build before starting these approved
stages. Keep changes in separate logical commits and measure affected workloads:

1. **Completed:** Move whole-frame CPU rendering off egui's thread, retaining Rayon pixel
   parallelism. Keep the latest pending camera request and reject obsolete
   provider/style/size results. Present compatible completed camera snapshots
   during motion, with matching overlay projections, to avoid starving the
   display. Perform texture upload on egui's thread.
2. **Completed:** Cache unchanged overlay projections. Evaluate `rstar` for screen-space label
   overlap queries while preserving placement priority and visible labels.
3. **Completed:** Use `wide` to batch independent ray calculations across pixels, preserving
   double-precision geometry and original raster fixture tolerances.
4. **Completed:** Reuse frame buffers and reduce conversion/copy work while preserving screenshot
   output and explicit buffer ownership across workers.
5. **Completed:** Separate tile decoding from long boundary parsing so the bounded blocking pool
   does not delay interactive tile loads; avoid unmeasured thread proliferation.

Replace the manual accent table with Unicode normalization in its own library
commit, testing existing Portuguese matching and international/decomposed names.
Only after these CPU and data stages are complete and validated, begin a wgpu
globe renderer. Switching the UI backend alone does not move ray casting to the GPU.

Treat cache clearing and its disk-usage readout as one cache-controls change.
Each rendering, spatial-index, SIMD, buffer, decoding and Unicode initiative gets
its own logical commit; group supporting dependencies only when synergistic.
After those higher-priority CPU/data optimizations, migrate persistent tiles to
SQLite-backed MBTiles in a separate change, retaining the clear button and usage
readout. Evaluate DashMap afterward only if request-lock contention measurements
justify changing admission coordination; it does not replace the decoded Moka cache.
After the complete series, including the later GPU renderer, passes validation,
bump the workspace version to 0.1.0 and regenerate the lockfile. The version
remains 0.0.2 during these intermediate stages.

## Background frame coordinator checkpoint

Stage 1 now runs whole-frame CPU rendering on an app-owned coordinator with the
existing Rayon pool. The mailbox coalesces pending camera input and retains only
one completed frame; provider/style/size changes reject incompatible output.
Compatible camera snapshots remain presentable during continuous motion, and
overlays/scale use their camera to stay aligned. Texture upload and screenshot
buffer ownership stay on egui's thread. Overview warming waits until visible
tile admission finishes; closing drops queued work without waiting for the raster.

Four deterministic scheduler tests cover background execution, coalescing 200
camera requests, presentation filtering, completed-buffer replacement, completion
repaint and non-blocking shutdown. All 51 workspace tests, strict Clippy, formatting
and the macOS ARM64 optimized-dev build passed; original raster fixtures are
unchanged. Native checks opened Settings during continuous rotation, toggled the
grid and switched topography/satellite. Exported PNGs were 808×601 while moving
and 2000×1488 after idle recovery; imagery and geographic overlays were inspected.
The isolated native window also closed successfully during rotation.
These checks establish scheduling and behavior, not a quantified FPS improvement.
Overlay caching/spatial indexing, SIMD, buffer reuse, decoding isolation and
Unicode normalization remain separate pending stages before MBTiles.

## Overlay cache and label index checkpoint

Stage 2 caches projected administrative borders and placed label shapes for one
displayed camera basis. Boundary patches, viewport/DPI, altitude, layer flags,
language and map scale invalidate it; tile completion and unrelated repaints do
not. Ruler and placemarks remain dynamic. `rstar` 0.13.0 replaces accepted-label
rectangle scanning with inclusive, short-circuit R*-tree queries, preserving
stable priority/rank order, padding and the 500-label limit. Its declared Rust
minimum is 1.85, within the workspace's 1.88 requirement. Existing dependency
versions remain locked; only its supporting packages/features were added.

Six new tests cover reuse and invalidation, priority/rank/stable ties, border
endpoints, the label cap, touching edges/containment and 20,000 deterministic
rectangle decisions against egui intersection. All 57 workspace tests, strict
Clippy, formatting and the macOS ARM64 optimized-dev build passed. Original
raster fixtures remain unchanged. In the isolated native build, stationary
label/border toggles updated immediately and a newly added placemark appeared
without moving the camera. Borders/labels were visually inspected against the
previous build and during rotation; this is not a pixel-exact native screenshot
comparison or a full cross-platform/input smoke test.

The manual synthetic benchmark reported medians of seven rounds:

| Workload | Previous approach | Current approach |
| --- | --- | --- |
| 8,000 label rectangles, 1920×1080 area, 500 admitted | Linear: 0.367 ms | R-tree: 0.682 ms |
| 8,000 label rectangles, 960×600 area, 255 admitted | Linear: 1.215 ms | R-tree: 1.293 ms |
| 10,000 32-point lines + 8,000 labels, unchanged view | Rebuild: 15.044 ms | Reuse: 0.120 ms |

The R-tree is retained as the library implementation of screen-space occupancy;
it was slower than a flat scan at these small capped sizes. Projection caching
provides the measured improvement. A trial with wider tree nodes increased
insertion/query cost and was reverted to library defaults. Measurements exclude
rasterization, tessellation, GPU upload/drawing and services, and are not FPS
claims. Commands and workload limitations are in the contributing guide.

Next is the separate `wide` ray-batching stage, then buffer reuse, decoding
isolation and Unicode normalization before MBTiles and the later GPU renderer.

## GeoRust KML checkpoint and geographic evaluation

GeoRust `kml` 0.14.0 now owns KML reading and writing, with default ZIP/geo-types
features disabled. The entry point lives in `crates/earthcraft-data/src/kml/mod.rs`,
with private `guard` and `tests` modules. There is no legacy parser fallback.
A quick-xml preparation pass skips empty Points, limits recursive depth, closes
incomplete outer containers and wraps legacy bare Placemark coordinates. This
prevents the published reader's demonstrated empty-Point panic and unclosed
Placemark EOF loop while retaining the original fragment/roundtrip fixtures.
Coordinate decoding, namespaces, text/entities, metadata and serialization belong
to the library. CDATA is literal and invalid coordinates/metadata return typed
KML errors. The app remains a marker importer/exporter; no line/polygon or KMZ
feature is added. Only kml was added to the package graph; sharing quick-xml 0.41
removed 0.38 without unrelated dependency upgrades.

Eight KML tests cover original imports/roundtrips, namespaced nested folders,
localized names/order, empty Points with valid siblings, Unicode/entity whitespace,
CDATA, first-coordinate marker projection, typed failures, incomplete containers
and bounded nesting. All 63 workspace tests, strict Clippy, formatting and the
macOS ARM64 optimized-dev build passed; the three manual benchmarks remain ignored
by default. Original raster fixtures are unchanged. Native KML dialogs and the
minimum-toolchain/other-platform builds were not rerun for this checkpoint.

An isolated comparison of published geo 0.33.1 did not demonstrate faster
compatible math: distance adapters were close, and generating our fixed-segment
DVec3 arcs was substantially slower. Existing production math stays in place.
See [geospatial evaluation](GEOSPATIAL.md) for the measurements, compatibility
checks and proposed roles of geozero, MVT and geo-index. Those remain evaluation
options without runtime dependencies; they do not replace the approved next
wide/buffer/decoding/Unicode stages or silently change data sources.

## SIMD ray checkpoint

`wide` 1.7.1 now rotates, normalizes and intersects four independent `f64` rays
at a time inside the existing eight-row Rayon batches. Geographic conversion,
LOD selection, tile sampling and presentation retain their existing behavior.
Each frame captures the camera transform once; short final groups shade only
real pixels and do not generate padded tile requests. No approximate reciprocal,
trigonometric helper or reduced geographic precision was introduced.

Rust 1.89 is the new declared minimum, matching wide's requirement. CI and setup
documentation use that minimum; unrelated dependency versions remain locked.
The audit also removed core's unchanged glam/Fluent re-exports: imports now use
the owning package directly, with this rule recorded in AGENTS.md.

Original raster fixtures remain unchanged. Additional fixtures were captured
from the pre-SIMD renderer at `c31bfa8` for six partial-row/provider/pole/dateline
and altitude scenarios. They compare RGBA output within the original one-byte
tolerance and missing-tile keys/priorities exactly. Camera-ray invariants cover
both altitude limits and narrow/odd image dimensions. All 65 workspace tests,
strict Clippy, formatting and the macOS ARM64 optimized-dev build passed.

`cargo +1.89.0 check --workspace --all-targets --offline --locked` also passed on
macOS ARM64, verifying the new minimum locally. The isolated native build exercised
grid, sunlight and auto-rotation controls, opened Settings during rotation and
exported a 2000×1488 PNG. Imagery, grid, day/night shading, atmosphere and stars
were inspected in that export. This is not a complete native-input, live-provider
or cross-platform smoke checklist.

Three alternating comparisons used the same compiler/profile, eight Rayon
threads, 512×384 frame, 341 warm 2×2-pixel fixtures and five rounds of 200 frames:

| Comparison | Previous scalar rays | Four-lane rays |
| --- | --- | --- |
| 1 | 2.796 ms | 2.592 ms |
| 2 | 2.615 ms | 2.731 ms |
| 3 | 3.056 ms | 2.539 ms |

The direction is mixed and samples are noisy; this is not a reliable improvement
percentage or application FPS claim. The tiny texture working set excludes
realistic tile-memory pressure, GUI conversion/upload, geographic overlays,
network readiness and eviction. The earlier single-pass result is insufficient
to establish a general speedup. SIMD geometry remains covered by production
fixtures; realistic end-to-end measurement is a separate evaluation step.

## CPU scheduling and geographic culling checkpoint

Displayed-frame picking, separate two-thread feature/decode services, relevant
missing-tile redraws, buffer reuse and texture sub-updates, and current-view tile
queue priority are implemented in separate commits. Focused regressions cover
pending-camera picking, saturated feature workers, pending tile tracking,
complete buffer overwrite/allocation reuse, obsolete admission and queued work
retirement. Active downloads, retry timing and coarse fallback remain intact.

Static boundary lines now use `geo-index` 0.3.4, with conservative interpolation
bounds and horizon/frustum rejection before projection. Mutable label occupancy
remains in rstar. Pole/dateline/horizon tests sample line interiors across headings
and both altitude extremes; source order and existing overlay fixtures survive.
On the synthetic 10k 32-point-line workload, one local optimized-dev run measured
12.612 ms for bounds plus packed-index construction (rstar index-only: 5.387 ms).
Mean moving-view queries over 360 views were 0.012 ms for geo-index plus planes,
0.029 ms for a linear plane scan and 0.031 ms for rstar plus planes. Candidate sets
matched. The packed tree occupied 362,754 bytes, excluding IDs/3D bounds. These
are synthetic index measurements, not full overlay or application FPS results.

Unicode name matching now uses canonical decomposition and combining-mark
removal from `unicode-normalization`, followed by full lowercase expansion and
alphanumeric filtering. Existing Portuguese matching is preserved; decomposed
names, Vietnamese, Greek and Turkish accents are covered, while Cyrillic/CJK
letters remain in their own scripts. Display names are never rewritten.

## Ownership audit checkpoint

Overlay geometry, geographic culling, stable label candidate ordering and R-tree
occupancy now live in `earthcraft-render::overlays`, with emath rather than egui.
The bounded frame coordinator also moved into render. The app retains font
measurement, colors/strokes, egui shape caching, input, texture ownership and
repaint/event wiring. Camera flight interpolation moved to core; local suggestion
ranking/deduplication/merging moved to data. Existing overlay/worker tests moved
with their production algorithms, while egui shape tests stay in the app.
Agent rules record these boundaries for future additions.

Ownership validation: 76 workspace tests passed (four manual benchmarks ignored),
strict workspace Clippy, formatting and the macOS ARM64 build passed. Native
rotation showed aligned imagery/borders/labels, Settings opened during rotation,
and the isolated window closed normally. Full mouse input, live-provider,
file-dialog and other-platform checks are still separate. The shutdown test now
waits for channel disconnection rather than racing the capture destructor order.

## Background file jobs

KML open/drop/save and PNG encoding now run through a data-owned ordered service,
with one file worker and at most four pending jobs. Save/export jobs capture their
inputs before execution; admission is checked before cloning snapshot data.
Completion wakes egui, which applies imported marks, flight targets, paths and
localized statuses. Native dialogs remain app-owned. Feature parsing and tile
decoding use their separate pools. A local test covers ordered save/open, edits
after submission, missing-file errors, PNG content and completion wakeups.

## MBTiles checkpoint

Persistent tiles now use MBTiles 1.3 with tokio-rusqlite 0.8 and rusqlite 0.40
(bundled SQLite). One background connection worker attaches separate tilesets
for providers, stable Wayback releases and PNG/JPEG formats. XYZ-to-TMS rows,
metadata, encoded payloads, lazy legacy migration and reopen reads are tested.
Overview plus visited detail use the user-selected 128 MiB managed-file target;
eviction counts SQLite pages/metadata, prefers retaining the global overview,
and reclaims file bytes through full auto-vacuum. Clearing still preserves RAM
and other data, rejects pre-clear writeback, and refreshes actual usage. Decoded
tiles publish before persistence; corrupt tile bytes are invalidated for refetch.

The bundled R-tree and FTS5 modules are available and tested without loading
extensions. They belong in a separate geometry/place catalog if needed, not the
MBTiles imagery schema. No Serde row-mapping dependency is needed for key/BLOB
queries. Native cache usage and clearing were exercised while imagery remained
available. Rust 1.89 all-target checks, workspace regressions, strict Clippy and
the macOS ARM64 build passed. Other platforms and native KML dialogs remain
unverified for this checkpoint.

## Loaded-name prefix index

`fst` 0.4.7 now builds compact immutable prefix automata for both loaded name
languages on the bounded feature CPU service. Pending dataset rebuilds coalesce,
only a matching generation is used, and the original scan remains a fallback
while rebuilding. Prefix queries normalize once rather than folding every loaded
name. Source rank/kind/UTF-8 length/stable-order ties, normalized duplicates and
remote merging are unchanged; a four-entry standard-library heap avoids sorting
all hits for broad prefixes. Geometry arrays are not copied by this index.

One synthetic 16k-name run measured 19.431 ms construction off the UI thread,
4.637 ms/query for the original scan versus 0.118 ms/query for FST across broad,
narrow and missing prefixes. The two maps occupied 143,470 bytes, excluding the
immutable label snapshot. This is a component result, not an FPS guarantee.
Tests compare complete suggestion records against the existing algorithm for
both languages, composed/decomposed accents, Cyrillic/CJK, ranks and duplicates,
and verify latest-generation background publication and repaint.

Prefix validation also passed Rust 1.89 all-target checks, workspace regressions,
strict Clippy and the macOS ARM64 build. Native typing of `sao` displayed the
expected accented local names, including São Tomé and São Paulo. No new provider
or language was added; public service behavior and cross-platform input remain
separate checks.
