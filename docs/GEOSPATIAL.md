# Geospatial library evaluation

Keep glam for camera matrices, rotations and renderer vectors. Geographic
libraries can complement it for geometry operations, formats and indexes; they
are not automatically faster replacements. Use upstream APIs and fixtures for
validation without making every reference implementation a runtime dependency.
If adapting upstream code, retain its license and attribution.

## Geo comparison

A local, isolated evaluation used published `geo` 0.33.1, EarthCraft's production
core functions, the optimized dev profile (`opt-level = 2`, no debug information),
and the same spherical Earth radius. It did not add geo to the workspace.

Seven-round medians were repeated in three alternating-order passes on macOS
ARM64. The distance workload evaluated 262,144 deterministic global coordinate
pairs per round. Great-circle work evaluated 1,024 arcs with 128 segments each,
including conversion back to the `DVec3` output required by our drawing code.

| Work | EarthCraft | Geo adapter |
| --- | --- | --- |
| Spherical distance, with matching call boundaries and a NaN fallback | 7.73–7.93 ms | 7.83–8.16 ms |
| Fixed-segment great-circle vectors | 1.04–1.12 ms | 18.28–18.55 ms using ratio interpolation |
| Distance-spaced great-circle vectors | Same fixed-segment baseline | 8.74–9.42 ms using bulk interpolation |

The initial inlined distance adapter appeared slightly faster; including the
compatibility fallback and comparable function calls removed that apparent win.
These small distance differences are not a demonstrated performance improvement.
Geo's bulk interpolation also produced a different point count for 946 of 8,192
sample arcs; it cannot replace our fixed-segment contract directly. Ordinary
finite distance outputs differed by at most about 4e-11 km in those samples.

Use core's spherical math. Compare future geographic operations individually,
including dateline/pole/antipodal behavior, rather than importing geo wholesale
or assuming SIMD from a vector type. These measurements are component timings,
not application FPS, and should be repeated for an actual proposed adapter.

## KML

Published `kml` 0.14.0 owns decoding and writing. Default features are disabled:
EarthCraft does not need its geo-types conversions or ZIP/KMZ support. Sharing
quick-xml 0.41 with the already-locked dependency graph removes the duplicate
0.38 version. quick-xml declares Rust 1.79; kml 0.14 is edition 2021 and does not
publish a Rust minimum. Minimum-toolchain coverage uses the Rust 1.89 CI job.

An isolated upstream probe demonstrated an empty-Point panic and an unclosed
Placemark that did not finish within two seconds. The child process was terminated.
The private XML preparation pass removes Points without coordinate text, bounds
nesting, completes missing outer-container endings and wraps legacy bare
Placemark coordinates. Truncated fields and mismatched tags produce errors.
KmlReader then decodes the entire prepared document; there is no custom fallback
KML parser. Empty Points are skipped, including inside MultiGeometry, without
losing valid siblings.

The app keeps its marker model and localized fallback names, longitude-first
KML coordinates, document name/namespace, XML declaration and zero-altitude
exports. Imported lines/polygons/collections still yield a marker at their first
coordinate. No line/polygon display or KMZ feature is added. Library semantics
replace ad hoc parsing: CDATA stays literal and malformed coordinates or metadata
produce typed errors instead of filtering numeric tokens or ignoring those errors.
The serializer includes explicit default extrusion/altitude-mode fields; those
have the same meaning as the previously omitted defaults.

## Geozero and vector tiles

[Geozero](https://github.com/georust/geozero) provides processor callbacks and
adapters for GeoJSON, MVT and other formats. A processor could write directly
into our shared line/label structures, avoiding an intermediate geometry model.
However, a zero-copy processing API does not guarantee a streaming input path:
the reviewed 0.15.1 general GeoJSON reader reads the whole document into a string.
Its explicit collection reader uses GeoJSON FeatureReader. Preserve our metadata,
empty-array and truncated-envelope fixtures when evaluating that path; our
current Serde collection visitor already processes one feature at a time.

[Mapbox Vector Tiles](https://github.com/mapbox/vector-tile-spec/tree/master/2.1)
encode tiled geometry and feature properties. They could stream borders, roads
and place labels at appropriate zoom levels; satellite imagery remains raster.
Geozero supports MVT decoding. This is a separate source/rendering initiative:
choose a provider/schema first, preserve the required names/ranks, transform
extent-scaled tile-local coordinates to globe coordinates, define styling and
handle clipped features/duplicate labels across tile boundaries. The format
alone supplies neither a provider nor a map style. A switch must not silently
replace current data or change layer coverage.

MVT can be stored in MBTiles as `pbf` tile data. Imagery persistence uses MBTiles independently; do not treat vector tiles as an interchangeable payload
for the current RGBA sampling path.

## Geo-index

[Geo-index](https://github.com/georust/geo-index) packs immutable two-dimensional
R-trees and k-d trees into contiguous buffers. Its strongest candidate here is
an index built once per loaded administrative dataset, to select border segments
and label candidates before projection. Benchmark build cost, retained bytes,
query time and complete overlay rebuilds against scanning our own datasets.

Its queries are planar: geographic visibility still needs conservative globe
culling, antimeridian handling and horizon/pole tests. Avoid dropping visible
features through a naive longitude bounding box. Rebuild/swap the dataset index
when boundary patches arrive. Keep rstar for the screen-label occupancy tree,
which accepts rectangles incrementally in priority order. Static dataset indexes
and dynamic label placement solve different jobs.

Geo-index is now installed for static boundary-line visibility in the render
crate. Conservative interpolation bounds, horizon/frustum rejection and tests
for visible interiors at poles/dateline preserve the current projection.
The synthetic 10k-line comparison measured 12.612 ms for bounds/index construction
and 0.012 ms for moving-view queries, versus 0.029 ms for a linear plane scan and
0.031 ms for rstar plus planes. Candidate sets matched; retained packed-tree bytes
were 362,754, excluding IDs/3D bounds. These are component measurements, not FPS.
Geozero and MVT are evaluation options without runtime dependencies.

## Current SQLite and vector-source decision

Imagery persistence now uses standard MBTiles with core SQLite tables, through
one tokio-rusqlite worker. Bundled SQLite's R-tree and FTS5 modules are available
and tested. Use them for a separate persistent geometry/place catalog when a
query workload needs them: R-tree for bounding-box candidates, FTS5 for token,
phrase and richer text search. Exact imagery keys already use the MBTiles B-tree;
per-frame visibility uses the packed in-memory geo-index tree, and loaded-name
prefix lookup uses an immutable FST. Duplicating these indexes would add memory
and maintenance without changing the current prefix-only search feature.
`serde_rusqlite` remains a useful option for that future typed catalog; current
small key/BLOB queries use explicit mappings and do not need it.

[OpenFreeMap](https://openfreemap.org/quick_start/) is a concrete vector-source
candidate. Its [OpenMapTiles schema](https://openmaptiles.org/docs/schema/)
supports tiled administrative lines and named places. This establishes format
feasibility, not feature parity with our current Natural Earth/global-label and
IBGE municipal/metropolitan datasets. In particular, tiled generalization and
clipped geometry, name/rank mappings, seam deduplication, and global autocomplete
coverage need validation; metropolitan membership still comes from IBGE.
Keep those layers until an adapter proves equivalent coverage and semantics.

Geozero is a good processor/decoder candidate for that future MVT adapter. The
current feature-at-a-time GeoJSON visitor is already streaming file input, so
adding another reader today is not an established optimization. A vector switch
needs a tiled data planner independent of raster sampling; it does not replace
satellite PNG/JPEG imagery. No MVT source or geozero runtime dependency was added
to the application.
