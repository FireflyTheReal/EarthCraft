[English](README.md) | [**Português (Brasil)**](README.pt-BR.md)

# EarthCraft 🌍

[![Listed on Remade](https://remade.wiki/badges/listed-on-remade.svg)](https://remade.wiki/projects/earthcraft/)

Globo 3D estilo Google Earth em Rust (egui + renderização do globo por ray-casting na CPU, com aceleração opcional na GPU).
Roda em Windows, macOS e Linux sem dependências de sistema além do driver gráfico.

### Compilar e executar
Requer Rust 1.89+ (https://rustup.rs):

    cargo run --release

Linux (se faltar algo): `sudo apt install libxkbcommon-dev libwayland-dev libx11-dev` / `sudo apt install build-essential pkg-config libatk1.0-dev libgtk-3-dev`

Uma conexão com a internet é necessária para baixar os tiles do mapa.

### Funções

- Globo 3D com órbita, zoom, rotação da vista (heading) e bússola
- Imagens de satélite (Esri), ruas (OSM) e topografia (OpenTopoMap) com tiles em múltiplos níveis de zoom e cache em disco
- Carregamento em segundo plano da visão de satélite de todo o globo na distância inicial; tiles detalhados carregam ao aproximar
- Limpeza dos tiles baixados em disco pelas configurações
- Busca de lugares/endereços (Nominatim) e entrada direta de coordenadas "lat, lon"
- Voo animado até o destino (duplo clique, busca ou lista)
- Placemarks com importação/exportação KML
- Régua de distância geodésica (grande círculo)
- Grade lat/lon, atmosfera, estrelas, luz solar em tempo real (dia/noite)
- Barra de escala, coordenadas do cursor, captura de tela PNG, menu de contexto
- Renderização do globo por ray-casting na CPU ou GPU (opcional) em segundo plano, mantendo retorno automático à CPU em casos não suportados

### Controles
Arrastar: girar · Botão direito/Shift+arrastar: girar vista · Roda/+/-: zoom ·
Duplo clique: voar · Setas: mover · N: norte para cima

No Windows e no macOS, o menu nativo **Arquivo** abre/salva KML e salva capturas PNG. Use Ctrl (Windows) ou Cmd (macOS): O para abrir, S para salvar, Shift+S para Salvar como e Shift+E para uma captura. As capturas usam a pasta escolhida nas Configurações. No macOS, Sair fica no menu do aplicativo.

### Não implementado (exigiria dados/infra proprietários)
Terreno 3D com relevo real, prédios 3D, Street View, Google Earth Studio,
fotos geolocalizadas incorporadas e integração completa em 3D de terreno ou nuvens volumétricas
Os tiles são serviços de terceiros: respeite os termos de uso (OSM limita uso intenso).

### Desenvolvimento

O aplicativo fica em `apps/earthcraft`, com bibliotecas em `crates/`.
Veja a [arquitetura](docs/ARCHITECTURE.pt-BR.md), o [guia de contribuição](docs/CONTRIBUTING.md)
e as [regras para agentes](AGENTS.md) para entender os módulos e continuar o desenvolvimento.


### Explorer add-on (October 2026) / Módulo Explorar

Open **Explorar +** in the top bar for: coordinates and external OSM details of
search results; Wikimedia Commons photo search; editable narrated tours using
the operating system TTS (Linux requires `espeak`); external real-world
weather/cloud/ocean/aircraft/satellite data websites; links to NASA Moon and
Mars resources; local PNG/JPEG planet image preview; save and reload custom
`.earthplanet` manifests (keep the accompanying `*-texture.png/jpg` file in
the same directory); and preview custom sky/galaxy images.

**Limitações:** O Explorador não implementa relevos tridimensionais reais,
malhas de edifícios em 3D ou cartões completos de lugares do Google Maps.
A Lua e o planeta personalizado usam malhas desenhadas pelo egui, sem
aceleração direta pelo novo renderizador de GPU. Camadas de aviões e de
meteorologia dependem de serviços de terceiros e conexão com a internet.
A compilação e execução nativa não puderam ser verificadas neste ambiente
por ausência de toolchain Rust.

### Explorer language / Idioma do Explorar
The Explorer panel and toolbar labels follow the existing top-right language selector (English / Português (Brasil)). User-authored narration is preserved when changing language.
O painel Explorar e a barra superior acompanham o seletor de idioma no canto superior direito. Textos de narração editados pelo usuário não são traduzidos automaticamente.


### Aceleração por GPU (opcional)

O EarthCraft possui um renderizador opcional via `wgpu`, portado da versão GPU.
Por segurança, a CPU continua sendo o padrão. Para experimentar GPU, use
`EARTHCRAFT_RENDERER=auto` (quadros grandes) ou `EARTHCRAFT_RENDERER=gpu`
(forçar tentativa). Falhas de inicialização ou indisponibilidade de GPU
retornam à CPU. Visualizações muito próximas e a grade usam CPU.
Consulte [docs/GPU_RENDERER.md](docs/GPU_RENDERER.md) para limitações e testes.
