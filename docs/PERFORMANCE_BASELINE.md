# Native CPU baseline, 2026-10-13

One macOS ARM64 native run used the optimized development profile (`opt-level=2`,
no debug information/incremental output), eight Rayon threads and an isolated
cache. Cached geographic data files and z0–4 satellite files were copied;
the real user cache/settings were not changed. Live detail downloads and lazy
municipal loading remained enabled. Compilation/checks finished before measured
phases. This is an instrumented observational baseline, not a controlled FPS
benchmark or a cross-platform result.

| Phase / operation | Samples | Median | p95 |
| --- | ---: | ---: | ---: |
| Warm rotation: 808×601 raster | 1,619 | 5.812 ms | 20.064 ms |
| Warm rotation: overlay preparation | 1,628 | 7.572 ms | 13.920 ms |
| Warm rotation: request-to-UI delivery, 808×601 | 1,505 | 24.798 ms | 64.656 ms |
| Warm rotation: app UI update | 2,836 | 0.605 ms | 1.339 ms |
| Flight/detail loading: 2000×1488 raster | 36 | 64.282 ms | 133.633 ms |
| Flight/detail loading: app UI update | 588 | 0.189 ms | 0.489 ms |
| Flight/detail loading: decode queue wait | 442 | 0.024 ms | 3.639 ms |
| Flight/detail loading: feature queue wait | 28 | 0.024 ms | 5.843 ms |
| Municipal/metropolitan loading: index build | 2 | 584.466 ms | 633.301 ms |
| Municipal/metropolitan loading: app UI update | 67 | 0.414 ms | 2.944 ms |
| Boundary-heavy close rotation: 808×601 raster | 1,277 | 16.180 ms | 187.076 ms |
| Boundary-heavy close rotation: app UI update | 3,218 | 0.215 ms | 0.922 ms |
| Boundary-heavy close rotation: decode queue wait | 4,027 | 0.186 ms | 10.460 ms |

Phase lengths were about 43 seconds warm rotation, 53 seconds flight/detail
loading, 79 seconds stationary boundary loading, and the recorded interval in
the JSON for close rotation. The coordinate flight targeted São Paulo at 3 km.
Country/state/name layers stayed on; municipal/metropolitan layers were enabled
for the final two phases. Automatic rotation at close zoom is a tile-churn stress
case, not a typical manual pan. World warming did not eliminate higher-detail
requests. Eleven of the 28 feature jobs overlapped decode work during the flight
phase, exercising concurrent services; the later stationary boundary phase had
no active tile requests.

Timed tile-admission lock acquisition rounded below 1 microsecond at p95; admission
itself stayed below 0.04 ms at p95 across the moving phases. This provides no
justification for DashMap in that measured path. It does not measure every cache
miss-path lock or rule out Moka/internal contention under different workloads.

The decoded-cache estimate reached 256 MiB during close rotation, with admission
bounded at 48. Four `ps` RSS snapshots were approximately 209, 52, 52 and 185 MiB;
an additional moving-view sample was 275 MiB. These are sparse resident-memory
observations, not total allocations, peak memory, or an equivalent of Moka's byte
estimate. They cannot establish a hard application memory ceiling.

Outliers matter: the stress phase had a 1.267-second active-resolution raster
maximum, 4.282 seconds among 25 full-resolution rasters, and a 497 ms app-update
maximum. Warm rotation also had large spikes. No causal attribution of those
outliers was established; scheduling, loading and memory pressure are not held
constant. Short typical UI updates do not establish uniformly smooth input.

## Interpretation

Full-resolution raster work remains a substantial cost. World-view overlay
preparation is also significant, and large dataset changes rebuild indexes. Keep
those as distinct optimization candidates.

The report measures app update wall time, not egui's later tessellation, actual
driver texture upload, GPU execution, screen presentation or OS input-to-display
latency. `frame.delivery` ends when the UI polls a completed frame. Profile events
can overlap across threads, so medians must not be summed into a frame budget.
No profile events were dropped. Capture adds instrumentation overhead.

Aggregate data and phase intervals are in
[the JSON report](benchmarks/2026-10-13-native-profile.json). Reproduction instructions
are in [contributing](CONTRIBUTING.md). Raw capture for this run was retained at
`/tmp/earthcraft-native-profile.csv`, outside Git; temporary native cache files were
removed after the owned app closed cleanly.
