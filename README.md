# EarthCraft 🌍
## English
A 3D globe in the style of Google Earth, written in pure Rust (egui + CPU ray-casting). Runs on Windows, macOS and Linux with no system dependencies beyond a graphics driver.

### Build and run
Requires Rust 1.82+ (https://rustup.rs):

    cargo run --release

Linux (if something is missing):  `sudo apt install libxkbcommon-dev libwayland-dev libx11-dev`

An internet connection is required to download map tiles.

### Features
3D globe with orbit, zoom, view rotation (heading) and a compass
Satellite imagery (Esri), streets (OSM) and topography (OpenTopoMap), with multi-level tile zoom and on-disk caching
Place and address search (Nominatim), plus direct "lat, lon" coordinate input
Animated fly-to (double-click, search or list)
Placemarks with KML import/export
Geodesic distance ruler (great-circle)
Lat/lon grid, atmosphere, stars, real-time sunlight (day/night)
Scale bar, cursor coordinates, PNG screenshot, context menu

### Controls
Drag: rotate globe · Right-drag / Shift+drag: rotate view · Wheel / +/-: zoom · Double-click: fly to point · Arrow keys: move · N: north up

### Not implemented (would require proprietary data/infrastructure)

Real 3D terrain, 3D buildings, Street View, historical imagery, Google Earth Studio, geotagged photos, narrated tours, data layers (weather, oceans, Mars/Moon), KMZ/icons. Tiles come from third-party services: please respect their terms of use (OSM limits heavy usage).

## Brazilian Portuguese
Globo 3D estilo Google Earth em Rust puro (egui + renderização por ray-casting em CPU).
Roda em Windows, macOS e Linux sem dependências de sistema além do driver gráfico.

### Compilar e executar
Requer Rust 1.82+ (https://rustup.rs):

    cargo run --release

Linux (se faltar algo): `sudo apt install libxkbcommon-dev libwayland-dev libx11-dev`

### Funções
- Globo 3D com órbita, zoom, rotação da vista (heading) e bússola
- Imagens de satélite (Esri), ruas (OSM) e topografia (OpenTopoMap) com tiles em múltiplos níveis de zoom e cache em disco
- Busca de lugares/endereços (Nominatim) e entrada direta de coordenadas "lat, lon"
- Voo animado até o destino (duplo clique, busca ou lista)
- Placemarks com importação/exportação KML
- Régua de distância geodésica (grande círculo)
- Grade lat/lon, atmosfera, estrelas, luz solar em tempo real (dia/noite)
- Barra de escala, coordenadas do cursor, captura de tela PNG, menu de contexto

### Controles
Arrastar: girar · Botão direito/Shift+arrastar: girar vista · Roda/+/-: zoom ·
Duplo clique: voar · Setas: mover · N: norte para cima

### Não implementado (exigiria dados/infra proprietários)
Terreno 3D com relevo real, prédios 3D, Street View, histórico de imagens, Google Earth Studio,
fotos geolocalizadas, tours narrados, camadas de dados (clima, oceanos, Marte/Lua), KMZ/ícones.
Os tiles são serviços de terceiros: respeite os termos de uso (OSM limita uso intenso).
