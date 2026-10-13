[English](README.md) | [**Português (Brasil)**](README.pt-BR.md)

# EarthCraft 🌍

Globo 3D estilo Google Earth em Rust puro (egui + renderização por ray-casting em CPU).
Roda em Windows, macOS e Linux sem dependências de sistema além do driver gráfico.

### Compilar e executar
Requer Rust 1.88+ (https://rustup.rs):

    cargo run --release

Linux (se faltar algo): `sudo apt install libxkbcommon-dev libwayland-dev libx11-dev` / `sudo apt install build-essential pkg-config libatk1.0-dev libgtk-3-dev`

Uma conexão com a internet é necessária para baixar os tiles do mapa.

### Funções
- Globo 3D com órbita, zoom, rotação da vista (heading) e bússola
- Imagens de satélite (Esri), ruas (OSM) e topografia (OpenTopoMap) com tiles em múltiplos níveis de zoom e cache em disco
- Carregamento em segundo plano da visão de satélite de todo o globo na distância inicial; tiles detalhados carregam ao aproximar
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
Terreno 3D com relevo real, prédios 3D, Street View, Google Earth Studio,
fotos geolocalizadas, tours narrados, camadas de dados (clima, oceanos, Marte/Lua) e ícones
Os tiles são serviços de terceiros: respeite os termos de uso (OSM limita uso intenso).

### Desenvolvimento

O aplicativo fica em `apps/earthcraft`, com bibliotecas em `crates/`.
Veja a [arquitetura](docs/ARCHITECTURE.pt-BR.md), o [guia de contribuição](docs/CONTRIBUTING.md)
e as [regras para agentes](AGENTS.md) para entender os módulos e continuar o desenvolvimento.
