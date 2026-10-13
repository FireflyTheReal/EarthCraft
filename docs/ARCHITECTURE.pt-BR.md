# Arquitetura

[English](ARCHITECTURE.md) | [Português (Brasil)](ARCHITECTURE.pt-BR.md)

O EarthCraft usa um workspace Cargo com um aplicativo e três bibliotecas.
Esta reorganização mantém os recursos, controles, serviços e textos existentes.

| Pacote | Responsabilidade |
| --- | --- |
| `apps/earthcraft` | Janela, egui, entrada do usuário, estado, diálogos e coordenação |
| `crates/earthcraft-core` | Matemática do globo, câmera, idioma e valores de placemarks |
| `crates/earthcraft-data` | Tiles, busca, limites geográficos, cache, configurações e KML |
| `crates/earthcraft-render` | Renderização por CPU e prioridade dos tiles ausentes |

O aplicativo depende das três bibliotecas. `data` depende de `core`; `render`
depende de `core` e `data`. Nenhuma biblioteca depende do aplicativo.
`core` usa apenas `emath` para tipos de geometria da tela, sem widgets ou janelas.
`data` recebe um callback para solicitar repintura, sem depender de egui.

O manifesto raiz centraliza versões, metadados, perfis e `Cargo.lock`.
`cargo run --release` continua funcionando na raiz porque o membro padrão é
`apps/earthcraft`. Os assets continuam em `assets/` na raiz. Use `--workspace`
para executar verificações e testes de todas as bibliotecas.

## Onde continuar o trabalho

- Operações da câmera: `earthcraft-core/src/camera.rs`.
- Matemática e coordenadas: `earthcraft-core/src/math.rs`.
- Serviços, cache e persistência: módulos de `earthcraft-data/src/`.
- Pixels, amostragem e iluminação: `earthcraft-render/src/lib.rs`.
- Controles do globo: `apps/earthcraft/src/app/globe.rs`.
- Painéis e botões: `apps/earthcraft/src/app/ui/`.
- Busca e resultados assíncronos: `app/search.rs`.
- Importação, exportação e capturas: `app/files.rs`.
- Bordas, nomes e régua: `app/overlays.rs`.

`app/mod.rs` mantém o estado privado e o ciclo de atualização. Os módulos filhos
implementam comandos e interfaces sobre esse estado, sem expô-lo às bibliotecas.
As bibliotecas fazem o trabalho de domínio e retornam valores; o aplicativo
aplica resultados dos canais e atualiza a interface.

## Contratos preservados

Coordenadas internas usam `(latitude, longitude)` em graus; KML e GeoJSON usam
longitude primeiro. Altitude da câmera e distâncias usam quilômetros. Caminhos
de cache, arquivos `lang.txt`/`settings.json`, diretórios de saída, URLs,
attribuições, tempos limite, atalhos e limiares de zoom foram mantidos.
Não é necessária uma migração de dados.

A lista global de imagens históricas, a duração dos workers de tiles e o parser
KML original permanecem como antes. Uma alteração nesses comportamentos deve
ser tratada separadamente, com testes próprios. O estado do aplicativo ainda
pertence a uma estrutura privada, dividida em módulos por responsabilidade.

O empacotamento já referenciava `icon.icns` e `packaging/earthcraft.desktop`,
que não estão no repositório. Os caminhos foram ajustados para a nova estrutura,
mas os pacotes continuam sem validação até esses arquivos serem disponibilizados.

Leia [o guia de contribuição](CONTRIBUTING.md) para os comandos de verificação,
o checklist manual e a forma de adicionar recursos. As regras para agentes
estão no [AGENTS.md](../AGENTS.md). A [versão completa em inglês](ARCHITECTURE.md)
explica o fluxo de renderização e os limites entre os pacotes.

## Constantes e bibliotecas

Unidades físicas e limites compartilhados ficam em `earthcraft-core/src/lib.rs`.
Os parâmetros do renderer ficam no módulo privado `tuning`; políticas de cache
ficam em `data`; geometria e tempos da interface ficam nos módulos do aplicativo.
Nomes indicam unidades (`_KM`, `_DEG`, `_SECS`, `_PX`, `_POINTS`). Números iguais
com significados diferentes continuam separados. Os valores originais e a ordem
de cálculo foram preservados.

Veja [as opções de bibliotecas](LIBRARY_OPTIONS.md) para próximas mudanças em
JSON, KML, erros e vetores, com verificação de compatibilidade.
