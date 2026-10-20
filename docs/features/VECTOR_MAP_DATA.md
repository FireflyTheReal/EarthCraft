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

- Bundled with the app: a small `world-low` pack (countries, first-order
  places). The app must draw a correct globe on first launch, offline.
- Fetched at runtime from the provider: everything else.
- Baked offline and shipped to the user by hand: Overture slices, until a
  hosting decision exists.

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
all properties to aid inspection. Per-tile byte size was not measured. That
matters: if a division tile is fat, a slimmed offline extract still beats the
upstream archive for first-load time even though upstream is technically tiled.
Treat upstream Overture as usable-but-unverified until a tile is decoded and
measured.

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

## Migration

1. Bake `world-low` and `world-high`; measure size, decode time, visual parity,
   dateline and pole behaviour. Stop here if parity fails.
2. Introduce the seams (`LayerId`, `LayerSpec`, `TileKey`, `LayerTile`,
   `MapSnapshot`, `CountryIndex`) behind the existing crate contract, with the
   current whole-dataset path still producing identical output.
3. Move source knowledge into adapters; make the layer list a registry.
4. Switch the app to the registry; the UI gains a data-driven layer list and
   loses the per-layer booleans.
5. Enable per-tile refresh, then attempt cross-fade.

Each step must pass the existing fixture comparisons and the desktop smoke
checklist. Step 1 is a spike and may be discarded.

## Open questions

- Do Overture's published PMTiles tiles carry sane per-tile sizes, or is a
  slimmed offline extract required for usable first-load time? Archive sizes and
  range support are verified; tile payloads are not.
- Do we accept the first-load cost of national `FeatureCollection` sources, or
  bootstrap them from a bundled low-detail pack?
- Is a dedicated pole map wanted, or is the cap layer sufficient?
- Tile cache budget: the current imagery budget is 256 MiB. Vector tiles need
  their own, separately accounted budget.
