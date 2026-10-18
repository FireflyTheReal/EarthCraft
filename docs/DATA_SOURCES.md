# EarthCraft data sources
Candidate map and data layers for EarthCraft, researched 2026-10-18.
Suggested location in the repo: `docs/DATA_SOURCES.md`.
Every row was confirmed by web search that date unless marked **†** (from memory, unverified).
Check each licence before shipping. Terms change.
## Legend
**Effort** is measured against EarthCraft today (Rust, egui, CPU ray-casting, HTTP tile fetch with on-disk cache, no system dependencies).

| Mark | Meaning |
|---|---|
| 🟢 | Plain XYZ or WMTS raster tiles. Same code path as the existing imagery layers. |
| 🟡 | JSON, CSV, WebSocket or PMTiles. Needs a small parser or client plus overlay rendering. |
| 🔴 | Large raster or columnar archive. Needs offline preprocessing (GDAL, PMTiles) or a heavy decoder. |

**Auth:** `None` = no key. `Key` = free key or token. `Acct` = free account.
**Implementation in EarthCraft** (second column added 2026-10-18 second pass) states how the row plugs into the app's on-the-fly HTTP fetch model:
- **Tile path** — reuse the existing XYZ/WMTS fetcher + on-disk cache (the imagery/terrain code path).
- **Geo fetch** — one-shot or periodic HTTP JSON/CSV/GeoJSON pulled straight into the overlay/vectored-boundary pipeline (how the app already loads boundaries and Overpass results). Works on the fly when the file is small.
- **Key client** — same as Geo fetch but needs a key from env/user config (never shipped).
- **Pre-bake** — needs an offline step (GDAL/netCDF/GRIB → bundled PNG/PMTiles/COG). Not a runtime source; the app only reads preprocessed output.
- **Bundle quilt** — vendored static asset baked into the release (safe for anything old and small, and applies to any source whose licence permits redistribution).
**Longevity & openness** grades who actually guarantees the row stays open long term:
- **🟣 institutional** — government/agency duty (NASA, NOAA, USGS, EU programmes, national mapping agencies): effectively permanent.
- **🔵 org-backed** — well-funded project or foundation (ESA, JRC, Protomaps, Overture, World Bank).
- **🟠 community/goodwill** — depends on one org's ongoing goodwill (EOX, OpenSky, Mapterhorn mirror): no guaranteed right; have a fallback or vendor a snapshot.
- **🔴 fragile** — single maintainer, no SLA, or a licence that permits revocation (RainViewer, FreeLightning, Esri free tiers): opt-in only, always with fallback.
---
## 1. Terrain and bathymetry
README gap: *"Real 3D terrain"*. Items 1.1 and 1.2 close it.

| # | Source | Access | Coverage / resolution | Auth | Licence / limits | Effort | Implementation in EarthCraft | Longevity & openness |
|---|---|---|---|---|---|---|---|---|
| 1.1 | **AWS Terrain Tiles** (Terrarium) | XYZ PNG: `https://s3.amazonaws.com/elevation-tiles-prod/terrarium/{z}/{x}/{y}.png`. Also `s3://elevation-tiles-prod` | Global, z0–16, 256 px. Land blended with ocean bathymetry | None | Attribution to Joerd/Tilezen. Decode: `elev_m = (R*256 + G + B/256) - 32768` | 🟢 | Tile path; terrarium PNG decode (`(R*256+G+B/256)-32768`, formula in row) | 🟣 AWS Registry of Open Data, hosted since 2016 — snapshot fallback would also be licence-clean |
| 1.2 | **Mapterhorn** | PMTiles (WebP) at `download.mapterhorn.com` (`planet.pmtiles`). Pipeline: `github.com/mapterhorn/mapterhorn` | Global 30 m (Copernicus GLO-30) with national LiDAR down to 0.4 m over many European regions. **Confirmed 2026-10-18** via `attribution.json` (151 sources): the Netherlands (AHN5), Spain (MDT02/MDT50, CC BY 4.0 SCNE), Switzerland (swissALTI3D), Japan (GSI DEM), all 16 German states' 1 m grids, Italy (regional DTMs: Trentino, Lombardia, Piemonte, Sardegna, Sicilia, Alto Adige CC0, plus TINITALY 10 m), Luxembourg (LiDAR 2019 MNT CC0), Portugal mainland + Madeira, Romania (LAKI II 0.5 m), Slovakia, Slovenia, Latvia, Estonia (1 m), Iceland (ÍslandsDEM), Faroes, Svalbard, Greenland, Taiwan (20 m), Israel (10 m), UK 1 m LiDAR (England / Scotland / Wales, OGL), US 3DEP 1 m and 1/3 arc-sec (PD), NZ (1 m), Australia (5 m + Tasmania 2 m), Canada, French overseas territories, Rwanda (10 m) | None | Attribution required (`mapterhorn.com/attribution`, machine-readable `download.mapterhorn.com/attribution.json`). Per-source licences are mixed (CC BY 4.0, DL-DE/Zero, CC0, OGL, PD, Licence Ouverte) — the aggregated planet file is usable with the combined attribution list. Aerial imagery in development | 🟡 | Tile path (PMTiles over HTTP range-requests) or pre-extract a region with `pmtiles extract` and ship it | 🟠 NLnet-backed mirror today, but every underlying source is 🟣 national OGD — a dead mirror is replaceable |
| 1.3 | **EMODnet Bathymetry** | WMTS `https://tiles.emodnet-bathymetry.eu/` | European seas | None | Open data. Confirm current terms | 🟢 | Tile path (sea-only overlay) | 🟣 EU / GMESA programme funding |
| 1.4 | **Copernicus DEM** (GLO-30 / GLO-90) | COGs: `s3://copernicus-dem-30m`, `s3://copernicus-dem-90m` | Global. GLO-30 Public has gaps for some countries | None | Copernicus licence | 🔴 | Pre-bake (per-region COG reads via `async-tiff`); not a runtime source at whole-globe scale | 🟣 Copernicus permanent, free & open |
| 1.5 | **GEBCO grid** (GEBCO_2026, released April 2026) | Global netCDF (~4 GB) / GeoTIFF / Esri ASCII via CEDA (`dap.ceda.ac.uk/bodc/gebco/global/gebco_2026/…`, also 8× 90°×90° tiles), user-area download app `download.gebco.net`, OPeNDAP | Global land + ocean, 15 arc-second. Includes ice-surface / sub-ice variants and a TID grid | None | Verified 2026-10-18: **placed in the public domain, free including commercial use**; GEBCO asks (not requires) the DOI citation | 🔴 | Pre-bake, or a user-directed ~4 GB regional download at runtime | 🟣 GEBCO (IHO+IOC) — public domain by stated policy |

## 2. Imagery and live weather

| # | Source | Access | Coverage / resolution | Auth | Licence / limits | Effort | Implementation in EarthCraft | Longevity & openness |
|---|---|---|---|---|---|---|---|---|
| 2.1 | **NASA GIBS** | WMTS `https://gibs.earthdata.nasa.gov/wmts/epsg3857/best/`. Read `GetCapabilities` for layers. Omit the time dimension for today | Global, dozens of daily layers (clouds, aerosols, sea ice, SST, fires) | None | NASA open data | 🟢 | Tile path; the only change is a time-dimension parameter in the URL template | 🟣 NASA EOSDIS — institutionally guaranteed |
| 2.2 | **EOX Sentinel-2 cloudless** | WMTS (`cloudless.eox.at`) | Global 10 m, yearly mosaics 2016–2025 | None | Attribution required. **Sources disagree on licence** (CC BY vs BY-SA). Verify before shipping | 🟢 | Tile path now; licence must be resolved before it can ship in a default layer | 🟠 company goodwill (free for over a decade, no guaranteed right) — keep GIBS as fallback |
| 2.3 | **OpenWeatherMap Maps 1.0** | XYZ PNG: `https://tile.openweathermap.org/map/{layer}/{z}/{x}/{y}.png?appid={key}` | Global clouds, precipitation, pressure, temperature, wind | Key | Maps 1.0 included in free plan. Attribution | 🟢 | Key client (appid from env) | 🔴 company free tier; ToS-dependent — optional plugin, never a default |
| 2.4 | **RainViewer radar** | Index `https://api.rainviewer.com/public/weather-maps.json`, then `{host}{path}/{size}/{z}/{x}/{y}/{color}/{options}.png`. Rust crate: `rain_viewer` | Global radar, past 2 h at 10 min | None | Free for personal, educational and small community use only. Attribution. No SLA | 🟢 | Tile path via the two-step JSON index; `rain_viewer` crate fits | 🔴 no SLA, personal/small-community terms — non-default layer with a fallback |
| 2.5 | **NOAA GFS 10 m wind** | GRIB2 via NOMADS `filter_gfs.pl`, converted with `grib2json` | Global, 1° grid, forecast | None | NOAA public data | 🟡 | Pre-bake wind vector snapshot (GRIB is parsed offline) | 🟣 NOAA NOMADS |
| 2.6 | **OSCAR ocean currents** | NetCDF via S3 `s3://podaac-ops-cumulus-protected/OSCAR_L4_OC_*` with temporary credentials from the `s3credentials` endpoint, or Earthdata Search | Global 0.25° daily surface currents (top 30 m), 1993–present, v2.0 (final / interim / NRT latencies) | Acct (free Earthdata Login, required — the S3 bucket is controlled access) | CC BY 4.0. Verified 2026-10-18 | 🔴 | Pre-bake netCDF per month; Earthdata auth blocks anonymous runtime fetch | 🟣 NASA, CC BY 4.0 |
| 2.7 | **Open-Meteo** | JSON: `https://api.open-meteo.com/v1/forecast`, `archive-api.open-meteo.com/v1/archive` | Point queries, 80-year history, ERA5-based | None | **Non-commercial only**. About 10,000 calls/day. Data CC BY 4.0 | 🟡 | Geo fetch client with per-place cache | 🟠 company goodwill with an explicit non-commercial clause — keep DWD as fallback |
| 2.8 | **NASA Earth Observations (NEO)** | JPEG / GeoTIFF downloads | 50+ climate layers (aerosols, chlorophyll, SST) | None | NASA open data | 🟡 | Vendored snapshot (static files) | 🟣 NASA — files frozen but permanent |
| 2.9 | **Esri World Imagery + Wayback** (EarthCraft's current default imagery) | Unauthenticated XYZ/WMTS: `server.arcgisonline.com/…/World_Imagery/MapServer/tile/` and `wayback.maptiles.arcgis.com/…/tile/{release}/{z}/{y}/{x}` | Global imagery + 500+ historical Wayback releases | None (grey) | **Verified 2026-10-18 against Esri's own ToU:** free services are for "internal purposes or noncommercial external purposes" only; Esri may alter or discontinue them at its sole discretion; unauthenticated endpoint use is reserved for Esri-product users per Esri staff; no bulk tile export. Attribution string: "Imagery © Esri, Maxar, Earthstar Geographics, and the GIS User Community" | 🟢 | Already wired — but should become an **opt-in layer**, never a default, for an open app | 🔴 non-commercial-only and revocable; a free ArcGIS Developer account (2M tiles/mo) is the compliant variant; GIBS (2.1) and EOX (2.2) are the attribution-clean fallbacks |

## 3. Wind resource

| # | Source | Access | Coverage / resolution | Auth | Licence / limits | Effort | Implementation in EarthCraft | Longevity & openness |
|---|---|---|---|---|---|---|---|---|
| 3.1 | **Global Wind Atlas 3** | GIS downloads for a point, area or country (`globalwindatlas.info`) | Global, 250 m, 10 / 50 / 100 / 150 / 200 m heights: wind speed, power density, capacity factor | None | Free. No tile endpoint found, so pre-bake to tiles | 🔴 | Pre-bake per country of interest; no tile endpoint exists | 🟣/🔵 World Bank ESMAP + DTU — attribution keeps bundling fine |
| 3.2 | **Wind Toolkit** (NLR, ex-NREL) | Download API (`developer.nlr.gov/docs/wind/wind-toolkit/wtk-download/` — **the nrel.gov domains were rebranded to NLR and `developer.nrel.gov` no longer resolves**, verified 2026-10-18) | US, 2.4M+ sites, 2007–2014 | Key | CC BY. Dataset is dated | 🟡 | Key client; the dataset is frozen (2007–2014), so one runtime import or a bundled copy | 🟣 US DOE (CC BY class on data) |
| 3.3 | **ERA5 (Copernicus CDS)** | Python API / CLI (`cds.climate.copernicus.eu`) | Global reanalysis since 1940, GRIB / netCDF | Acct | Free, Copernicus licence | 🔴 | Pre-bake only; CDS queue/credentials make runtime fetch pointless | 🟣 Copernicus |

## 4. Solar resource

| # | Source | Access | Coverage / resolution | Auth | Licence / limits | Effort | Implementation in EarthCraft | Longevity & openness |
|---|---|---|---|---|---|---|---|---|
| 4.1 | **Global Solar Atlas** | GeoTIFF (`globalsolaratlas.info/download`) | Global. GHI / DNI / DIF / GTI 250 m, PVOUT and TEMP 1 km | None | CC BY 4.0 (World Bank). World PVOUT file is 3.6 GB. Pre-bake | 🔴 | Pre-bake the 3.6 GB world raster into tiles outside the app | 🔵 World Bank ESMAP (CC BY 4.0) — redistribution permitted with attribution |
| 4.2 | **PVGIS 5.3** (EU JRC) | GET: `https://re.jrc.ec.europa.eu/api/v5_3/PVcalc?lat=45&lon=8&peakpower=1&loss=14` | Global except poles. Hourly series, TMY, PV yield | None | Free. 30 calls/s per IP. Not callable from browser AJAX (not an issue natively) | 🟡 | Geo fetch GET client (30 calls/s is plenty) | 🟣 EU JRC — free, best-effort, no SLA |
| 4.3 | **NASA POWER** | REST API (`power.larc.nasa.gov`) | Global, daily 1981–now: irradiance, wind, humidity, rain | None | Free. Throttles rapid repeat queries | 🟡 | Geo fetch client with backoff for throttling | 🟣 NASA |
| 4.4 | **NSRDB solar (PSM v4)** (NLR, ex-NREL) | REST returning CSV: `developer.nlr.gov/api/nsrdb/v2/solar/nsrdb-GOES-aggregated-v4-0-0-download.json` (PSM v3 endpoints deprecated). POST with a WKT point/multipoint | Americas (GOES) 4 km, 30-min, 1998–2023+, extended annually; Himawari / Meteosat endpoints for the rest of the world | Key | Free, CC BY 3.0 US (not 4.0). Verified 2026-10-18 | 🟡 | Key client, point-or-area CSV pull at runtime | 🟣 US DOE lab (now NLR) — CC BY 3.0 US |

## 5. Power infrastructure and base maps

| # | Source | Access | Coverage / resolution | Auth | Licence / limits | Effort | Implementation in EarthCraft | Longevity & openness |
|---|---|---|---|---|---|---|---|---|
| 5.1 | **Open Infrastructure Map** | Vector tiles via TileJSON (best-effort). Overpass for small GeoJSON extracts | Power lines, substations, wind turbines, solar generators, pipelines, telecom | None | OSM licence. Tile server may be restricted in future | 🟡 | Vector-tile decode or reuse the Overpass pattern; safest is a bundled PMTiles copy | 🟠 community project over OSM data — ODbL applies to derived distributions |
| 5.2 | **WRI Global Power Plant Database** | CSV (`github.com/wri/global-power-plant-database`) | About 35,000 plants, 167 countries: capacity, fuel, owner | None | Open. **Unmaintained** since v1.3.0 (June 2021) | 🟡 | Vendored CSV import | 🟠 WRI dataset frozen since 2021 — stable as a frozen snapshot |
| 5.3 | **Protomaps OSM basemap** | Daily PMTiles builds (`docs.protomaps.com/basemaps/downloads`) | Planet about 120 GB, z0–15. `pmtiles extract` for regions | None | ODbL, OSM attribution | 🟡 | PMTiles path; or pre-extract a country and bundle | 🔵 Protomaps Foundation; ODbL data is perpetual |
| 5.4 | **UMN "Primary Resources Extracted"** | Shapefile (U-Spatial) | Global points: mines, oil and gas, nuclear, hydro, solar and wind farms | None | Research use. **Redistribution or commercial use needs permission** | 🟡 | Bundle only after explicit permission | 🔴 redistribution needs permission — don't plan around it |
| 5.5 | **Renewable Energy Data Explorer** (NREL) | GIS layer downloads | Selected non-US countries | None | Check per layer | 🟡 | Bundle per selected layer | 🟣 US DOE |

## 6. Farmland, soil and land cover

| # | Source | Access | Coverage / resolution | Auth | Licence / limits | Effort | Implementation in EarthCraft | Longevity & openness |
|---|---|---|---|---|---|---|---|---|
| 6.1 | **ESA WorldCover 2021 v200** | COGs: `s3://esa-worldcover/v200/2021/map` (about 120 GB) | Global 10 m, 11 classes (cropland is one) | None | CC BY 4.0, attribution required | 🔴 | Pre-bake COG subsets per region (`async-tiff`) | 🟣 ESA (CC BY 4.0) |
| 6.2 | **ESA WorldCereal** | GeoParquet farm points (`source.coop/streambatch/worldcereal`). Rasters via Digital Earth Africa | 10 m maize, winter and spring cereal masks, irrigation | None | Open. Cite the WorldCereal paper | 🟡 | Geo fetch GeoParquet (source.coop range reads) | 🔵 ESA-funded WorldCereal programme |
| 6.3 | **USDA Cropland Data Layer** | GeoTIFF (`croplandcros.scinet.usda.gov`) | US, 30 m, crop-specific, annual | None | Public domain | 🔴 | Pre-bake US mosaic | 🟣 USDA public domain |
| 6.4 | **SoilGrids 2.0** (ISRIC) | GeoTIFF (`files.isric.org/soilgrids/latest/data/`), WCS | Global 250 m, 6 depths: pH, carbon, texture, CEC, N | None | CC BY 4.0. **REST API currently paused** | 🔴 | Pre-bake GeoTIFF volumes; do not depend on the paused REST API | 🟣 ISRIC (CC BY 4.0) |
| 6.5 | **Hansen Global Forest Change** | Direct 10° GeoTIFF tiles, no auth: `storage.googleapis.com/earthenginepartners-hansen/GFC-2025-v1.13/download.html`. Also GEE asset `UMD/hansen/global_forest_change_2025_v1_13` | Global ~30 m, updated annually: tree cover 2000, loss, gain, loss year, datamask (v1.13 = 2000–2025) | None | **CC BY 4.0**, verified 2026-10-18 (old appspot.com v1.3 links are dead) | 🔴 | Geo fetch of 10° GeoTIFFs via async-tiff; or consume the keyless GFW tile PNGs (13.5) as a far easier runtime layer | 🟣 CC BY 4.0; GEE-published asset also mirrored through Global Forest Watch |
| 6.6 | **NASA SEDAC Global Agricultural Lands** | Raster download | Global extent and intensity of agriculture, year 2000 | None | CC BY. **Old data** | 🔴 | Vendored snapshot | 🟣 NASA SEDAC (CC BY) — frozen 2000 data |
| 6.7 | **WorldClim** | Raster download | Global climate normals (1950–2000) | None | Check terms | 🔴 | Vendored snapshot (small) | 🟠 research product — terms to verify before shipping |
| 6.8 | **HydroSHEDS** | Vector / raster download | Global rivers and basins, from SRTM | None | Check terms | 🔴 | Vendored snapshot | 🟠 USGS/WWF lineage — terms to verify |
| 6.9 | **Natural Earth / GSHHG** | Shapefile download | Coastlines, borders, lakes | None | Check terms | 🟡 | Vendored GeoJSON in build | 🟣 public domain lineage, updated for decades |

## 7. Buildings and population

| # | Source | Access | Coverage / resolution | Auth | Licence / limits | Effort | Implementation in EarthCraft | Longevity & openness |
|---|---|---|---|---|---|---|---|---|
| 7.1 | **VIDA Google + Microsoft footprints** | PMTiles / GeoParquet / FlatGeobuf (`source.coop/vida/google-microsoft-open-buildings`) | Global, about 2.58 billion footprints | None | Source licences apply (Google, Microsoft) | 🟡 | PMTiles over HTTP range-requests, or pre-extract a metro area for interactive use | 🟠 source.coop mirror of Google/Microsoft releases; per-source licences vary — verify the release you bundle |
| 7.2 | **Overture Maps buildings** | GeoParquet on S3 / Azure (use the latest release under `s3://overturemaps-us-west-2/release/`) | Global. Merges OSM, Esri, Google, Microsoft | None | Per-source licences | 🔴 | Pre-extract at build time (5.6 GB GeoParquet is not a runtime source) | 🔵 Overture Foundation (ODbL) — industry-backed and stable |
| 7.3 | **GHSL population** (EU JRC) | GeoTIFF (`human-settlement.emergency.copernicus.eu`) | Global grids 1975–2100, 5-year steps. World Mollweide, so reproject | None | Open (JRC) | 🔴 | Pre-bake per region | 🟣 EU JRC |
| 7.4 | **GHS-POP COG mirror** (unofficial) | COGs at `data.source.coop/nlebovits/ghsl/` | GHS-POP R2023A, 100 m, 1975–2030 | None | Mirror of JRC data | 🔴 | Pre-bake; convenience mirror only | 🔴 unofficial mirror — cite JRC, keep 7.3 as the primary |

## 8. Live tracking and hazards

| # | Source | Access | Coverage / resolution | Auth | Licence / limits | Effort | Implementation in EarthCraft | Longevity & openness |
|---|---|---|---|---|---|---|---|---|
| 8.1 | **OpenSky Network** | `https://opensky-network.org/api/states/all?lamin=..&lomin=..&lamax=..&lomax=..` | Live aircraft, bbox queries | Optional | Anonymous access is rate-limited, credits apply | 🟡 | Geo fetch with credit budget; degrade gracefully to adsb.lol (already wired in the binary) | 🟠 non-profit; the anonymous tier has been shrinking — non-default layer |
| 8.2 | **AISstream.io** | WebSocket `wss://stream.aisstream.io/v0/stream`. Send subscription (key + bbox) within 3 s | Live global vessel positions | Key | Free, fair use, **beta / unstable**. Key must not ship in client builds | 🟡 | WS client + key env; keep the HTTP fallback (OpenSeaFeed / digitraffic are already in the binary) | 🟠 beta service — treat the fallbacks as the primary plan |
| 8.3 | **CelesTrak GP** | `https://celestrak.org/NORAD/elements/gp.php?GROUP=active&FORMAT=json` | About 16,000 active satellites. Needs SGP4 propagation | None | Free. Cache, refresh every few hours | 🟡 | Geo fetch of GP daily + an SGP4 crate | 🟠 run by T.S. Kelso since the 1980s; dependable but community-run |
| 8.4 | **NASA FIRMS** | API / WMS / WFS. Key at `firms.modaps.eosdis.nasa.gov/api/map_key/` | Global fire detections, 375 m VIIRS / 1 km MODIS, within 3 h | Key | Free. 5,000 requests per 10 min | 🟡 | Key client + prefetched fire tiles | 🟣 NASA — 5,000 req/10 min free tier |
| 8.5 | **NOAA SWPC aurora (OVATION)** | `https://services.swpc.noaa.gov/json/ovation_aurora_latest.json` | Global probability grid, 30–90 min forecast | None | NOAA public | 🟡 | Geo fetch every ~30 min into a filled-polygon overlay | 🟣 NOAA |
| 8.6 | **NOAA SWPC space weather** | JSON under `services.swpc.noaa.gov/json/` (Kp index, X-ray flares, solar wind) | Global | None | NOAA public | 🟡 | Same pattern as 8.5 | 🟣 NOAA |
| 8.7 | **NWS active alerts** | `api.weather.gov/alerts/active` | US | None | Public domain. **Verified 2026-10-18:** a User-Agent header identifying the app with contact info is required (official format `(myweatherapp.com, contact@myweatherapp.com)`); NWS says it may be replaced by an API key in future. Rate limits exist but are unpublished — on a limit, retry after ~5 s | 🟡 | Geo fetch; send a User-Agent like `EarthCraft/<version> (+https://github.com/therichronicles/EarthCraft)` | 🟣 US federal public domain |
| 8.8 | **NOAA AviationWeather** | `/api/data/metar`, `/api/data/airsigmet` | Global METAR, AIRMET / SIGMET polygons | None | NOAA public | 🟡 | Geo fetch (METAR TAF text + SIGMET JSON on a 2-min cycle) | 🟣 NOAA |
| 8.9 | **NOAA NDBC buoys** | `latest_obs.txt` | Global ocean buoys: wind, waves, pressure | None | NOAA public. No CORS, fine natively | 🟡 | Geo fetch of text products on the 50-min cycle | 🟣 NOAA |
| 8.10 | **USGS earthquakes** | GeoJSON summary feeds: `earthquake.usgs.gov/earthquakes/feed/v1.0/summary/{significant,4.5,2.5,1.0,all}_{hour,day,week,month}.geojson`; FDSN event search at `/fdsnws/event/1/query` | Global quakes, live (hour/day feeds update every minute) | None | **Verified 2026-10-18: public domain, no key.** USGS recommends the feeds for real-time display and asks clients not to hammer `/query`; the old "fetch details 1/s" advice stays unconfirmed | 🟡 | Geo fetch (already how the app consumes it) | 🟣 US federal public domain |

## 9. Other worlds

| # | Source | Access | Coverage / resolution | Auth | Licence / limits | Effort | Implementation in EarthCraft | Longevity & openness |
|---|---|---|---|---|---|---|---|---|
| 9.1 | **NASA Moon Trek** | RESTful WMTS. Product list: `trek.nasa.gov/tiles/apidoc/trekAPI.html?body=moon` | Moon mosaics, DEMs, equirectangular + polar | None | NASA open. Read each product's capabilities for matrix set and extension | 🟡 | Tile path + non-WGS84 projection handling per body | 🟣 NASA |
| 9.2 | **NASA Mars Trek** | RESTful WMTS (`api.nasa.gov/mars-wmts/catalog`) | Mars: MOLA hillshade, Viking, THEMIS, 21 products | None | NASA open. Missing tile means no coverage | 🟡 | Same as 9.1 | 🟣 NASA |
| 9.3 | **NASA Vesta Trek** | RESTful WMTS | Vesta, 6 Dawn products | None | NASA open | 🟡 | Same as 9.1 | 🟣 NASA |

## 10. Catalogs (find more data)

| # | Source | Access | Coverage / resolution | Auth | Licence / limits | Effort | Implementation in EarthCraft | Longevity & openness |
|---|---|---|---|---|---|---|---|---|
| 10.1 | **Earth Search** (Element84) | STAC `https://earth-search.aws.element84.com/v1` | Sentinel-2 COGs (11M+ scenes), Landsat, NAIP | None | Free, scene licences apply | 🔴 | Discovery portal (STAC) for scene pulls; items are pre-bake input, not runtime layers | 🟣-🟠 AWS Open Data Registry + Element 84 mirror |
| 10.2 | **Microsoft Planetary Computer** | STAC `https://planetarycomputer.microsoft.com/api/stac/v1` | 20+ PB, anonymous access to most datasets | None | Per-dataset | 🔴 | Discovery + per-dataset access | 🔵 Microsoft Planetary Computer programme |
| 10.3 | **Esri Open Data Hub** | Web portal | 250,000+ datasets from 5,000+ organisations | Varies | Per-dataset | n/a | Discovery portal only | mixed — evaluate per dataset |

## 11. Reference repos and Rust building blocks

| # | Project | What it gives EarthCraft | Notes |
|---|---|---|---|
| 11.1 | `cambecc/earth` + `cambecc/grib2json` | Particle-style wind and ocean-current visualisation from GFS and OSCAR JSON | MIT. Prototype for the wind layer |
| 11.2 | `TomHeaven/netcdf2json` | OSCAR netCDF to JSON conversion script | Offline step |
| 11.3 | `VrushankPatel/godseye` | Browser 3D globe wiring OpenSky, CelesTrak, SWPC, METAR, NDBC, AISstream | Good feed checklist. Includes military-site overlays. Take feeds only |
| 11.4 | `tapiab/heimdall` | Rust raster viewer with STAC browser, COG streaming, Sentinel-2 cloudless basemap | Uses **GDAL**, which conflicts with EarthCraft's no-system-deps goal. Read, don't copy |
| 11.5 | `async-tiff` (Rust crate) | Pure-Rust async COG / tiled TIFF reader via `object_store` | No GDAL. Fits 1.4, 6.1, 6.3, 7.4 |
| 11.6 | `rain_viewer` (Rust crate) | RainViewer client | Fits 2.4 |
| 11.7 | PMTiles tools (`go-pmtiles`) | `pmtiles extract` for regional cutouts | For 1.2, 5.3, 7.1 |
| 11.8 | TiTiler | Server-side COG to XYZ tiles | Option for 🔴 rasters if a tile proxy is acceptable |

## 12. Papers and citations

| # | Reference | Covers |
|---|---|---|
| 12.1 | Poggio et al. 2021, *SoilGrids 2.0*, SOIL 7, 217–240 | 6.4 |
| 12.2 | Hansen et al. 2013, *High-Resolution Global Maps of 21st-Century Forest Cover Change*, Science 342, 850–853 | 6.5 |
| 12.3 | Zanaga et al. 2022, *ESA WorldCover 10 m 2021 v200* | 6.1 |
| 12.4 | World Bank, *Global Solar Atlas 2.0 Technical Report* | 4.1 |
| 12.5 | WRI, *A Global Database of Power Plants* | 5.2 |
| 12.6 | GEBCO Compilation Group 2026, *The GEBCO_2026 Grid*, doi:10.5285/4f68d5c7-45eb-f999-e063-7086abc036fa (released April 2026; supersedes the 2025 grid) | 1.5 |
| 12.7 | ESA WorldCereal system paper (see WorldCereal project site) | 6.2 |
| 12.8 | Zhang et al. 2024, *GLC_FCS30D: the first global 30 m land-cover dynamics monitoring product…*, Earth System Science Data 16, 1353 | 14.18 |
| 12.9 | Souza et al. 2020, *Reconstructing three decades of land use and land cover changes in Brazilian biomes with Landsat archive and Earth Engine*, Remote Sensing 12(17), doi:10.3390/rs12172735 | 14.17 |
| 12.10 | Reetz et al. 2022, *DWD Geoportal – A central hub for Open Data, API and communication*, EMS2022-468, doi:10.5194/ems2022-468 | 14.5, 14.6 |
| 12.11 | JAXA EORC, *ALOS World 3D-30m (AW3D30) Version 4.0 Product Description*, April 2023 | 14.15 |
| 12.12 | Protomaps blog, *Mapterhorn – Terrain for Web Mapping* (2025); Wipfli, *Mapterhorn Terrain and Imagery*, State of the Map 2026 | 1.2 |

## 13. Live tracking and ambient extras (leads from the first pass, verified 2026-10-18)
Two leads came back negative (Dynamic World direct access, ADS-B Exchange free tier); the rest are usable. Key contact-free wins: GFW tiles (13.5), NOAA CO-OPS (13.6), the NCEI volcano endpoint (13.7).

| # | Source | Access | Coverage / resolution | Auth | Licence / limits | Effort | Implementation in EarthCraft | Longevity & openness |
|---|---|---|---|---|---|---|---|---|
| 13.1 | **Global Fishing Watch (4WINGS)** | REST v3 at `gateway.api.globalfishingwatch.org/v3`: gridded apparent fishing effort as PNG/MVT tiles + CSV/JSON/TIFF reports. Datasets: `public-global-fishing-effort:latest`, `-sar-presence`, `-presence` | Global ocean 0.01° grid, daily, 2012–present | Key (free token; registration asks for an organisation-period email — "sustainability-oriented org") | **CC BY-NC 4.0 — non-commercial only**, "Powered by Global Fishing Watch" attribution mandatory; 50,000 req/day, 1.5M/mo | 🟡 | Key client; PNG/MVT effort + CC BY-NC means non-commercial builds only | 🔵 well-funded non-profit, but the -NC licence is a hard constraint on shipping |
| 13.2 | **OpenAQ v3 air quality** | REST `api.openaq.org/v3` (`/locations`, `/sensors/{id}/measurements`); bulk copy on AWS S3 without a key | ~170 countries: PM2.5/PM10/O3/NO2/SO2/CO/BC from reference + low-cost monitors | Key (free, `X-API-Key`, register at explore.openaq.org) | Open with per-provider terms — OpenAQ demands attribution and disclaims aggregate licensing. 60 req/min, 2,000/hour; don't rehost as a competing service | 🟡 | Key client (`X-API-Key`); bulk bundle from the AWS S3 copy needs no key | 🟠 NGO; per-provider licences vary — fetch `/v3/licenses` and honour flags at runtime |
| 13.3 | **Dynamic World 10 m land cover** | Verified: CC BY 4.0 and available in Earth Engine (`GOOGLE/DYNAMICWORLD/V1`, 9 classes, Sentinel-2 refresh every 2–5 days). **Direct access refuted:** the long-cited GCS bucket `dynamic-world.earthengine` does not exist; non-GEE path = Earth Engine Python API with a GCP project (free only for research/education/nonprofit) | Global 10 m, 2015-06–now | Acct (GEE) for any practical path | CC BY 4.0 + Copernicus Sentinel notice. **Not usable for EarthCraft as-is** — only worth revisiting if Google opens the raster export | 🔴 (blocked) | No usable runtime path — revisit only if Google opens raster export | — blocked |
| 13.4 | **VIIRS night lights (NASA Black Marble)** | HDF5 archive at NASA **LAADS DAAC**: VNP46A2 daily gap-filled ~15 arc-sec (~500 m), 2012–now, plus monthly VNP46A4 and JPSS-1 (VJ146) equivalents; also free in GEE (`NASA/VIIRS/002/VNP46A2`) | Global | Acct (Earthdata); a Bearer token speeds scripted fetch | Free, released Aug 2020, citation DOI:10.5067/VIIRS/VNP46A2.002. **EOG / Colorado School of Mines products are the paywalled ones** — use NASA Black Marble. Big HDF5 files → offline preprocessing | 🔴 | Pre-bake HDF5 per region | 🟣 NASA (free since 2020, permanent URL) |
| 13.5 | **Global Forest Watch tiles (Hansen layers)** | Keyless tile URLs (live-tested): `https://tiles.globalforestwatch.org/umd_tree_cover_loss/v1.13/mercator/{z}/{x}/{y}.png` (512 px, PBF also available; implementations `mercator` / `gfw` / `dynamic`; covers alerts `umd_glad_*`, carbon flux, etc.). Analysis endpoints on `data-api.globalforestwatch.org` need a free key | Global 30 m raster tiles (z≤12) | Tiles None; Data API Key (free) | CC BY 4.0 (UMD/Hansen layers). **Complements 6.5**: the same data served as plain XYZ-style PNG tiles — no GDAL, fits the existing tile path today | 🟡 | | Tile path (keyless PNG) — the easiest live forest layer; cache like imagery | 🟣 CC BY 4.0 (same 6.5 data, second host) |
| 13.6 | **NOAA CO-OPS tides and currents** | Live-tested `https://api.tidesandcurrents.noaa.gov/api/prod/datagetter?…` (products `water_level`, `predictions`, currents, wind, met) + Metadata API `…/mdapi/prod/webapi/stations/<id>.json` | US tide and current stations, observations + hi/lo predictions | None | Public domain. No separate "v2" confirmed; documented set = Data API + Metadata API | 🟡 | Geo fetch `datagetter` + station metadata cache | 🟣 US federal public domain |
| 13.7 | **Smithsonian GVP volcano list** | Download at `volcano.si.edu/database/list_volcano_holocene_excel.cfm` (XML-Excel / CSV ~1,400 Holocene volcanoes; VEI lives in the separate eruption list). **Gotchas:** Cloudflare 403s plain HTTP clients (browser-like fetch required), and search/download availability is uncertain during the 2026 site rebuild | Global | None (download) | Smithsonian terms: free for personal / educational / non-commercial with attribution. **Better claim-free alternative (live-tested, ~1,600 entries):** NOAA NCEI `https://www.ngdc.noaa.gov/hazel/hazard-service/api/v1/volcanolocs` (JSON, public domain, VEI per eruption under `/volcanoes`) | 🟡 | Vendored CSV; or live Geo fetch from NOAA NCEI instead (public domain, no terms friction) | 🟠 Smithsonian terms are non-commercial; the NCEI alternative is 🟣 |
| 13.8 | **ADS-B Exchange** (live aircraft) | **Paid now:** Community API via RapidAPI $10/mo per 10k req (non-commercial), Enterprise tiers for real use; volume keys never ship in a client | Global, ~500+ community-fed receivers, 500 ms updates | Paid | Monthly sub, commercial licence only. **Free paths:** self-hosted receiver (RTL-SDR + dump1090/readsb), or community aggregators **adsb.lol** (free, ODbL API) and **airplanes.live** (free; access granted by email with a project description) | 🟡 self via adsb.lol / 🔴 paid | adsb.lol HTTP path (already in the binary); ODbL attribution | 🟠 community network — ODbL guarantees the data stays open even if the host dies |

Not usable / dropped: Dynamic World (13.3) for direct access; ADS-B Exchange free tier no longer exists (13.8).
## 14. National and regional sources (found via non-English search)
All 20 planned languages are covered. **Arabic: no open national source found** — Arabic-language searches only point back to global sources (NASA Earthdata, USGS, OpenTopography, Sentinel).
**Mapterhorn (1.2) confirmed via `attribution.json` for: Austria (1 m), Denmark, Flanders (1 m), Wallonia (0.5 m), Finland (2 m), Estonia (1 m), Czechia (5 m), France (Licence Ouverte 2.0), all 16 German states, Cyprus (1 m), Canada, Australia (5 m), plus the previously open questions — Netherlands (AHN5), Spain (MDT02/50), Switzerland (swissALTI3D), Japan (GSI DEM)** — and bonus: Italy, Luxembourg (CC0), Portugal, Romania, Slovakia, Slovenia, Latvia, Iceland, Faroes, Svalbard, Greenland, Taiwan, Israel, UK, NZ, Tasmania, Rwanda, French overseas territories. Terrain for a country is usually 1.2; the rows below add imagery and base maps. Details:
 Austria (1 m), Denmark (CC BY 4.0), Flanders (1 m), Wallonia (0.5 m), Finland (2 m), Estonia (1 m), Czech Republic (5 m), France (Licence Ouverte 2.0: RGE ALTI 1 m metropolitan + overseas, LiDAR HD), all 16 German states (1 m; licences DL-DE/BY-2.0, DL-DE/Zero or CC BY 4.0 depending on the state), Cyprus (1 m), Canada, Australia (5 m) — **and the countries that were open questions: Netherlands (AHN5 5 m, CC BY 4.0), Spain (MDT02/MDT50, CC BY 4.0 SCNE), Switzerland (swissALTI3D, Open Government Data), Japan (GSI 基盤地図情報 DEM, GSI terms with survey-law approval)**. Bonus coverage the per-country rows below don't duplicate: Italy (regional DTMs CC BY 4.0 / CC0, TINITALY 10 m INGV), Luxembourg (LiDAR MNT CC0), Portugal mainland + Madeira, Romania (LAKI II 0.5 m, OGL-ROU-1.0), Slovakia, Slovenia, Latvia, Iceland (ÍslandsDEM 1 m CC BY 4.0), Faroes, Svalbard, Greenland, Taiwan (20 m, OGD licence 1.0), Israel (10 m ASTER-derived PD), UK (Environment Agency / Scotland / Wales 1 m LiDAR, OGL), NZ (1 m LiDAR CC BY 4.0), Tasmania (2 m CC BY 3.0), Rwanda (10 m CC BY 4.0) and French overseas territories (RGE ALTI 1 m, Licence Ouverte). If terrain for a country is all you need, 1.2 is usually the answer; the per-country rows matter for imagery (orthophotos) and base maps, where Mapterhorn has no data.
Sorted by effort, then country. Language is the language of the source material.

| # | Source (language) | Access | Coverage / resolution | Auth | Licence / limits | Effort | Implementation in EarthCraft | Longevity & openness |
|---|---|---|---|---|---|---|---|---|
| 14.1 | **GSI Japan elevation tiles** 地理院標高タイル (ja) | XYZ PNG: `https://cyberjapandata.gsi.go.jp/xyz/dem5a_png/{z}/{x}/{y}.png` (also `dem1a_png`). Rust decoder crate: `demtile` (MIT) | Japan. DEM5A / 5B / 5C / 10B / DEMGM, finest first. Sea pixels are RGB (128,0,0) = no data | None | GSI terms of use (利用規約). Confirm. **Not Terrarium encoding.** Use GSI's own formula (`maps.gsi.go.jp/development/demtile.html`) | 🟢 | Tile path + per-source DEM decoder (`demtile` crate) — a second decode branch next to Terrarium | 🟣 GSI statutory free-use (with survey-law notice for derived sheets) |
| 14.2 | **PDOK aerial photos** Luchtfoto (nl) | WMTS `https://service.pdok.nl/hwh/luchtfotorgb/wmts/v1_0/Actueel_orthoHR/{TileMatrixSet}/{TileMatrix}/{TileCol}/{TileRow}.jpeg` | Netherlands, 8 cm and 25 cm, newest year | None | CC BY (capabilities keyword "open data cc-by"). Attribution to Kadaster / PDOK | 🟢 | Tile path | 🟣 Kadaster/PDOK statutory open data |
| 14.3 | **PDOK BRT Achtergrondkaart** (nl) | WMTS `https://service.pdok.nl/brt/achtergrondkaart/wmts/v2_0`. Also OGC API Vector Tiles | Netherlands topographic base map, EPSG:3857 available | None | CC BY 4.0 | 🟢 | Tile path | 🟣 same as 14.2 |
| 14.4 | **swisstopo geo.admin.ch** (de / fr) | WMTS / WMS / vector tiles. Layers include swissIMAGE, swissALTI3D hillshade, swissBATHY3D hillshade | Switzerland | None | Open Government Data since 2021-03-01. Free, commercial use allowed with attribution "©swisstopo" | 🟢 | Tile path | 🟣 swisstopo OGD policy (since 2021) |
| 14.5 | **DWD radar and weather WMS** (de) | WMS at `maps.dwd.de` (layer `dwd:Radar_rv_product_1x1km_ger`) and `cdc.dwd.de/geoserver` | Germany radar, 1 km, 5-minute steps, past plus 2 h forecast | None | CC BY 4.0, commercial use allowed. Attribution "Deutscher Wetterdienst". WMS needs bbox requests, not XYZ | 🟡 | WMS path (bbox, not XYZ) — new fetch branch; or 14.6 for point data | 🟣 DWD CC BY 4.0 |
| 14.6 | **DWD Open Data** (de) | HTTPS tree `opendata.dwd.de` (ICON forecasts, MOSMIX, RADOLAN radar). Python client `wetterdienst`. JSON wrapper: Bright Sky | Germany radar and observations. MOSMIX point forecasts for 5,400 stations worldwide | None | CC BY 4.0. No SLA, no support ticket (`opendata@dwd.de`) | 🟡 | Geo fetch over the HTTPS file tree | 🟣 DWD CC BY 4.0 |
| 14.7 | **IGN Géoplateforme** (fr) | WMTS / WMS / WFS (`geoservices.ign.fr`). Separate altimetry services | France | Check | Licence not confirmed in my sources. Check before use | 🟡 | Tile/WMS path once licence confirmed | 🟣 IGN — Licence Ouverte presumed, verify |
| 14.8 | **NGII background and satellite maps** 영상지도·배경지도 (ko) | WMTS. Layers: general, multilingual, satellite, night, hybrid | South Korea | Key (free, membership) | Public work, attribution (공공저작물_출처표시) | 🟡 | Key client with attribution string in the map footer | 🟣 Korean public-works statutes |
| 14.9 | **VWorld** 브이월드 (ko) | WMTS / WMS / WFS, 168 layers. WebGL 3D API | South Korea | Key (free) | Free. Traffic limits depend on the agency. Check terms | 🟡 | Key client | 🟣/🟠 traffic limits per agency |
| 14.10 | **Tianditu** 天地图 (zh) | `https://t{0-7}.tianditu.gov.cn/{vec\|img\|ter}_{c\|w}/wmts?...&tk={key}`. Vector, imagery, terrain + labels | China, CGCS2000 and Web Mercator. A Chinese tutorial says its imagery has no coordinate offset, unlike commercial Chinese maps | Key (developer registration, phone verification) | Terms of service apply. Confirm redistribution and caching rules | 🟡 | Key client (registration + phone verification) | 🟠 state ToS; redistribution and caching rules unclear — cautious |
| 14.11 | **CNIG MDT05 / MDS05** (es) | Download centre `centrodedescargas.cnig.es`. WCS 2.0 `https://servicios.idee.es/wcs-inspire/mdt` (1000, 500, 200, 25, 5 m) | Spain, 5 m terrain (MDT05) and surface (MDS05) from PNOA-LiDAR | None | **CC BY 4.0 (SCNE)**. First LiDAR coverage dates 2008–2015 | 🟡 | WCS client or pre-extract per region | 🟣 CC BY 4.0 (SCNE) |
| 14.12 | **IGN LiDAR HD** (fr) | GeoTIFF tiles (MNT 5 m, MNS 0.5 m, MNH) and point clouds via `cartes.gouv.fr/telechargement` and `data.gouv.fr` | France, at least 10 pulses/m². Rollout in progress (track at `macarte.ign.fr`). **Restricted zones (ZICAD) withheld.** MNS 50 cm tile ≈ 24 MB | None | Licence not confirmed. Check | 🔴 | Pre-extract LiDAR tiles per region | 🟠 IGN licence to confirm; ZICAD zones withheld |
| 14.13 | **swissALTI3D / swissSURFACE3D** (de / fr) | Download via swisstopo (also read through OpenTopography) | Switzerland, 0.5 m terrain + surface from the 2017–2025 national LiDAR campaign | None | Open Government Data, commercial use allowed, attribution "Federal Office of Topography swisstopo" | 🔴 | Pre-extract | 🟣 swisstopo OGD |
| 14.14 | **AHN** Actueel Hoogtebestand Nederland (nl) | Via PDOK (WCS, download) | Netherlands LiDAR terrain and surface. **AHN5 confirmed in Mapterhorn (2026-10-18) as 5 m grid, plus 0.5 m for Bonaire, Saba and St. Eustatius** | None | **CC BY 4.0** per `mapterhorn.com/attribution` (the older OSM-thread "CC0" claim refers to earlier editions — use CC BY 4.0 with attribution to AHN.nl) | 🔴 | Pre-extract (already inside Mapterhorn 1.2 — prefer that) | 🟣 CC BY 4.0 |
| 14.15 | **JAXA ALOS World 3D (AW3D30)** (ja / en) | Tiled GeoTIFF from JAXA EORC; also OpenTopography | Global 30 m DSM (trees and buildings included), v4.1 April 2024 | Acct | Free. **DSM, not bare earth** | 🔴 | Pre-extract after free registration | 🟠 JAXA free with account; DSM caveat |
| 14.16 | **PLATEAU 3D city models** (ja) | `geospatial.jp/ckan/dataset/plateau`. CityGML, 3D Tiles, MVT. Converter: PLATEAU GIS Converter (also outputs Minecraft MCA) | Japan cities (about 200 targeted for fiscal 2023, 500 for 2027). Buildings with height, floors, use, year built | None | Free including commercial use. Buildings also under CC BY 4.0 / ODbL / ODC-BY. Site policy applies | 🔴 | Bundle per city (3D Tiles or GeoJSON/GeoParquet extract) | 🟣 MLIT statutory open; sub-licences CC BY 4.0 / ODbL — strong |
| 14.17 | **MapBiomas** (pt) | GeoTIFF export via Earth Engine (`mapbiomas-user-toolkit`), downloads at `brasil.mapbiomas.org` | Brazil annual 30 m land use 1985–present: agriculture, pasture, forest. Also Bolivia, Ecuador and others | None / Earth Engine account | Attribution. Check terms. Matches the pt-BR README audience | 🔴 | Pre-bake from an Earth Engine export | 🟠 MapBiomas NGO pipeline + attribution |
| 14.18 | **GLC_FCS30D** (zh / en) | Zenodo `10.5281/zenodo.8239305` (36 zip files, 135–194 GB) | Global 30 m land cover 1985–2022, 35 classes incl. **rainfed vs irrigated cropland and orchards** | None | Check Zenodo record. The paper is CC BY 4.0 | 🔴 | Pre-bake from the Zenodo record | 🟣 Zenodo permanent DOI; paper CC BY 4.0 |
| 14.19 | **Geoportale Nazionale / PCN** (it) | WMS / WMTS view services (`www.pcn.minambiente.it`) | Italy: national orthophoto 0.5 m (several epochs 1988–2012), CTR vector map, DTM 10 m | None (viewer) | Metadata says **CC-BY-NC-ND 3.0 IT** and portal "termini di utilizzo" is stricter/murkier on redistribution — keep tiles client-side, attribution + same licence | 🟢 | Tile path, display-only — respect the NC-ND clause (don't republish tiles outside the app) | 🟠 national metadata says CC BY-NC-ND and portal terms are stricter — region sources are safer |
| 14.20 | **Regione Piemonte Geoportale** (it) | WMTS + downloads (`www.geoportale.piemonte.it`) | Piemonte: ortofoto AGEA 2021/2024, BDTRE raster 1:10,000, DTM LiDAR 1 m | None | Regional open data, licence reported CC BY-type † (verify per dataset) | 🟢 | Tile path | 🟠 regional OGD — verify per dataset |
| 14.21 | **Portale Geocartografico Trentino** (it) | WMS / WMTS + DTM downloads (`www.territorio.provincia.tn.it`) | Trentino: ortofoto 2021/2011, DTM 2 m, LiDAR | None | Regional attribution terms † (verify per dataset) | 🟢 | Tile path | 🟠 same |
| 14.22 | **geoportal.gov.pl Ortofotomapa WMTS** (pl) | Known-good WMTS: `https://mapy.geoportal.gov.pl/wss/service/PZGIK/ORTO/WMTS/StandardResolution` and `.../HighResolution` | Poland: ortho 5 cm urban / 10 cm / 25 cm pixel | None | Open PZGIK data (attribution-style open licence commonly cited as CC BY 4.0 †) | 🟢 | Tile path (plain WMTS, already confirmed live) | 🟣 Polish geodetic law (PZGIK) — statutory open |
| 14.23 | **geoportal.gov.pl NMT / LiDAR point clouds** (pl) | "Pobierz dane" module + open-data servers | Poland: NMT 1 m grid + raw LAS clouds, cycles 2011–2024 | None | Open PZGIK, free download (bulk fetching tolerated but official bulk API would need a key) | 🔴 | Pre-extract LAS/GeoTIFF | 🟣 same as 14.22 |
| 14.24 | **Dataforsyningen** (Klimadatastyrelsen, da) | WMTS / WMS, anonymous access confirmed (`dataforsyningen.dk`); Datafordeler needs a key for extra products | Denmark: ortofoto_forår (~10–12.5 cm), topo kort, DHM terrain 0.4 m (the last also inside Mapterhorn 1.2) | None | Frie offentlige geodata — attribution (kildeangivelse) mandatory; exact WMTS URL parameterisation still to be read from GetCapabilities | 🟢 | Tile path (anonymous access confirmed) | 🟣 Danish free-geodata statute |
| 14.25 | **Lantmäteriet öppna geodata** (sv) | WMTS under `opengeodata.lantmateriet.se` (paths not fully verified) + LIDAR FFM download tables | Sweden: ortofoto 0.5 m, topografisk webbkarta, national DEM from LiDAR | None | **CC0** (confirmed on lantmateriet.se; attribution welcome, not required) | 🟢 | Tile path | 🟣 Lantmäteriet CC0 — the strongest licence class |
| 14.26 | **Kartverket topo4 WMTS + NiB + hoydedata** (no) | WMTS (topo4 open cache), ortho at `norgeibilder.no`, DEM via `hoydedata.no` | Norway: topo4 raster, NiB orthophotos, DTM 1 m (Terrengmodellar also inside Mapterhorn 1.2, CC BY 4.0) | None for topo/DEM; NiB bulk needs an agreement | **CC BY 4.0** for free products; NiB bulk access is a separate licence | 🟢 | Tile path | 🟣 Kartverket CC BY 4.0 |
| 14.27 | **Maanmittauslaitos avoimet aineistot** (fi) | WMTS (`Karttakuvapalvelu`, free API key) + file-table downloads | Finland: taustakartta / raster maps WMTS, ortokuva 50 cm, korkeusmalli 2 m (also in Mapterhorn 1.2) | Key (free, for the map query service; downloads none) | **CC BY 4.0** | 🟢 | Tile path with free API key; file downloads need none | 🟣 NLS Finland CC BY 4.0 |
| 14.28 | **Landmælingar Íslands gagnatorg** (is) | WMS / WFS / WMTS at `gagnatorg.lmi.is` + DEM downloads (`ÍslandsDEM` 2 m / 10 m, also inside Mapterhorn 1.2, CC BY 4.0) | Iceland: IS50V topo, ÍslandsDEM v1.0 | None | CC BY 4.0 † (exact terms text to double-check) | 🟢 | Tile path | 🟠 small agency, statutory open (CC BY 4.0, exact text †) |
| 14.29 | **map.is / Loftmyndir ehf** (is) | Tile viewer (`www.map.is`) | Iceland: aerial photo mosaic < 0.5 m | None (viewing) | **Commercial** — personal viewing, no open tile API, no redistribution | 🟢 | Not usable — exclude | 🔴 commercial, no toggle endpoint |
| 14.30 | **Rosreestr** Публичная кадастровая карта / НСПД (ru) | Web viewer; unofficial community WMS endpoints exist | Russia: cadastral parcels, ortho/basemap in the viewer | None (viewer) | **No open licence; commercial use of the cadastral map is not permitted** (OSM community note). Unofficial WMS = legally murky †. Treat as display-only and effectively restricted | 🟡 | Do not ship | 🔴 effectively closed licensing |
| 14.31 | **Bhuvan / NRSC Open EO Data Archive (NOEDA)** (hi) | Web viewer + archive downloads (`bhuvan.nrsc.gov.in`, `bhuvan-app3.nrsc.gov.in/data/`) | India: LISS-III / LISS-IV, Cartosat DEM, ortho products | Acct (registration required) | NRSC Open Data rules — free for registered users with non-commercial-style restrictions † (licence text unverified) | 🟡 | Registration-gated download; viewer tiles are unofficial | 🟠 NRSC restrictions; registration needed |
| 14.32 | **İBB Şehir Haritası + data.ibb.gov.tr** (tr) | Tile service (`sehirharitasiapi.ibb.gov.tr`) + open-data portal | Istanbul only: uydu (satellite) imagery, building-height layer | None (viewer) | İBB open-data portal licence, attribution-style † (exact terms unverified) | 🟢 | Tile path (Istanbul only) | 🟠 municipal OGD, exact terms † |
| 14.33 | **TKGM Ada/Parsel Sorgu / TKKG** (tr) | Parcel query API + WMS/WMTS behind registration (`parselsorgu.tkgm.gov.tr`) | Turkey: cadastral parcels, orthophoto in the TKKG viewer | Acct / registration | Government service, restricted use, no open bulk † | 🟡 | Registration-gated parcel API; no open bulk | 🔴 restricted use |

## 15. Borders and administrative areas (second pass)
**What EarthCraft uses today** (inspected from the shipping 0.2.0 binary + cache): Natural Earth **1:50m** countries (~3× coarser than the 1:10m variant on the same host), `datasets/geo-countries`, NE admin_1 states, NE populated places, live Overpass for municipal/metro layers, plus an IBGE/geodata-br special case for Brazil. All sizes and licences below were checked live on 2026-10-18.

| # | Source | Access | Coverage / resolution | Auth | Licence / limits | Effort | Implementation in EarthCraft | Longevity & openness |
|---|---|---|---|---|---|---|---|---|
| 15.1 | **Natural Earth 1:10m ladder** (the immediate win) | GeoJSON on `raw.githubusercontent.com/nvkelso/natural-earth-vector/master/geojson/…` — **verified sizes:** countries 13.3 MB, map units 13.5 MB, admin_1 states 40.7 MB, urban areas 27.9 MB, populated places 4.9 MB | ~10× more vertices than the 1:50m file EarthCraft currently loads | None | Public domain — vendoring fully allowed | 🟡 | Drop-in: swap `ne_50m_admin_0_countries` for the 10m URL. Also add `map_units` (its `type` field carries "Disputed"/"Indeterminate"/"Lease") | 🟣 Public domain since 2009; vendor files into each release, keep GitHub raw as the live-update channel |
| 15.2 | **geoBoundaries gbOpen** (William & Mary geoLab) | API `https://www.geoboundaries.org/api/current/gbOpen/{ISO3}/{ADM0-5}/` returns per-country GeoJSON/TopoJSON/shp URLs on GitHub raw; `simplifiedGeometryGeoJSON` per boundary | ~1M boundaries, 200+ entities, ADM0–ADM5; full + simplified per country; CGAZ composites at ADM0/1/2 | None | **CC BY 4.0** verbatim ("only requirement… is acknowledgement"); redistribution and bundling allowed | 🟡 | Targeted per-country fetch (a country ADM2 file is ~2–15 MB) fits the existing one-shot GeoJSON path | 🔵 academic lab, funded since 2016; releases already public can't be revoked — vendor what you ship |
| 15.3 | **geoBoundaries CGAZ composites** | `github.com/wmgeolab/geoBoundaries/raw/main/releaseData/CGAZ/geoBoundariesCGAZ_ADM0.geojson` (ADM1/ADM2 same pattern) | Global composites, **ADM0 401 MB / ADM1 361 MB / ADM2 550 MB** full-resolution GeoJSON | None | CC BY 4.0 | 🔴 | Not a first-load fetch; pre-simplify offline (or bundle a simplified regional subset). Disputed areas are clipped to US State Dept lines | 🔵 same lab, but the full-size composites will shift every release |
| 15.4 | **Overture Divisions** | Anonymous GeoParquet over HTTP: `overturemapswestus2.blob.core.windows.net/release/2026-09-23.1/theme=divisions/type=*` (also `s3://overturemaps-us-west-2/release/…`) | ~5.8M divisions: country → region → county → locality → borough → neighborhood; localized names, population, `perspectives` (disputed_by / accepted_by) for the politics UI | None | **ODbL 1.0** (OSM-derived); per-feature `sources[].license` also carries geoBoundaries CC BY 4.0; share-alike attribution required | 🔴 | Pre-bake per-country slices at build time (theme is ~5.6 GB zstd; bbox-scanning in-app is awkward). Best source for upgrading the Overpass metro/locality layer into bundled polygons | 🔵 Microsoft/Meta/Amazon/TomTom-backed foundation — monthly releases since 2023; shares OSM's perpetual ODbL guarantee |
| 15.5 | **GADM v4.1** | Static files `geodata.ucdavis.edu/gadm4.1/…` | Global ADM0–3 (+ADM4/5 in some countries), 3.5M+ units | None | **"Freely available for academic and other non-commercial use. Redistribution or commercial use is not allowed without prior permission"** — cannot be bundled in an OSS build | 🔴 | **Excluded** for shipping. Fetch-only personal reference at most; geoBoundaries 15.2 is the open equivalent | — (excluded by licence) |
| 15.6 | **UN SALB** | Per-country on `salb.un.org` (uneven coverage), UN ArcGIS FeatureServers, no registration | ADM1/ADM2 to a 1:1m target | None | **Non-commercial only, revocable, no geometry changes; UN copyright + contributor credit.** `gbAuthoritative` inherits the restriction | 🔴 | **Excluded** for shipping | — (excluded by licence) |
| 15.7 | **US Census cartographic boundaries** | `www2.census.gov/geo/tiger/GENZ2023/shp/cb_2023_us_*` — **county_20m 0.90 MB, county_500k 11.6 MB, cbsa_20m 0.35 MB** (counties, CBSA/CSA metros, ZCTAs, tracts) | US detail down to tracts; new vintage each year | None | Public domain (US govt) | 🟡 | First-class "US special case" like Brazil's IBGE path — tiny fetches, heavier TIGER/Line files available too | 🟣 permanent US-gov open data |
| 15.8 | **Statistics Canada boundary files** | `www12.statcan.gc.ca/…/boundary-limites/` (SHP/GML/-gdb) + WMS / ESRI-REST at `geo.statcan.gc.ca` | Provinces, census divisions, census subdivisions (≈ municipality), CMA/CA metros | None | **Open Government Licence – Canada** (attribution; commercial OK) | 🟡 | Same special-case pattern as 15.7 | 🟣 statutory open-data framework |
| 15.9 | **France IGN ADMIN EXPRESS / COG** | Keyless Atom API `https://data.geopf.fr/telechargement/download/ADMIN-EXPRESS-COG/…` + WFS/WMS `data.geopf.fr` | Commune → canton → département → région; updated monthly | None | **Licence Ouverte / Open Licence 2.0** — attribution, commercial and redistribution OK | 🟡 | Per-country plugin (sgf→GeoJSON fetch; the already-wired data.geopf.fr host also serves altimetry 14.7) | 🟣 French state open licence |
| 15.10 | **Germany BKG VG250 / VG2500** | Open data `www.bkg.bund.de` + WFS `sgx.geodatenzentrum.de` | Bund → Länder → Regierungsbezirke → Kreise → Gemeinden; yearly | None | **GeoNutzV "nutzungsfrei"** (de-facto CC0-equivalent) — attribution not even mandatory | 🟡 | Per-country plugin | 🟣 strongest licence in the group |
| 15.11 | **Spain CNIG / IGN municipalities** | `centrodedescargas.cnig.es` (keyless) | Provincias / municipios / CCAA | None | **CC BY 4.0** (Orden FOM/2807/2015), redistribution OK | 🟡 | Per-country plugin; pairs with 14.11's WCS for Spain in general | 🟣 |
| 15.12 | **Mexico INEGI Marco Geoestadístico** | Zip download from `www.inegi.org.mx/geo/contenidos/geoestadistica/…` | Sates / municipios / AGEB / manzana (Brazil-grade detail) | None | **"Libre Uso MX"** — free incl. commercial, attribution required | 🟡 | Per-country plugin (shp → GeoJSON at build) | 🟣 national statistical institute |
| 15.13 | **NZ LINZ / Stats NZ boundaries** | `data.linz.govt.nz` + Stats NZ digital boundaries | Regions, territorial authorities, wards, SA1/SA2 | None | CC BY 4.0 | 🟡 | Per-country plugin, small country so easy | 🟣 |
| 15.14 | **GeoFabrik OSM extracts** | `download.geofabrik.de` daily `.osm.pbf` / `.gpkg` per country (0.7–33 GB) | All admin_level 2–10 + everything else | None | ODbL 1.0 | 🔴 | Too broad for runtime; use for offline preprocessing only | 🔵 15+-year-old German service, funded; ODbL keeps the data base open |
| 15.15 | **ne_10m_urban_areas** | GeoJSON, 27.9 MB (verified) | Global city-extent polygons | None | Public domain | 🟡 | One-shot fetch or bundle; the only free global "urban footprint" polygon set besides Overture | 🟣 |
| 15.16 | **osm-boundaries.com** | Web UI, download needs OAuth-OSM login; monthly import | All admin levels, configurable export | OAuth | ODbL passthrough; commercial operator (Ground Zero Communications AB) | 🔴 | **Not usable as an anonymous fetch path** — keep Overpass (already wired) for live OSM boundaries instead | ✗ (no server-based API) |

**Disputed borders verdict:** Natural Earth map_units (15.1) is the always-renderable default with its `type`/`disp` fields; geoBoundaries per-country gives "as the country represents itself" views; Overture `perspectives` is the only first-class multi-narrative dataset. OSM has one consensus set plus unofficial `disputed=*` tags — that's what the Overpass layers see today.
## 16. Traffic, night lights and second-pass extras
**Traffic verdict:** no free, key-free, global, live traffic feed exists (OSM serves none; Google/Apple tiles are ToS-forbidden). TomTom's free tier — 200,000 flow tiles/month, no card — stays the practical live option. The rows below are free alternatives or new categories.

| # | Source | Access | Coverage / resolution | Auth | Licence / limits | Effort | Implementation in EarthCraft | Longevity & openness |
|---|---|---|---|---|---|---|---|---|
| 16.1 | **Earth Traffic (iTIC/ELEMNT platform)** | `https://traffic.elemnt.earth/api/v1/tiles/h3-traffic/{z}/{x}/{y}.mvt?at=<ISO hour>` | Global H3 congestion hexes from 3.4B GPS probes, **hourly archive 2017–now**; live ingestion "forthcoming" | None | **CC BY 4.0**, attribution | 🟡 | MVT decode is new code (no vector parser for H3 yet); great for replay/time-slider, not for live jam colouring | 🟠 one small foundation, demo service — vendor the CC BY archive if relied on |
| 16.2 | **NDW (Netherlands road data)** | `https://opendata.ndw.nu/` DATEX II XML | NL only; 24k point-speed sites, ~1 min | None | Open gov | 🔴 | Custom DATEX II parser + location tables; useful per-country pattern only | 🟣 statutory national traffic authority |
| 16.3 | **GIBS Earth at Night (Black Marble city lights)** | WMTS `https://gibs.earthdata.nasa.gov/wmts/epsg3857/best/{VIIRS_Black_Marble\|VIIRS_CityLights_2012\|VIIRS_Night_Lights}/default/{date}/{z}/{y}/{x}.png` — resolve the fixed `date` from GetCapabilities once and hardcode | Global static night composites (2012 / 2016) | None | NASA open | 🟢 | Plain tile-path layer — this is the zero-effort night-lights basemap many browser globes use | 🟣 NASA GIBS |
| 16.4 | **OurAirports** | CSV: `https://davidmegginson.github.io/ourairports-data/airports.csv` (+ `runways.csv`, `navaids.csv`, `airport-frequencies.csv`) | ~85,000+ airports worldwide with runway geometry; rebuilt daily | None | **Public domain** (Unlicense) | 🟡 | One-shot CSV straight into the existing placemark pipeline | 🟠 single-maintainer but Unlicense: bundle a snapshot and the data is yours forever |
| 16.5 | **NOAA GOES GLM lightning** | Anonymous S3: `s3://noaa-goes{18,19}/GLM-L2-LCFA/YYYY/DDD/HH/` (~20-s netCDF flash-group-event files) | Americas + Central America discs, ~8 km | None | Public domain | 🔴 | S3 netCDF polling is real engineering; practical variant is a small pre-parsed flash JSON snapshot per 10 minutes | 🟣 NOAA — permanent open archive |
| 16.6 | **Blitzortung.org** | `data.blitzortung.org` (protected endpoints), min-latency global regions | Global community receiver network | **Participants only** | Redistribution and commercial use prohibited | ✗ | Excluded — no app-tier feed without joining the network | — |
| 16.7 | **FreeLightning (parsed GLM)** | `https://www.freelightning.com/glm/…` plain-text placefile, 40 s | GLM flashes reparsed | None | "Must credit FreeLightning.com" | 🟡 | Trivial text parse — good starter lightning layer | 🔴 hobbyist-run single maintainer; keep 16.5 as the durable fallback |
| 16.8 | **ETOPO 2022 / ETOPO1 relief** | `https://www.ngdc.noaa.gov/mgg/global/…` GeoTIFF/netCDF; ArcGIS ImageServer `gis.ngdc.noaa.gov/arcgis/rest/services/etopo1/MapServer` | 1 arc-min (old) / 15 arc-sec 2022 blend of topography + bathymetry | None | Public domain | 🟡 | Mid-res gauss grid is bundleable; the ImageServer can also be hit as tiles | 🟣 NOAA NCEI — permanent |
| 16.9 | **EMAG2 v3 magnetic anomalies** | GeoTIFF downloads `www.ngdc.noaa.gov/geomag/emag2_download.html`; keyless `getSamples` on `gis.ngdc.noaa.gov/arcgis/rest/services/EMAG2v3/ImageServer` | Global 2 arc-min (nT) | None | Public domain — "not subject to copyright", no navigation use; cite doi 10.7289/V5H70CVX | 🟡 | Bundle GeoTIFF; ImageServer is a handy verification tool | 🟣 NOAA |
| 16.10 | **GloFAS river discharge (Copernicus CEMS)** | GRIB/netCDF via `ewds.climate.copernicus.eu/datasets/cems-glofas-forecast`; WMS-T at `global-flood.emergency.copernicus.eu` | Global 0.05° (v4), 30-day forecasts, reanalysis back to 1979 | Acct (ECMWF) | Free & open (CEMS); registration; **explicitly not a time-critical operational service** | 🔴 | Not runtime material (GRIB + queue); use the WMS-T flood maps as a tile layer and pre-bake analytics | 🟣 Copernicus institutional |
| 16.11 | **GeoNames gazetteer** | `https://download.geonames.org/export/dump/` (daily dumps) | 12M+ place names, feature classes, admin links | None | CC BY 4.0 | 🟡 | Great fallback/dedup layer behind Natural Earth populated places; the geonames "cities15000" subset is a nice small bundle | 🟠 single-maintainer project but CC BY, so bundle freely |

**Negative results (closed):** no key-free XYZ host for Dynamic World, VIDA buildings or NASA DEM (keep as pre-bake); no free shipping-density feed; no new ocean/chlorophyll beyond GIBS/OSCAR; no on-the-fly geology raster (use 16.8/16.9).
## 17. Archaeological LiDAR and bare-earth DEM (third pass, verified 2026-10-18)
Scoping question: can EarthCraft ship "fly over a lost city" terrain (Maya cities, Amazonia geoglyphs/earthworks, Sahel tells, Andes, Mediterranean) from open LiDAR? Method: OpenTopography DCAT catalogue grepped live (`portal.opentopography.org/csw/catalog.json`, every record carries a licence field), dataset pages and Zenodo/Figshare/Dryad APIs read directly. Headline finding: **OpenTopography hosts genuinely archaeological collections (Caracol Belize, Middle Usumacinta, Quirigua) and a vegetation-stripped South America DTM (ANADEM), all CC BY 4.0 — but its API policy forbids embedding keys in a distributed app, so anything runtime must be user-keyed or pre-baked.** No open host exists for raw PACUNAM terrain and nothing for the Sahel. Two further sweeps the same day targeted South America and Africa specifically: verified open stacks exist in Brazil/Mexico (GeoSampa, INEGI) and municipal/insular Africa (Cape Town, Rwanda, Réunion/Mayotte), the flagship Amazon campaigns' raw LiDAR is legally or contractually closed, and North Africa / the Sahel / most of East Africa have no open airborne LiDAR at all.

| # | Source | Access | Coverage / resolution | Auth | Licence / limits | Effort | Implementation in EarthCraft | Longevity & openness |
|---|---|---|---|---|---|---|---|---|
| 17.1 | **OpenTopography global DEM API** — incl. **ANADEM** | REST `portal.opentopography.org/API/globaldem?demtype=ANADEM…&outputFormat=GTiff` bbox cut-out; catalogue via STAC (`/stac/raster_catalog.json`) and DCAT/CSW (both live-read) | Global 30 m set: **ANADEM (South America DTM, vegetation removed from Copernicus GLO-30 — geoglyphs become visible)**, COP30/90, SRTM, NASADEM, GEDI L3 1 km; ≤450,000 km²/request | Free OT API key per user | DCAT: all listed CC BY 4.0; policy: 50 calls/24 h non-academic, keys "may not… be embedded within an application", commercial integration needs an **Enterprise API key** | 🔴 | Geo fetch — user-supplied key from config (never shipped); server-proxy or pre-bake otherwise; STAC for catalogue verification | 🟣 NSF-funded facility, DOIs pinned |
| 17.2 | **OpenTopography archaeology LiDAR collections** | Direct DOI → public `.laz` tiles on `opentopography.s3.sdsc.edu` (Caracol verified keyless) | **Caracol/Chiquibul Belize** (10.5069/G9833Q42: 64 km², 1.5 B pts, ~24 pts/m²) · **Middle Usumacinta, Mexico** (10.5069/G95B00NF) · **Quirigua UAV, Guatemala** (10.5069/G9416V95) | None for download | CC BY 4.0 in DCAT; Caracol page itself says "Use License: Not Provided" **†** — cite PI + DOI, keep the ambiguity flag | 🟡 | Pre-bake — ground-classified DEM tiles per landmark Maya site, bundled; `.laz` too heavy for runtime | 🔵/🟠 single-project datasets on durable NSF storage; DOIs stable |
| 17.3 | **USGS 3DEP** | National Map raster + point-query `epqs.nationalmap.gov/v1` (also via OT mirror; 1 m there is academic-only) | USA only, 10 m seamless + 1 m tiles | None | **Public domain** (US federal); the reference model for EarthCraft's licence-verified catalog | 🟡 | Tile path / point query; out of primary target area but free of every caveat | 🟣 permanent USGS archive |
| 17.4 | **NASA GEDI L3 + ICESat-2 ATL08** (spaceborne, combined) | LP DAAC Data Pool HTTPS; GEDI L3 1 km DTM also in OT's `globaldem`; ATL08 via Earthdata/NSIDC | Global 1 km grid (GEDI) and track photon spots (ATL08) | Free Earthdata login for ATL08 | NASA EOSDIS: "full and open sharing of all data…" / GEDI CC BY 4.0 in DCAT | 🟢 | Geo fetch as background ground-elevation sanity check **only** — neither reveals earthworks or tells | 🟣 NASA archive, decades-scale |
| 17.5 | **Figshare: ML-ready Maya remote sensing (Chactún)** | API + HTTPS verified: `api.figshare.com/v2/articles/22202395` — `Chactun_ML_ready_lidar.zip` ≈ 1.08 GB ALS raster stack + masks/CHM, ~2.1 GB total | Chactún area, Campeche, Maya lowlands; ALS-derived DTM/CHM tiles | None | **CC BY 4.0** (verified in Figshare API licence field) | 🔴 | Pre-bake — decode ALS rasters to hillshade/relief tiles offline, bundle as the first fly-over Maya landmark | 🔵 Figshare DOI permanent; files stable |
| 17.6 | **Stonehenge LiDAR Archaeology Landscape (Zenodo)** | `10.5281/zenodo.10334054`, direct download | Stonehenge, Avenue, Cursus, barrow field | None | **CC BY 4.0**, open | 🟢 | Bundle — small pre-encoded show-case site (Neolithic/Medieval analogue for tells) | 🔵 Zenodo DOI |
| 17.7 | **Geoglyph DTM-anomaly toolkit (Zenodo)** | `10.5281/zenodo.5087620` — code + sample data for "Fast computation of digital terrain model anomalies… for geoglyph detection" | Nazca/Peru sample; method not full coverage | None | CC BY 4.0, open | 🟡 | Offline guidance only — the algorithm EarthCraft could run over 17.1 ANADEM tiles | 🔵 Zenodo DOI |
| 17.8 | **Dryad PACUNAM 2,144 km² Guatemala LiDAR** | `10.5061/dryad.k51j708` — only **25.04 MB** of derived shapes/stats | Northern Guatemala (the famous 2018 Science survey) **shapes only — no DTM/point cloud** | None | **CC0** † for the derived data; raw terrain closed / on request | 🔴 | Not a terrain source — cite 17.2-style pre-bakes instead | 🟣 metadata; raw stays closed |
| 17.9 | **Archaeology Data Service (ADS)** | Catalogue + DOI archives, mostly UK (Environment-Agency-grade LiDAR) | UK-dominant; no non-UK LiDAR surfaced (search is an SPA — not exhaustive) **†** | None | "…licence conditions clearly identified within each data collection"; metadata CC BY 4.0 | 🟢 | Not a pipeline — copy its per-collection licence-tagging pattern; UK bundle-quilt option only | 🟣 CoreTrustSeal curated |
| 17.10 | **GeoSampa — city of São Paulo LiDAR** | Download page `geosampa.prefeitura.sp.gov.br/PaginasPublicas/_SBC.aspx`; GeoNetwork record "Nuvem de pontos MDT 2020" (read directly) | ~1,522 km² city; LAS/LAZ 10 pts/m² + ground-filtered MDT 0.50 m, 2017/2020 vintages | None | Record states access "semRestricoes"; state open-data licence with attribution | 🟡 | Pre-bake MDT excerpt — the model open Brazilian municipal LiDAR stack | 🟣 PMSP institutional catalogue (ISO 19115) |
| 17.11 | **IGC-SP — São Paulo *state* LiDAR 2023/24** | `igc.sp.gov.br` downloads + state WMS geoportal (bot-blocked to curl from here) | State-wide slices, 10 pts/m², 2023/2024 flights (1:10,000 base) | None | State open-data assumed **†** — licence wording and per-tile download unverified | 🟡 | Pre-bake via IGC download flow once reachable | 🟣 state cartographic institute **†** downloads unverified |
| 17.12 | **INEGI MDE / historical LiDAR tiles (Mexico)** | `inegi.org.mx/programas/mde/`; Biblioteca (upc 702825796341); Continuo de Elevaciones Mexicano | National; LiDAR 2010–2017 vintages, MDT ~1:10k blocks + CEM 3.0 continuous | None | **"Libre Uso MX"** open-data terms with attribution | 🟡 | Geo fetch of tile catalogue — the workflow pattern to copy (per-tile LAS/LAZ + raster MDT) | 🟣 national statistical-agency portal |
| 17.13 | **Acre geoglyph site vectors (de Souza 2018, Nat Commun)** | `nature.com/articles/s41467-018-03510-7` supplementary (OA) | ~47,887 km² southern rim of Amazonia; geoglyph/earthwork polygons | None | Article + supplement **CC BY 4.0**; raw LiDAR never published | 🟡 | Bundle site vectors as placemark overlay on the ANADEM (17.1) hillshade | 🔵 Springer OA, stable DOI |
| 17.14 | **SE-Acre mound villages (Iriarte 2020, JCAA)** | `journal.caa-international.org/articles/10.5334/jcaa.45` — full OA text | 65.6 km² transects (Oct-2018 RIEGL flights); Table 1 = UTM coords of 57 mound villages | None | Article **CC BY 4.0**; DTM not published (ERC PAST/FUTURES) | 🟡 | Extract Table 1 → placemark layer; terrain stays research-held | 🔵 Ubiquity/CAA OA journal |
| 17.15 | **Loreto sites gazetteer (Coomes 2021, Sci Data)** | figshare collection `10.6084/m9.figshare.c.5262530` (Excel + API verified) | 415 geolocated archaeological sites, Loreto, Peruvian Amazon | None | **CC BY 4.0** (verified in figshare) | 🟡 | Bundle vector placemarks — the target list for any future LiDAR there | 🔵 figshare DOI + Springer OA |
| 17.16 | **Peru / Colombia / Ecuador national LiDAR** | None found | Ministry of Culture (Machu Picchu, Nasca): restricted heritage data; IDEAM Colombia: claim unverifiable (`datos.gov.co` Socrata search = 0 hits); Ecuador: no ministerial portal | – | Not open / not verifiable | – | No action — re-check if portals appear | 🔴 closed or non-existent |
| 17.17 | **City of Cape Town LiDAR DEM/DTM** | ArcGIS open-data portal + MapServers `Elevation_CCT_GLM2019_5m_DEM` and `Elevation_DTM_2m` (both live-read) | ~2,400 km² municipality; 2019 LiDAR → 5 m bare-earth DEM; 2025 LiDAR → 2 m ground-only DTM (2–3 pts/m², 0.1 m v-acc) | None | City Open Data Terms of Use — free, attribution, reuse OK | 🟡 | Pre-bake or live ArcGIS REST pull — the cleanest fully-open African municipal LiDAR | 🔵 municipal program, 2019→2025 refresh cycle |
| 17.18 | **Kruger NP river LiDAR 2012 (CEDA)** | CEDA catalogue uuid `a2e82c7f…`, direct web download + OPeNDAP | 3× 50 km river reaches (Sabie/Olifants/Letaba); Optech M200 point cloud + contours + orthos, 12 GB | Free CEDA registration | **OGL v3** — "Permitted Use: Any", citation required | 🔴 | Link-only (point cloud) — the only verified open airborne point-cloud archive on the continent | 🟣 NERC-funded permanent archive |
| 17.19 | **Rwanda 10 m DEM (via Mapterhorn)** | `download.mapterhorn.com/sources/rw.tar` (~316 MB; origin `github.com/WASAC/rwanda-dem`) | Whole country, 10 m grid | None | **CC BY 4.0** (NLA + WASAC/UNDP — attribution string read from `attribution.json`) | 🟡 | Bundle; cross-ref the 1.2 pipeline | 🟣 open gov source, Mapterhorn mirror |
| 17.20 | **IGN RGE ALTI + LiDAR HD — Réunion & Mayotte** | Mapterhorn tarballs (frrgealti1reunion ≈ 3.1 GB; frhd2975 LiDAR HD 0.5 m ≈ 14 GB) + `cartes.gouv.fr` | Full-island 1 m/5 m MNT both islands; Réunion LiDAR HD 0.5 m | None | **Licence Ouverte 2.0**; IGN docs name "archéologie" as an intended application | 🟡 | Bundle/mirror via Mapterhorn | 🟣 French open data, Mapterhorn mirrored |
| 17.21 | **Kilwa Kisiwani 3D (CyArk, Zenodo)** | `10.5281/zenodo.10377387` — `.glb` meshes | Swahili port-town monuments; terrestrial LiDAR + photogrammetry | None | **CC BY-NC-SA 2.0** — non-commercial + share-alike | 🟡 | Bundle glTF as site overlay — only if the NC licence is acceptable | 🔵 Zenodo DOI stable |
| 17.22 | **Great Zimbabwe (Zamani Project)** | `zamaniproject.org` viewer | Great Enclosure + Hill Complex terrestrial laser-scan models | Viewer public; point clouds on request **†** | Academic courtesy reuse — clarify per request | 🔴 | Link 3-D models; no bulk data | 🟠 university-hosted, request-based |
| 17.23 | **CD:NGI South Africa + eThekwini footprints** | `ngigeoportal.dlrrd.gov.za` (email-gated downloads) + eThekwini open-data portal | SA-wide orthos/topo/25 m DEM; **nationwide 1–2 m DSM/DTM is commercial** (Hexagon/GeoSpace); Durban: LiDAR-derived building footprints only | Email handoff | Free use with attribution; hi-res elevation commercial **†** | 🟡 | On-demand pulls only; do not bundle | 🟣 long-lived custodian; portal restructures change URLs |

**Archaeology verdict:** only OpenTopography plus the two small bundles are shipping-grade today: 17.2 (Caracol/Usumacinta ground DEMs) and 17.5 (Chactún) give two fly-over Maya sites as pre-bakes; 17.1 ANADEM is the only open *bare-earth* layer that could reveal Amazonia earthworks and Sahel tells at 30 m — worth a hillshade experiment before committing. Spaceborne (17.4) is not archaeology-visible, matching expectations. **South America:** GeoSampa (17.10) and INEGI (17.12) prove open LiDAR stacks exist; the free site-vector bundles (17.13–17.15) are the immediate wins — overlay the Acre/Loreto placemarks on the ANADEM hillshade for an instant "flying over the geoglyphs" demo. **Africa:** Cape Town (17.17), Rwanda (17.19) and Réunion/Mayotte (17.20) are bundleable now; Kruger (17.18) is the open point-cloud exemplar; everything prehistoric on the continent otherwise rides on ANADEM/COP30 hillshading.

**Negative results (closed):** raw PACUNAM DTM tiles (17.8) not open; Santa Rosa Xtampak LiDAR DEM Zenodo 10.5281/zenodo.7983013 is "restricted"; no open Sahel-tell LiDAR on Zenodo/Dryad/Figshare; El Shincal (Inca, Argentina) Zenodo 10.5281/zenodo.7382162 is open CC BY 4.0 but a paper, not a DEM. **Amazonia flagship campaigns:** Pärssinen et al. 2026 (*Nature* 657:420–425, 20,000+ earthworks, 4,497 km²) states verbatim *"The original LiDAR raw data are restricted under Brazilian law"*; Prümers 2022 (Bolivian Mojos) shares the DTM only on request via the DAI; Rostain 2024 (Upano Valley) releases supplementary vectors only. **Federal Brazil:** IBGE has no airborne LiDAR archive. **Africa:** North Africa (Egypt/Sudan/Libya/Tunisia/Morocco) — no open archaeological LiDAR anywhere (Zenodo sweeps returned nothing; Giza is photogrammetry/GIS only); Sahel/West Africa (Benin Iya, Djenné-Djenno, Timbuktu) — the active DAI–MOWAA Benin survey uses drone photogrammetry, no LiDAR, no release; Kenya — commercial contractor only; Tanzania — no airborne archaeology LiDAR; Ethiopia (Aksum) — PDF chapter only; Madagascar — nothing.

## 18. General archaeology and heritage place layers (third pass, verified 2026-10-18)
Scoping question: beyond LiDAR terrain, what open **point/vector layers of archaeological and heritage sites** can EarthCraft overlay — on land and under water (ocean mapping: shipwrecks, submerged prehistory)? Method: live WMS/JSON/API reads. The strongest wins: **GeoNames already contains archaeology/wreck feature codes inside the existing 16.11 dumps (zero new pipeline)**, **Pleiades** is the ancient-Mediterranean gazetteer (CC BY 3.0, JSON API live-tested), and **EMODnet Human Activities** serves Cultural-Heritage shipwreck + submerged-prehistoric layers as WMS/WFS for European seas. National heritage registers (UK/Ireland/US) are the per-country-plugin pattern; UNESCO lists are display-quality only for now.

| # | Source | Access | Coverage / resolution | Auth | Licence / limits | Effort | Implementation in EarthCraft | Longevity & openness |
|---|---|---|---|---|---|---|---|---|
| 18.1 | **GeoNames heritage feature codes** | Already-downloaded dumps (16.11): filter `featureClass=S` codes — **verified on the codes page:** `RUIN` ruins, `PYR/PYRS` pyramids, `RLG` "ancient site of significant religious importance", `WALLA` "ancient wall", `TMB` tombs, `SHRN` shrines, `HSTS` historical sites, `FT` forts/earthworks, `WRCK` wrecks | Global points; density varies by country | None | CC BY 4.0 | 🟢 | No new fetch — a filter over the 16.11 pipeline (placemark layer "archaeology"); the codes list above is the exact filter set | 🟠 GeoNames CC BY; bundle a snapshot and it is yours |
| 18.2 | **Pleiades** (ancient Mediterranean places) | JSON per place: `pleiades.stoa.org/places/{id}/json` (live-tested); bulk "Pleiades Datasets 4.1" downloads on the site | 41,480 citable ancient places (v4.1, May 2025): Greek/Roman/Levantine sites, forts, tombs | None | Footer verified: "Creative Commons Attribution 3.0 License (cc-by)" — attribution + cite DOI per place | 🟡 | Geo fetch or bundle the v4.1 CSV/GeoJSON dumps into the placemark pipeline (one-time asset, small) | 🔵 academic consortium (NYU/ISAW), two decades running |
| 18.3 | **EMODnet Human Activities — Cultural Heritage** | WMS verified in GetCapabilities: layers `heritageshipwrecks`, `heritageshipwrecksv2`, `wwshipwrecks`; WFS + CSV download on the portal; also **Submerged Prehistoric Archaeology and Landscapes** points | European seas: wreck positions (heritage + WWI/WWII), submerged prehistoric sites | None | EMODnet open-data (CC BY 4.0 † — confirm on the download page) | 🟢 | Tile path (WMS) or WFS one-shot into the vector overlay — zero new infrastructure | 🟣 EU/CINEA-funded service line |
| 18.4 | **NOAA ENC chart data — WRECKS/OBSTRN** | S-57 ENC downloads (keyless) + chart-vector pipeline; wreck (`WRCK`), obstruction (`OBSTRN`) are standard S-57 object classes | US waters and coasts, chart-accurate | None | Public domain (US federal) | 🔴 | Pre-bake — S-57 → GeoJSON offline (no runtime S-57 decoder), ship as wreck placemarks | 🟣 NOAA permanent |
| 18.5 | **NPS National Register of Historic Places** | Bulk data page `nps.gov/subjects/nationalregister/database.htm` (page live; bulk CSV link not grep-visible — JS) **†** | ~95,000 US historic places incl. archaeological districts | None | Public domain (US federal) | 🟡 | One-shot CSV into the placemark pipeline | 🟣 US federal |
| 18.6 | **National heritage registers: UK / IE** | Historic England NHLE data services (API, free key) — Cloudflare-blocks plain clients † · Canmore (Scotland, OGL †) · Coflein (Wales) · Ireland Historic Environment Viewer / RMP shapefiles (301-redirect verified, exact open-licence text †) | UK + Ireland: listed buildings, scheduled monuments, SMR/RMP sites | Key or none | OGL / open-reuse family (attribution) — per-source text to verify before shipping | 🟡 | Per-country plugin in the 15.9–15.12 pattern (key from config if required) | 🟣 statutory heritage agencies |
| 18.7 | **UNESCO World Heritage list** | Official list XML `whc.unesco.org/en/list/xml` — exists but **Cloudflare-gates plain HTTP clients** (verified) — browser-UA fetch or mirror needed † | 1,200+ WH sites incl. all major archaeology properties | None | UNESCO terms apply — redistribution text unverified **†** | 🟡 | Geo fetch with browser-like headers; kiosk-quality fallback: OSM `heritage=2` tag mining via Overpass (already wired) | 🟣 UNESCO institution; mirror risk low |
| 18.8 | **UNESCO Underwater Cultural Heritage database** | Web viewer only (`unesco.org/en/underwater-cultural-heritage`); **no open API/bulk found** (fetches empty) **†** | Global designated UCH sites | None | Unclear; viewer-only | 🔴 | Not usable now — re-check if UNESCO opens bulk data | 🔴 no API — excluded for now |
| 18.9 | **Wikidata "archaeological site" (Q839954) SPARQL** | `query.wikidata.org` SPARQL with coordinate export; also PeriodO (period definitions, CC0) for time-sliders | Global; completeness wildly uneven by country | None | **CC0** (Wikidata) — attribution-free | 🟡 | Geo fetch / SPARQL one-shot into the placemark pipeline; good cross-check layer for dedup | 🔵 Wikimedia infrastructure, CC0 |

**Heritage-layer verdict:** start with 18.1 (already in the binary, just a filter), then 18.2 + 18.3 (bundle/fetch, clean licences). The national registers (18.5/18.6) are the Brazil-pattern per-country plugins. Ocean archaeology for EU waters is genuinely open via EMODnet; US via NOAA ENC pre-bakes; elsewhere there is no open global wreck list — UNESCO's stays viewer-only.

## 19. Nature, science and historic-map extras (third pass, verified 2026-10-18)
Scoping question: what remains uncovered that would make the globe feel alive — the natural and historical layers nobody ships? Live-verified: **GBIF** (keyless species-occurrence API, response read), **Paleobiology Database** (bbox JSON with lat/lon + geological age, response read), **Randolph Glacier Inventory** (site live), **timezone-boundary-builder** (licence quoted verbatim from the README). For "time-travel" mode, historical topographic maps exist as free tile/scans services in the US, UK and NL. openAIP is the one aviation dataset, but its licence is non-commercial — opt-in only.

| # | Source | Access | Coverage / resolution | Auth | Licence / limits | Effort | Implementation in EarthCraft | Longevity & openness |
|---|---|---|---|---|---|---|---|---|
| 19.1 | **GBIF species occurrences** | Keyless REST verified: `api.gbif.org/v1/species/match?name=…` (live) + `/v1/occurrence/search?taxonKey=…&hasCoordinate=true`; also bulk occurrence snapshots on AWS S3 (`registry.opendata.aws/gbif`) | ~2.5 B geo-tagged occurrence records, all taxa, global | None | CC BY 4.0 / CC0 per contributing dataset; attribution "GBIF" | 🟡 | Geo fetch — species-search placemark layer ("where do golden eagles live"); bulk pre-bake from the S3 snapshot | 🔵 GBIF secretariat (30+ countries), permanent DOIs |
| 19.2 | **Paleobiology Database (fossils)** | Keyless JSON verified: `paleobiodb.org/data1.2/colls/list.json?lngmin=…&latmin=…&show=full` (live — returns lat/lon + geological age per fossil collection); also `colls/geo.json` GeoJSON | ~200k fossil collections worldwide with age (Maastrichtian, Pleistocene…) | None | CC BY per PBDB terms **†** (attribution; cite collection refs) | 🟡 | Geo fetch — "fossil sites" placemark layer with epoch filter; pairs with 9.x for deep-time mode | 🔵 400+ institutions; two decades |
| 19.3 | **Randolph Glacier Inventory (RGI 7.0) / GLIMS** | Site live (`glims.org/RGI/`); RGI 7.0 via NSIDC; GLIMS GLIMS glacier database WMS | ~215,000 glacier outlines, global, versioned snapshots | None | Freely available for research and education **†** (exact statement to re-read on the RGI page) | 🔴 | Pre-bake — glacier polygons as terrain "icing" mask or white-cap overlay | 🟣 NSIDC + GLIMS consortium |
| 19.4 | **Movebank animal tracking** | REST `www.movebank.org/movebank/service/direct-read` (Earthdata login) | Live GPS tracks of tagged animals (birds, whales), 3B+ locations | Acct (Earthdata) | Per-study licences (CC BY / CC0 / restricted) **†** — check each study's licence flag | 🟡 | Geo fetch — animated migration overlay for flagship open-licence studies only | 🔵 Max Planck/AGB consortium |
| 19.5 | **openAIP airspace** | Free API key; weekly airports + airspace volumes + navaids dumps | Global aviation data (airspace polygons would draw the "no-fly zones" over the globe) | Key (free) | **CC BY-NC 4.0 †** — non-commercial only | 🟡 | Key client — opt-in plugin only, like GFW 13.1; never a default layer | 🔴 NC licence + community model |
| 19.6 | **timezone-boundary-builder** | GitHub releases: `evansiroky/timezone-boundary-builder` — simplified/full shapefile + GeoJSON, `tzid` per polygon | Global tz boundaries matching IANA identifiers | None | Verbatim: "The outputted data is licensed under the Open Data Commons Open Database License (ODbL)" | 🟡 | Bundle — one-time, replaces per-app "what time is it there" guessing; share-alike on any redistributed modifications | 🔵 active GitHub project, yearly releases |
| 19.7 | **USGS historical topographic maps (topoView)** | Scan downloads per quad, keyless; also WMTS current-scales `basemap.nationalmap.gov/arcgis/rest/services/USGSTopo/MapServer` for the modern equivalent | USA, 1884–2006 topographic quads | None | Public domain (US federal) | 🔴 | Pre-bake — georeferenced quad scans for "time-travel" imagery over US landmarks | 🟣 USGS permanent |
| 19.8 | **NLS Scotland historic maps + NL Topotijdreis** | NLS georeferenced maps viewer/WMTS (`maps.nls.uk`) † · Topotijdreis WMTS via PDOK (`service.pdok.nl/hwh/topotijdreis`) — capabilities not greppable, † verify | Scotland 1840s–1960s; Netherlands 1815–today, national historic raster stack | None | NLS: free to use with attribution for most, per-collection † · Topotijdreis: open (PDOK family) † | 🟡 | Tile path with a year-parameter (time-travel mode for Europe's oldest mapped places) | 🟣 NLS + Kadaster institutions |
| 19.9 | **GEBCO Undersea Feature Names gazetteer** | Download from GEBCO data-products page (page URL moved, re-locate on `gebco.net`) **†** | ~30k named seamounts, trenches, abyssal plains — ocean-floor placemarks | None | Public domain (same GEBCO policy as 1.5) | 🟡 | Bundle as undersea placemarks — the "Atlantis-finder" layer for ocean zoom-ins | 🟣 GEBCO/IHO |

**Extras verdict:** 19.1/19.2/19.6/19.9 are cheap, licence-clean and distinctive (wildlife, fossils, time zones, seafloor names); 19.7/19.8 give EarthCraft a genuinely differentiating "time-travel" mode; 19.3/19.4 are pre-bake/fetch later; 19.5 stays opt-in due to the NC licence.

**Negative results (closed):** Grottocenter cave API returned nothing on the documented v1 endpoint (re-verify before use); GEBCO data-products URL restructure broke the gazetteer link (data itself still public); PBDB `colls/geo.json` path 404s — use `colls/list.json` (verified) or `colls/geo.json` per current API docs.
---
## Licence watch-list
### Long-term stability risk flags (second pass, 2026-10-18)
Graded for the "credible and long-term free" requirement. Sources not listed here are 🟣 institutional (NASA/NOAA/USGS/Copernicus-type, no flag needed).

| Source | Risk | Mitigation |
|---|---|---|
| **Esri World Imagery / Wayback (2.9 — EarthCraft's default today)** | Non-commercial-only ToU, revocable at will, unauthed use ToS-grey per Esri staff, some reference layers marked "(Retiring)" | Make it opt-in; default to GIBS + EOX; document the free-Developer-account path (2M tiles/mo, still non-commercial) |
| **RainViewer (2.4)** | Personal/educational terms, no SLA, radar owners can withdraw data at any time | Best-effort overlay only; fallback to NWS/DWD composites where available |
| **OpenSky (8.1)** | Anonymous = 400 credits/day, OAuth2-only since 2024, ToU non-commercial-research oriented | Route daily use through adsb.lol (ODbL, already in the binary); keep OpenSky as a higher-fidelity opt-in |
| **EOX cloudless (2.2)** | Free by goodwill for a decade, but not a licensed right; licence terms still unsettled (2.2 row) | Keep as imagery option with GIBS fallback; verify licence before shipping |
| **Mapterhorn (1.2)** | NLnet grant money is project-scoped; tiles served from individual-hosted infrastructure | Don't make it sole-source: 1.1 AWS Terrain (Registry of Open Data, very durable) + GEBCO/EMODnet cover the failure modes |
| **FreeLightning (16.7), Earth Traffic (16.1)** | Single maintainer / demo platform | Treat as accelerators with the direct NOAA S3 / vendored-archive fallback |
| **OpenAQ (13.2)** | v1/v2 retired Jan 2025 → v3 needs a key; per-source licences vary | Fetch `/v3/licenses`, respect flags, cache aggressively |
| **AISstream (8.2)** | Beta/unstable | OpenSeaFeed + digitraffic fallbacks are already wired |
| **EWDS / GloFAS (16.10)** | Explicitly "not for time-critical operation" | WMS-T tiles for visuals; GRIB analytics offline |
| **tile.openstreetmap.org in-app** | Faire caps heavy third-party usage of the main tile host | Prefer Protomaps PMTiles (5.3) for offline-grade basemap; keep OSM raster as light fallback |
| **NREL→NLR rebrand (3.2, 4.4)** | Domains changed once; may change again after rename | Keep the DOI/AWS mirrors (`registry.opendata.aws`) as anchors, not developer.nlr.gov URLs |
| **OpenTopography API (17.1)** | Free key: 50 calls/day non-academic; keys may not be embedded in an application; commercial integration requires a paid Enterprise key | Never ship a key — user-key from config or a pre-bake of ANADEM/archaeology tiles (17.2); treat the API as opt-in, not default |
| **OT archaeology collection licences (17.2)** | DCAT says CC BY 4.0 for Caracol Belize, but the dataset page itself reads "Use License: Not Provided" | Keep the † flag; cite DOI + PI (Weishampel 2019) and re-check the dataset page before shipping |

### Classic licence constraints (unchanged)
- **Non-commercial only:** Open-Meteo (2.7), RainViewer (2.4, personal / educational / small community), **Global Fishing Watch (13.1, CC BY-NC 4.0)**, **Smithsonian GVP (13.7)**, **openAIP (19.5, CC BY-NC 4.0 †)**. A German developer's design note reached the same conclusion and picked DWD data (CC BY 4.0, commercial use allowed) plus Bright Sky instead. If EarthCraft ever ships commercially, prefer 14.5 / 14.6 for weather in Germany.
- **Registration or keys required (regional):** NGII and VWorld (14.8, 14.9), Tianditu (14.10, with phone verification), IGN services (14.7, unconfirmed).
- **Restricted data:** IGN LiDAR HD withholds some zones (14.12). AW3D30 is a surface model, not bare earth (14.15).
- **Licence conflict or unclear:** EOX Sentinel-2 cloudless (2.2), UMN resources (5.4). ~~Hansen (6.5)~~ resolved 2026-10-18: **CC BY 4.0**.
- **Unmaintained or paused:** WRI power plants (5.2), SoilGrids REST API (6.4), AISstream (beta).
- **Key handling:** OpenWeatherMap, AISstream, FIRMS, NLR (NSRDB, Wind Toolkit), OpenAQ (13.2), Global Fishing Watch (13.1), GFW Data API (13.5), NRSC (14.31), **OpenTopography (17.1: user key from config, never embedded — policy verified verbatim)**, **openAIP (19.5)**, Movebank via Earthdata login (19.4). Never commit keys. Load from env or user config.
- **Rebrand note (verified 2026-10-18):** NREL is now the **National Laboratory of the Rockies (NLR)** — `developer.nrel.gov` and `www.nrel.gov` no longer resolve; use `developer.nlr.gov`. Affects 3.2, 4.4 and any NREL links elsewhere.
- **Attribution:** always show the source string required by the provider in the UI footer.
## Suggested build order
1. **Boundary detail upgrade (15.1 first, then 15.2 / 15.7–15.12).** One URL change (NE 50m → 10m) roughly triples country-border detail; then geoBoundaries ADM2 as the generic detailed-local layer, plus Census/France/Germany/Spain/Mexico plugins in the Brazil pattern. Overture per-country slices (15.4) can replace the fragile Overpass metro layer.
2. **Terrarium terrain (1.1).** Closes the "real 3D terrain" gap with the existing tile and cache code.
3. **GIBS (2.1) and weather overlays (16.3 night lights, 2.3 / 2.4, 14.5).** Reuse the imagery path, add a layer picker and timeline.
4. **Offline tiling script** (GDAL to PNG / PMTiles) for 3.1, 4.1, 6.1, 6.4, 16.8, 16.9. Keep it outside the app to preserve the no-system-deps promise.
5. **Point and vector layers:** 5.1, 5.2, 8.1–8.10, 13.2, 13.5–13.8, 16.4 (airports), rendered like existing placemarks.
6. **PMTiles reader** for 1.2, 5.3, 7.1 (and 15.4 slices).
7. **Other worlds (9.x)** after a body and projection abstraction exists.
8. **Regional terrain and imagery (section 14).** Start with 14.1 (Japan, plain XYZ), 14.2 / 14.3 (Netherlands) and 14.4 (Switzerland). They need no key and reuse the tile path. Add a per-source decoder for terrain, because encodings differ (Terrarium vs GSI).
9. **Cheap wins last:** 16.3 (night lights — plain GIBS layer), lightning via 16.7 with 16.5 as fallback.
10. **Archaeology pre-bakes (17.2, 17.5, 17.6, plus 17.17 Cape Town / 17.19 Rwanda / 17.20 Réunion-Mayotte)** with the same offline GDAL step, and the free site-vector overlays (17.13–17.15) on top; prototype the ANADEM hillshade (17.1) for Amazonia/Sahel before committing to any runtime key handling.
11. **Life and history layers (18, 19):** 18.1 (GeoNames filter — already downloaded), then 18.2/18.3 + 19.1/19.2 (placemark fetches), 19.6 (bundle time zones), 19.9 (seafloor names); time-travel imagery (19.7/19.8) after a tile-layer year-parameter exists.
---
## Source links
Where I could not confirm a URL directly, the row is marked † in its section instead.
### Sections 1–11

| # | Links |
|---|---|
| 1.1 | https://registry.opendata.aws/terrain-tiles |
| 1.2 | https://mapterhorn.com/ · https://protomaps.com/blog/mapterhorn-terrain/ · https://github.com/mapterhorn/mapterhorn · https://nlnet.nl/project/Mapterhorn/ · https://download.mapterhorn.com/attribution.json (full 151-source coverage + licence list, confirmed 2026-10-18) |
| 1.3 | https://tiles.emodnet-bathymetry.eu/ |
| 1.4 | https://registry.opendata.aws/copernicus-dem |
| 1.5 | https://www.gebco.net/data-products/gridded-bathymetry-data · https://www.gebco.net/news/release-gebco2026-grid · https://data.ceda.ac.uk/bodc/gebco/global/gebco_2026 |
| 2.1 | https://wiki.earthdata.nasa.gov/x/24soCw |
| 2.2 | https://eox.at/2017/03/sentinel-2-cloudless-original-tiles-available/ · https://osapiens.maps.eox.at/ |
| 2.3 | https://openweathermap.org/api/weathermaps |
| 2.4 | https://www.rainviewer.com/api/weather-maps-api.md · https://rainviewer.com/api.html · https://docs.rs/crate/rain_viewer/latest |
| 2.5 / 11.1 | https://github.com/cambecc/earth (see fork https://github.com/IMAGINARY/earth) |
| 2.6 / 11.2 | https://github.com/TomHeaven/netcdf2json · https://registry.opendata.aws/nasa-oscar/ · https://www.earthdata.nasa.gov/data/catalog/pocloud-oscar-l4-oc-interim-v2.0-2.0 · https://www.esr.org/data-products/oscar/ |
| 2.7 | https://zenodo.org/records/8112599 · https://freeapihub.com/apis/open-meteo-historical |
| 2.8 | https://gisgeography.com/best-free-gis-data-sources-raster-vector/ |
| 3.1 | https://globalwindatlas.info/about/KeyFeatures |
| 3.2 | https://developer.nlr.gov/docs/wind/wind-toolkit/wtk-download/ |
| 3.3 | https://casrai.org/guides/copernicus-climate-data-store-c3s |
| 4.1 | https://datacatalog.worldbank.org/search/dataset/0042316 · https://energydata.info/application/ |
| 4.2 | https://joint-research-centre.ec.europa.eu/photovoltaic-geographical-information-system-pvgis/getting-started-pvgis/api-non-interactive-service_en |
| 4.3 | https://power.larc.nasa.gov/ |
| 4.4 | https://developer.nlr.gov/docs/solar/nsrdb · https://developer.nlr.gov/docs/solar/nsrdb/psm3-2-2-download · https://registry.opendata.aws/nrel-pds-nsrdb |
| 5.1 | https://openinframap.org/about.html |
| 5.2 | https://old-datasets.wri.org/dataset/globalpowerplantdatabase · https://github.com/wri/global-power-plant-database |
| 5.3 | https://docs.protomaps.com/basemaps/downloads | PMTiles path; or pre-extract a country and bundle | 🔵 Protomaps Foundation; ODbL data is perpetual |
| 5.4 | https://geodata.libraries.mit.edu/record/gisogm:edu.umn:3d26e110c136 |
| 5.5 | https://guides.library.duke.edu/gisdata/gisdata_international |
| 6.1 | https://worldcover2021.esa.int/download · https://github.com/ESA-WorldCover/esa-worldcover-datasets |
| 6.2 | https://source.coop/streambatch/worldcereal · https://docs.digitalearthafrica.org/en/latest/data_specs/ESA_World_Cereal_specs.html |
| 6.3 | https://www.nass.usda.gov/Research_and_Science/Cropland/metadata/metadata_CDL24_FGDC-STD-001-1998.htm |
| 6.4 | https://www.isric.org/explore/soilgrids |
| 6.5 | https://developers.google.com/earth-engine/datasets/catalog/UMD_hansen_global_forest_change_2025_v1_13 · https://glad.earthengine.app/view/global-forest-change · https://storage.googleapis.com/earthenginepartners-hansen/GFC-2025-v1.13/download.html |
| 6.6 – 6.8 | https://en.wikipedia.org/wiki/List_of_GIS_data_sources · https://guides.library.duke.edu/gisdata/gisdata_international |
| 6.9 | https://www.helenmakesmaps.com/post/guide-to-free-gis-data-sources |
| 7.1 | https://source.coop/vida/google-microsoft-open-buildings/pmtiles | PMTiles over HTTP range-requests, or pre-extract a metro area for interactive use | 🟠 source.coop mirror of Google/Microsoft releases; per-source licences vary — verify the release you bundle |
| 7.2 | https://docs.overturemaps.org/guides/buildings | Pre-extract at build time (5.6 GB GeoParquet is not a runtime source) | 🔵 Overture Foundation (ODbL) — industry-backed and stable |
| 7.3 | https://human-settlement.emergency.copernicus.eu/ghs_wup_pop_r2025a.php |
| 7.4 | https://data.source.coop/nlebovits/ghsl/README.md |
| 8.1 | https://opensky-network.org/api/states/all |
| 8.2 | https://aisstream.io/documentation · https://github.com/aisstream | WS client + key env; keep the HTTP fallback (OpenSeaFeed / digitraffic are already in the binary) | 🟠 beta service — treat the fallbacks as the primary plan |
| 8.3 | https://celestrak.org/NORAD/elements/ |
| 8.4 | https://appliedsciences.nasa.gov/sites/default/files/2025-04/FIRMS_Part3_DMM_JO_MFC.pdf · https://atlas.co/data-sources/firms |
| 8.5 / 8.6 | https://www.spaceweather.gov/node/846 · https://github.com/NikolaTeslaInstitute/TeslaInstituteSpaceWeather |
| 8.7 – 8.9 / 11.3 | https://github.com/VrushankPatel/godseye |
| 8.7 | https://www.weather.gov/documentation/services-web-api · https://api.weather.gov/ |
| 8.10 | https://earthquake.usgs.gov/earthquakes/feed/v1.0/geojson.php · https://earthquake.usgs.gov/fdsnws/event/1/ · https://earthquake.usgs.gov/earthquakes/feed/policy.php |
| 9.1 – 9.3 | https://api.nasa.gov/mars-wmts/catalog · https://digital.gov/2016/04/01/your-trek-to-vesta-mars-starts-now |
| 10.1 | https://earth-search.aws.element84.com/v1 · https://element84.com/cloud-architecture/aws/sentinel-2-cloud-optimized-geotiffs-now-available-on-aws-registry-of-open-data/ |
| 10.2 | https://cng-python.gishub.org/book/case-studies/planetary-computer |
| 10.3 | https://gisgeography.com/best-free-gis-data-sources-raster-vector/ |
| 11.4 | https://github.com/tapiab/heimdall |
| 11.5 | https://docs.rs/async-tiff/ · https://developmentseed.org/async-geotiff/latest/blog/2026/02/03/introducing-async-geotiff/ |

### Section 13 (verified leads)

| # | Links |
|---|---|
| 13.1 | https://globalfishingwatch.org/our-apis/documentation · https://globalfishingwatch.org/our-apis/tokens |
| 13.2 | https://docs.openaq.org/using-the-api/api-key · https://docs.openaq.org/about/terms · https://docs.openaq.org/using-the-api/rate-limits |
| 13.3 | https://developers.google.com/earth-engine/datasets/catalog/GOOGLE_DYNAMICWORLD_V1 · https://dynamicworld.app/about/ |
| 13.4 | https://ladsweb.modaps.eosdis.nasa.gov/missions-and-measurements/products/VNP46A2/ · https://developers.google.com/earth-engine/datasets/catalog/NASA_VIIRS_002_VNP46A2 |
| 13.5 | https://tiles.globalforestwatch.org/openapi.json · https://data-api.globalforestwatch.org/ · https://data.globalforestwatch.org/ |
| 13.6 | https://api.tidesandcurrents.noaa.gov/api/prod/datagetter · https://tidesandcurrents.noaa.gov/web_services_info.html |
| 13.7 | https://volcano.si.edu/gvp_termsofuse.cfm · https://volcano.si.edu/database/list_volcano_holocene_excel.cfm · https://www.ngdc.noaa.gov/hazel/hazard-service/api/v1/volcanolocs |
| 13.8 | https://www.adsbexchange.com/community/developer-hub/ · https://api.adsb.lol/ · https://airplanes.live/ |

### Section 14 (non-English sources)

| # | Links |
|---|---|
| 14.1 | https://maps.gsi.go.jp/development/elevation.html · https://maps.gsi.go.jp/development/demtile.html · https://docs.rs/crate/demtile/latest · https://zenn.dev/shi_works/articles/6f50506d5bc3b0 |
| 14.2 | https://service.pdok.nl/hwh/luchtfotorgb/wmts/v1_0?request=GetCapabilities&service=wmts |
| 14.3 | https://data.overheid.nl/en/dataset/48457-brt-achtergrondkaart · https://www.pdok.nl/introductie/-/article/basisregistratie-topografie-achtergrondkaarten-brt-a- |
| 14.4 | https://atlas.co/data-portals/swisstopo-geo-admin-ch/ · https://opendata.swiss/en/group/envi?page=9 · https://www.baselland.ch/politik-und-behorden/direktionen/volkswirtschafts-und-gesundheitsdirektion/amt-fur-geoinformation/newsletter/freier-zugang-zu-geodaten-und-geodiensten-von-swisstopo |
| 14.5 / 14.6 | https://opendata.dwd.de/README.txt · https://www.ki-syndikat.de/tools/dwd-opendata/ · https://wetterdienst.readthedocs.io/en/latest/_sources/data/provider/dwd/index.md · https://meetingorganizer.copernicus.org/EMS2022/EMS2022-468.html · https://gitlab.com/brun-soft/das-etb/-/work_items/210 (design note, source of the licence comparison) |
| 14.7 | https://geoservices.ign.fr/sites/default/files/2024-06/Tuto_Geoplateforme.pdf |
| 14.8 | https://www.data.go.kr/catalog/15059358/openapi.json |
| 14.9 | https://www.data.go.kr/data/15058704/openapi.do · https://www.data.go.kr/data/3073144/openapi.do |
| 14.10 | https://lbs.tianditu.gov.cn/server/MapService.html · https://iclient.supermap.io/docs/leaflet/TiandituTileLayer.html · https://cloud.tencent.com/developer/inventory/23674/article/1709969 |
| 14.11 | https://inspire-geoportal.ec.europa.eu/srv/api/records/spaignMDT05 · https://inspire-geoportal.ec.europa.eu/srv/api/records/spaignwcs_mdt · https://datos.gob.es/en/catalogo/e00125901-spaignmdt |
| 14.12 | https://www.data.gouv.fr/datasets/mns-lidar-hd/ · https://geoservices.ign.fr/sites/default/files/2025-09/Offre_Produit_LiDAR_2025-08.pdf | Pre-extract LiDAR tiles per region | 🟠 IGN licence to confirm; ZICAD zones withheld |
| 14.13 | https://catalog.data.gov/harvest_record/add61b59-6572-44b4-9b15-17a193319ae3/raw · https://object.cloud.sdsc.edu/v1/AUTH_opentopography/www/metadata/CH_50cmDEM_metadata.pdf | Pre-extract | 🟣 swisstopo OGD |
| 14.14 | https://community.openstreetmap.org/t/ahn-gebruiken-als-achtergrondlaag-voor-bewerken/73056 · https://download.mapterhorn.com/attribution.json (AHN5 licence) | Pre-extract (already inside Mapterhorn 1.2 — prefer that) | 🟣 CC BY 4.0 |
| 14.15 | https://www.eorc.jaxa.jp/ALOS/en/dataset/aw3d30/data/aw3d30v4.0_product_e_1.0.pdf · https://data.amerigeoss.org/dataset/alos-world-3d-30m | Pre-extract after free registration | 🟠 JAXA free with account; DSM caveat |
| 14.16 | https://www.mlit.go.jp/plateau/learning/tpc30/ · https://www.moguravr.com/?p=315618 · https://wiki.openstreetmap.org/wiki/JA:MLIT_PLATEAU · https://zenn.dev/rescuenow/articles/ff5dca69861b94 |
| 14.17 | https://developers.google.com/earth-engine/datasets/tags/mapbiomas-public · https://brasil.mapbiomas.org/wp-content/uploads/sites/4/2023/08/Nota_Tecnica_MapBiomas_Pastagem.pdf · https://api.openstreetmap.org/user/pedro_tharg/diary/406596 |
| 14.18 | https://zenodo.org/records/8239305 · https://essd.copernicus.org/articles/16/1353/2024/ · https://gee-community-catalog.org/projects/glc_fcs/ |
| 14.19 | http://www.pcn.minambiente.it/viewer/ · https://www.alspergis.altervista.org/data/ortofoto_pcn.html |
| 14.20 | https://www.geoportale.piemonte.it/geonetwork/srv/search?format=OGC:WMTS |
| 14.21 | http://www.territorio.provincia.tn.it/portal/server.pt/community/lidar/847/lidar/23954 |
| 14.22 | https://mapy.geoportal.gov.pl/wss/service/PZGIK/ORTO/WMTS/StandardResolution · https://mapy.geoportal.gov.pl/wss/service/PZGIK/ORTO/WMTS/HighResolution · https://www.geoportal.gov.pl/pl/dane/ortofotomapa-orto/ |
| 14.23 | https://www.geoportal.gov.pl/pl/dane/dane-pomiarowe-lidar-lidar/ | Pre-extract LAS/GeoTIFF | 🟣 same as 14.22 |
| 14.24 | https://dataforsyningen.dk/ · https://datafordeler.dk/dataoversigt/geodanmark-ortofoto/ortofoto-foraar-web-mercator-wmts/ |
| 14.25 | https://www.lantmateriet.se/sv/geodata/vara-produkter/oppnadata/ |
| 14.26 | https://www.kartverket.no/en/api-and-data/terms-of-use · https://norgeibilder.no |
| 14.27 | https://www.maanmittauslaitos.fi/en/maps-and-spatial-data/datasets-and-interfaces/product-descriptions/elevation-model-2-m · https://www.maanmittauslaitos.fi/ortokuvien-ja-korkeusmallien-kyselypalvelu |
| 14.28 | https://gagnatorg.lmi.is · https://gee-community-catalog.org/projects/iceland_dem/ |
| 14.29 | https://www.map.is/ |
| 14.30 | https://rosreestr.gov.ru/press/news/470917/ · https://community.openstreetmap.org/t/wms/50567?page=14 |
| 14.31 | https://bhuvan-app3.nrsc.gov.in/data/ · https://bhuvan.nrsc.gov.in/ |
| 14.32 | https://sehirharitasiapi.ibb.gov.tr/ · https://data.ibb.gov.tr |
| 14.33 | https://parselsorgu.tkgm.gov.tr |

### Section 15 (borders, second pass)

| # | Links |
|---|---|
| 15.1 / 15.15 | https://github.com/nvkelso/natural-earth-vector (sizes verified live 2026-10-18) |
| 15.2 / 15.3 | https://www.geoboundaries.org/api/current/gbOpen/ALL/ADM0/ · https://github.com/wmgeolab/geoBoundaries/tree/main/releaseData/CGAZ · Runfola et al. 2020, PLoS ONE (geoBoundaries paper) |
| 15.4 | https://docs.overturemaps.org/schema/reference/divisions/ · blob release `2026-09-23.1` verified via anonymous S3/Azure listing |
| 15.5 / 15.6 | https://gadm.org/license.html · https://salb.un.org/terms-of-use (both read verbatim; excluded for shipping) |
| 15.7 | https://www2.census.gov/geo/tiger/GENZ2023/shp/ (sizes verified) |
| 15.8 | https://www12.statcan.gc.ca/census-recensement/2021/geo/sip-pis/boundary-limites/index2021-eng.cfm · https://geo.statcan.gc.ca/geo_wa/rest/services/2021/Cartographic_boundary_files/MapServer |
| 15.9 | https://data.geopf.fr/telechargement/download/ADMIN-EXPRESS-COG/ (Atom API verified keyless) |
| 15.10 | https://www.bkg.bund.de → Open Data · WFS `sgx.geodatenzentrum.de` (GeoNutzV) |
| 15.11 | https://centrodedescargas.cnig.es (CC BY 4.0 per metadata) |
| 15.12 | https://www.inegi.org.mx/geo/contenidos/geoestadistica/ (Libre Uso MX) |
| 15.13 | https://data.linz.govt.nz/ · https://www.stats.govt.nz/products/digital-boundary-files |
| 15.14 / 15.16 | https://download.geofabrik.de (ODbL footer) · https://osm-boundaries.com (OAuth-gated download) |

### Section 16 (second-pass additions)

| # | Links |
|---|---|
| 16.1 – 16.2 | https://traffic.elemnt.earth/ · https://opendata.ndw.nu/ · https://github.com/graphhopper/open-traffic-collection · TomTom free-tier status via https://developer.tomtom.com/store/ |
| 16.3 | GIBS GetCapabilities (layer `VIIRS_Black_Marble', `VIIRS_CityLights_2012`, `VIIRS_Night_Lights`) |
| 16.4 | https://davidmegginson.github.io/ourairports-data/ |
| 16.5 / 16.6 / 16.7 | `s3://noaa-goes19` GLM-L2-LCFA · https://www.blitzortung.org/en/coverage_maps.php · https://www.freelightning.com/ |
| 16.8 / 16.9 | https://www.ngdc.noaa.gov/mgg/global/ · https://www.ngdc.noaa.gov/geomag/emag2_download.html |
| 16.10 | https://ewds.climate.copernicus.eu/datasets/cems-glofas-forecast · https://global-flood.emergency.copernicus.eu |
| 16.11 | https://download.geonames.org/export/dump/ |

Esri terms (2.9): https://www.esri.com/en-us/legal/terms/web-site-service (§2.2) · Wayback ToU via https://github.com/Esri/wayback-core · attribution string from item `10df2279f9684e4a9f6a7f08febac2a9`.

### Section 17 (archaeological LiDAR, third pass)

| # | Links |
|---|---|
| 17.1 / 17.2 | https://opentopography.org/developers (API keys, rate limits and embedding ban quoted verbatim, read 2026-10-18) · https://portal.opentopography.org/apidocs/ · https://portal.opentopography.org/stac/raster_catalog.json · https://portal.opentopography.org/csw/catalog.json (DCAT licence fields grepped live) |
| 17.1 | https://doi.org/10.5069/G9736P4G (ANADEM South America DTM) · https://doi.org/10.5069/G9028PQB (Copernicus GLO-30) · https://doi.org/10.5069/G9V12301 (GEDI L3) |
| 17.2 | https://doi.org/10.5069/G9833Q42 (Caracol/Chiquibul Belize LiDAR; S3 `.laz` verified keyless) · https://doi.org/10.5069/G95B00NF (Middle Usumacinta) · https://doi.org/10.5069/G9416V95 (Quirigua UAV) |
| 17.3 | https://epqs.nationalmap.gov/v1/docs · https://portal.opentopography.org/apidocs/#/Public/getUsgsDem |
| 17.4 | https://www.earthdata.nasa.gov/engage/open-data-services-software ("full and open sharing" policy) · https://nsidc.org/data/atl08 |
| 17.5 | https://api.figshare.com/v2/articles/22202395 · https://figshare.com/articles/dataset/…/22202395 (CC BY 4.0, files and sizes verified via API) |
| 17.6 / 17.7 | https://doi.org/10.5281/zenodo.10334054 (Stonehenge LiDAR landscape, CC BY 4.0) · https://doi.org/10.5281/zenodo.5087620 (DTM-anomaly geoglyph toolkit, CC BY 4.0) |
| 17.8 | https://datadryad.org/dataset/doi:10.5061/dryad.k51j708 (PACUNAM derived data, CC0, 25.04 MB total) · https://doi.org/10.5281/zenodo.7983013 (Santa Rosa Xtampak — restricted) |
| 17.9 | https://archaeologydataservice.ac.uk/terms-and-conditions/ (per-collection licence wording quoted verbatim) |
| 17.10 / 17.11 | https://geosampa.prefeitura.sp.gov.br/PaginasPublicas/_SBC.aspx · GeoNetwork record "Nuvem de pontos MDT 2020" (`metadados.geosampa.prefeitura.sp.gov.br`, licence read directly) · https://igc.sp.gov.br (downloads; bot-blocked to curl) · geoportal.igc.sp.gov.br:6080 WMS |
| 17.12 | https://www.inegi.org.mx/programas/mde/ · https://www.inegi.org.mx/app/biblioteca/ficha.html?upc=702825796341 · https://www.inegi.org.mx/app/geo2/elevacionesmex/ |
| 17.13 / 17.14 | https://www.nature.com/articles/s41467-018-03510-7 (supplementary geoglyph vectors) · https://journal.caa-international.org/articles/10.5334/jcaa.45 (Table 1 UTM coords of 57 mound villages) |
| 17.15 | https://doi.org/10.6084/m9.figshare.c.5262530 (Loreto 415-site collection, CC BY 4.0) |
| 17.16 | Negative evidence: Pärssinen et al. 2026, *Nature* 657:420–425 (raw LiDAR "restricted under Brazilian law") · Prümers 2022 OA copy https://ore.exeter.ac.uk/29787914 (DTM on request) · Rostain 2024 AAM https://archimer.ifremer.fr/doc/00872/98412 (supplementary only) |
| 17.17 | https://odp-cctegis.opendata.arcgis.com/search?tags=dem · https://cityimg.capetown.gov.za/erdas-iws/esri/GeoSpatial%20Datasets/rest/services/Elevation_CCT_GLM2019_5m_DEM/MapServer · …/Elevation_DTM_2m/MapServer (both live-read) · specs via https://gis.westerncape.gov.za/server2/rest/services/SpatialDataWarehouse/Elevation/MapServer/layers |
| 17.18 | https://catalogue.ceda.ac.uk/uuid/a2e82c7f92dc4f389a7fb7e4e6629c9e/ (OGL v3, fetched directly) |
| 17.19 | https://download.mapterhorn.com/sources/rw.tar · https://github.com/WASAC/rwanda-dem (licence string from `attribution.json`) |
| 17.20 | https://cartes.gouv.fr/rechercher-une-donnee/dataset/IGNF_RGE-ALTI · https://www.data.gouv.fr/datasets/mnt-lidar-hd · download.mapterhorn.com tarballs (frrgealti1reunion, frhd2975) |
| 17.21 / 17.22 | https://zenodo.org/records/10377387 (Kilwa Kisiwani, CC BY-NC-SA 2.0) · https://www.zamaniproject.org/site-zimbabwe-Great-Zimbabwe.html (point clouds on request) |
| 17.23 | https://ngigeoportal.dlrrd.gov.za/cdngiportal/ (portal state read) · https://gis-ethekwini.opendata.arcgis.com/ · commercial GeoSpace/Hexagon note per LIDAR Magazine 2026-02-16 |
### Section 18 (general archaeology / heritage layers, third pass)

| # | Links |
|---|---|
| 18.1 | https://www.geonames.org/export/codes.html (S-class codes `RUIN`, `PYR(S)`, `RLG`, `WALLA`, `TMB`, `SHRN`, `HSTS`, `FT`, `WRCK` read verbatim 2026-10-18) |
| 18.2 | https://pleiades.stoa.org/places/423025/json (API live-tested) · https://pleiades.stoa.org/datasets/4.1 · https://pleiades.stoa.org/about/license (footer "Creative Commons Attribution 3.0 License (cc-by)" quoted verbatim) |
| 18.3 | https://ows.emodnet-humanactivities.eu/wms?SERVICE=WMS&REQUEST=GetCapabilities (layers `heritageshipwrecks`, `heritageshipwrecksv2`, `wwshipwrecks` verified) · https://www.emodnet-humanactivities.eu/view-data |
| 18.4 | https://www.nauticalcharts.noaa.gov/ (ENC downloads; S-57 `WRECKS`/`OBSTRN` object classes) |
| 18.5 | https://www.nps.gov/subjects/nationalregister/database.htm |
| 18.6 | https://historicengland.org.uk/listing/the-list/ (Cloudflare-gates plain clients, verified) · https://canmore.org.uk · https://coflein.gov.uk · https://maps.archaeology.ie (301 verified) |
| 18.7 / 18.8 | https://whc.unesco.org/en/list/xml (Cloudflare-gated to plain curl, verified) · https://www.unesco.org/en/underwater-cultural-heritage (no API found) |
| 18.9 | https://query.wikidata.org (Q839954 "archaeological site") · https://perio.do (CC0) |

### Section 19 (nature, science and historic maps, third pass)

| # | Links |
|---|---|
| 19.1 | https://api.gbif.org/v1/species/match?name=Aquila%20chrysaetos (live-tested) · https://www.gbif.org/developer/summary · https://registry.opendata.aws/gbif |
| 19.2 | https://paleobiodb.org/data1.2/colls/list.json?lngmin=10&lngmax=15&latmin=50&latmax=55&limit=3&show=full (live-tested) · https://paleobiodb.org |
| 19.3 | https://www.glims.org/RGI/ (site live) · https://nsidc.org/data/nsidc-0770 (RGI 7.0) · https://www.glims.org |
| 19.4 | https://www.movebank.org (terms-of-use page live) |
| 19.5 | https://www.openaip.net (licence per site †) |
| 19.6 | https://github.com/evansiroky/timezone-boundary-builder (README licence paragraph quoted verbatim: ODbL for data, MIT for code) |
| 19.7 | https://www.usgs.gov/programs/national-geospatial-program/topoview · https://basemap.nationalmap.gov/arcgis/rest/services/USGSTopo/MapServer |
| 19.8 | https://maps.nls.uk (licence per collection †) · https://service.pdok.nl/hwh/topotijdreis/wmts/v1_0 (capabilities not greppable — verify in browser) |
| 19.9 | https://www.gebco.net/data-products/ (locate the Undersea Feature Names gazetteer download; URL restructured) |
