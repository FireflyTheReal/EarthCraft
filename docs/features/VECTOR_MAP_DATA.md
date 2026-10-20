# Vector map data architecture

Status: **proposed**. This document records the decision to rebuild the
map-data subsystem while keeping `main.rs`, `earthcraft-core` and the render
engine as they are. It does not describe code that exists yet.

## Problem

The boundary/overlay path grew one feature at a time and now has three
properties that block the next round of work:

1. **Sources are baked into the runtime.** `boundaries/load.rs` knows the base
   URLs, the load order, the per-source timers and the national special cases.
   Adding a dataset means editing data, render, app, UI and settings.
2. **Updates are whole-dataset.** A patch replaces every label and vector,
   the overlay cache is keyed by `Arc` pointer identity, and any arrival
   invalidates the whole frame. There is no way to draw a newly arrived tile
   next to the previous detail level.
3. **Labels carry global identity.** `source_index` values and pointer
   equality are assigned per snapshot, so nothing survives a merge.

Those three together rule out independent tiles, and without independent tiles
there is no fade-in, no incremental load and no per-layer refresh.

Sizes at the time of this decision: `earthcraft-data/src/boundaries` 2,371
lines, `earthcraft-render/src/overlays` 1,401, `apps/earthcraft/src/app/overlays`
778. The rest of the workspace (camera, rasterization, GPU path, UI, packaging)
stays untouched.

## Scope

In scope: `earthcraft-data/src/boundaries`, `earthcraft-render/src/overlays`,
`apps/earthcraft/src/app/overlays`, and the registry-driven wiring in the app.

Out of scope: `main.rs`, `earthcraft-core`, the CPU/GPU rasterizers, the camera,
imagery providers, MBTiles persistence, packaging and the egui layout.

The crate contract survives: data publishes immutable snapshots, render consumes
geometry with `emath` coordinates and owns projection/visibility/occupancy, the
app owns widgets, textures and dialogs.

## Layer kinds

Three kinds exist from the start. Forcing every dataset through one pipeline is
what produced the current tangle.

| Kind | Examples | Model |
| --- | --- | --- |
| `TiledVector` | borders, places, roads, landcover | z/x/y tiles, independent draw and refresh |
| `WholeWorld` | weather, clouds, satellite, vessel overview | one small dataset per view, replaced whole |
| `PolarCap` | polar geometry beyond the projection limit | outside the tiled path |

`WholeWorld` layers stay on their current path. They are small, they change
often, and splitting them into tiles would add requests without adding quality.

## Tiling

- Key: `TileKey { layer, z, x, y }` in Web Mercator (EPSG:3857), TMS rows.
- Extent: MVT default 4096 units per tile edge unless a layer needs more.
- Dateline: features crossing the antimeridian are split at bake time; the
  runtime never straddles it.
- Poles: Web Mercator stops at ±85.0511°. Polar geometry is a separate
  `PolarCap` layer with its own projection, drawn before the tiled layers.
  A pole map is the long-term answer; a cap layer is the interim one.

### LOD and fade-in

Each layer declares a `Lod` range (`min_z`, `max_z`, `switch_z`). The viewer
requests the next level slightly before it becomes necessary and keeps the
previous level resident until the new tile decodes, so the transition is a
cross-fade rather than a blank.

This is the reason the subsystem is being rebuilt. It is also the first thing to
drop if it does not pay for itself: if cross-fading costs more frame time than
it buys in smoothness, fall back to a hard swap at `switch_z` and keep the tile
model. The tile model is the requirement; the fade is an optimization.

Pre-fetch is bounded. At most one level ahead, at most the tiles intersecting
the current viewport plus a small margin, cancelled when the camera moves.
The existing frame coordinator and repaint callback stay in charge of pacing.

## Sources: upstream first

Default policy: **consume upstream, ship as little as possible, host nothing.**

- Bundled with the app: a small `world-low` dataset (countries, first-order
  places) as plain GeoJSON, not a pack. The app must draw a correct globe on
  first launch, offline. See the spike results for the size.
- Fetched at runtime from the provider: everything else.
- Overture detailed divisions: upstream PMTiles at runtime. No offline slice is
  required unless per-tile size or provider limits prove otherwise.

The runtime therefore accepts three source kinds, not one:

| Source kind | Typical provider | Cost |
| --- | --- | --- |
| `Tiled` | anything serving PMTiles/MVT | a range request per tile |
| `FeatureCollection` | Natural Earth, geoBoundaries, national ArcGIS/REST | whole-response decode |
| `Raster` | imagery, weather, satellite | unchanged path |

`FeatureCollection` sources are the honest reason this is not "just tiles":
most public vector providers do not serve tiles. They serve what they serve and
the app transforms it. The national → geoBoundaries → OSM fallback chain that
exists today encodes real, verified provider behaviour and moves into the source
adapters rather than being deleted.

### Upstream tiled source (verified)

Overture publishes one PMTiles archive per theme per release:

```
https://overturemaps-extras-us-west-2.s3.us-west-2.amazonaws.com/tiles/<RELEASE>/<THEME>.pmtiles
```

Measured on release `2026-09-23.1`: the `divisions`, `places` and `base`
archives all answer HTTP range requests with `206 Partial Content` and
`Accept-Ranges: bytes`. Their full sizes are 19.0 GB, 17.5 GB and 181 GB
respectively, so a client must range-read individual tiles and never fetch an
archive.

Overture's own documentation states these tilesets exist for inspection and are
**not** a production cartographic basemap, and that the default profiles carry
all properties to aid inspection.

Measured tile payloads (release `2026-09-23.1`, read with a minimal PMTiles v3
reader over HTTP Range):

| Tile | Stored | Decoded MVT |
| --- | --- | --- |
| `divisions` z6 over Iberia | 43.1 KB | 272.4 KB |
| `divisions` z8 over Madrid | 13.5 KB | 60.2 KB |
| `divisions` z10 over Madrid | 14.3 KB | 60.7 KB |

`divisions` covers z0–z12 with 12,068,596 addressed tiles. A cold tile costs
four range requests (header, root directory, leaf directory, payload) and about
14 KB of directory bytes; the header and root directory are cached after the
first read, leaving two requests per tile.

`places` is **z14 only** — its lowest addressed tile id maps to zoom 14. It
cannot supply world-scale or mid-zoom labels, so Natural Earth places remain the
label source below street zoom.

### Consequences of hosting nothing

- First load of a detailed layer is slow. That is accepted for now.
- A provider changing a URL can break a release. Mitigation: manifest-driven
  source definitions with the existing fallback chain.
- Provider rate limits, outages and terms of use become runtime concerns.
- Attribution obligations are per layer and must be shown where the data is.

Revisit only if a layer proves unusable from upstream. The design must make
"add a Release-hosted pack" a manifest entry plus a URL, never an architecture
change.

## Format: PMTiles container, MVT payload

Interchange is **PMTiles + MVT**. Everything EarthCraft-specific that MVT is bad
at lives in a small sidecar.

### Why PMTiles/MVT

- Single file, byte-range reads, no server logic. Works from a static host.
- Per-tile compression and independent decoding, which the tile model needs.
- Zoom levels map directly onto LOD.
- Multi-layer tiles: one request can carry several layers at the same z/x/y.
- Tooling exists (tippecanoe, pmtiles, mvt crates), so bake and inspect are
  not our problem.
- Third-party and community packs become possible later without a new format.

### What MVT costs us

- **Quantization.** Coordinates snap to a tile-local integer grid (4096 by
  default). Labels drift and borders wobble if `z` is chosen carelessly. Tile
  z ranges must be picked from measurement, not guessed.
- **No exact doubles.** The globe path is double precision; the tiled path is
  not. Tiled layers must not feed camera or picking math.
- **Attributes are not semantics.** Label rank, kind, name bytes, dispute
  provenance, ISO codes and the FST name index have no natural home in MVT.
- **Poles and dateline** are outside the projection and must be handled before
  encoding.
- **Opaque on disk.** Debugging needs a decoder, unlike today's cached GeoJSON.
- **New dependencies.** Encoder/decoder must stay off the UI thread.

### Why not a fully custom format

A tuned format would store our exact fields, mmap cleanly and skip attribute
encoding. It would also mean owning the bake, validation, versioning,
migration and debugging of a binary format forever, and re-solving layout,
compression and range reads that PMTiles already solves. Not worth it for a
handful of layers.

### Sidecar

A small companion structure holds what MVT cannot:

- normalized name index (FST) per language, built in data, not in render
- country outer rings and ISO lookup
- label rank/kind/provenance per feature id
- pack manifest: layer, `Lod`, attribution, license, source revision

Sidecar format is deliberately boring and versioned. It is the only part that
is ours.

## Licensing

Public-domain packs (Natural Earth, US Census, most national statistical
agencies) and share-alike packs (Overture, ODbL) are separate artifacts with
separate attribution. Never merge them into one pack. The manifest records the
license per layer so the UI can show the right attribution.

## Ownership

| Concern | Owner |
| --- | --- |
| source adapters, pack manifest, cache, name index | `earthcraft-data` |
| tile decode, projection, visibility, occupancy, label candidates | `earthcraft-render` |
| layer registry UI, textures, dialogs, settings | `apps/earthcraft` |
| camera, geographic math | `earthcraft-core` (unchanged) |

Decode and projection run off the UI thread; final font layout and shapes stay
on it. The frame coordinator keeps pacing tiles and overlays together.

## Spike results

Measured on 2026-10-20. Natural Earth GeoJSON came from the upstream
`natural-earth-vector` repository; sizes are uncompressed bytes on disk, point
counts are coordinate pairs in the geometry, and parse timings are Python
`json` and therefore an upper bound for Rust.

| Dataset | Size | Features | Points |
| --- | ---: | ---: | ---: |
| `ne_110m_admin_0_countries` | 0.80 MB | 177 | 10,654 |
| `ne_50m_admin_0_countries` | 2.94 MB | 242 | 99,613 |
| `ne_10m_admin_0_countries` | 12.67 MB | 258 | 548,471 |
| `ne_110m_populated_places` | 0.62 MB | 243 | 243 |
| `ne_50m_populated_places` | 3.20 MB | 1,251 | 1,251 |
| `ne_10m_admin_0_map_units` | 12.90 MB | — | — |
| `ne_10m_admin_1_states_provinces` | 38.84 MB | — | — |
| `ne_10m_populated_places` | 18.46 MB | — | — |

Three findings change the plan.

**1. `world-low` needs no tiling.** 110m countries plus 110m places is 1.42 MB
of plain GeoJSON, 10,897 points, and parses in ~17 ms in Python. Every tile
below zoom 5 would contain most of that data anyway. Bundle it as GeoJSON and
keep the pack format for detailed layers only. This removes a format from the
first milestone.

**2. LOD by zoom is the largest single win, and it is independent of tiling.**
Startup currently fetches four 10m layers unconditionally: countries, map units,
states and places, totalling ~82.9 MB uncompressed, at every zoom. The same
frame at low zoom needs ~1.4 MB, already on disk. Selecting 110m → 50m → 10m by
zoom costs a table lookup and removes nearly all startup network work. This
should ship before any tile machinery, because it is smaller, safer and pays
more.

**3. Tiles matter for detail, not for the basics.** The layers that genuinely
need tiles are the detailed national and Overture ones, where a viewport touches
a small fraction of a large dataset. At 14 KB per division tile, upstream
Overture is viable. It is not a replacement for Natural Earth labels.

Visual parity was **not** checked by eye. The 110m set has 1.9% of the 10m
vertex count, which is the right order for a globe view but must be confirmed on
screen before it replaces the current startup path.

## Migration

1. Select detail by zoom and bundle the 110m set. No new format, largest win.
   Confirm visual parity on screen first.
2. Introduce the seams (`LayerId`, `LayerSpec`, `TileKey`, `LayerTile`,
   `MapSnapshot`, `CountryIndex`) behind the existing crate contract, with the
   current whole-dataset path still producing identical output.
3. Move source knowledge into adapters; make the layer list a registry.
4. Switch the app to the registry; the UI gains a data-driven layer list and
   loses the per-layer booleans.
5. Enable per-tile refresh for the detailed layers, then attempt cross-fade.

Each step must pass the existing fixture comparisons and the desktop smoke
checklist. The bake step is dropped for `world-low`: it is already small enough
as upstream GeoJSON.

## Open questions

- Do we accept the first-load cost of national `FeatureCollection` sources, or
  bootstrap them from a bundled low-detail pack?
- Is a dedicated pole map wanted, or is the cap layer sufficient?
- Tile cache budget: the current imagery budget is 256 MiB. Vector tiles need
  their own, separately accounted budget.
