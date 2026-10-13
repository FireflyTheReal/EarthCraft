[**English**](README.md) | [Português (Brasil)](README.pt-BR.md)

# EarthCraft 🌍

A 3D globe in the style of Google Earth, written in pure Rust (egui + CPU ray-casting). Runs on Windows, macOS and Linux with no system dependencies beyond a graphics driver.

### Build and run
Requires Rust 1.82+ (https://rustup.rs):

    cargo run --release

Linux (if something is missing):  `sudo apt install libxkbcommon-dev libwayland-dev libx11-dev` / `sudo apt install build-essential pkg-config libatk1.0-dev libgtk-3-dev`

An internet connection is required to download map tiles.

### Features

- 3D globe with orbit, zoom, view rotation (heading) and a compass
- Satellite imagery (Esri), streets (OSM) and topography (OpenTopoMap), with multi-level tile zoom and on-disk caching
- Place and address search (Nominatim), plus direct "lat, lon" coordinate input
- Animated fly-to (double-click, search or list)
- Placemarks with KML import/export
- Geodesic distance ruler (great-circle)
- Lat/lon grid, atmosphere, stars, real-time sunlight (day/night)
- Scale bar, cursor coordinates, PNG screenshot, context menu

### Controls
Drag: rotate globe · Right-drag / Shift+drag: rotate view · Wheel / +/-: zoom · Double-click: fly to point · Arrow keys: move · N: north up

### Not implemented (would require proprietary data/infrastructure)

Real 3D terrain, 3D buildings, Street View, Google Earth Studio, geotagged photos, narrated tours, data layers (weather, oceans, Mars/Moon) and icons
Tiles come from third-party services: please respect their terms of use (OSM limits heavy usage).
