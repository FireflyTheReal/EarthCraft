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
`core` usa `emath` para geometria da tela, glam para matemática e Fluent para
traduções, sem widgets ou janelas.
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

A lista global de imagens históricas continua sincronizada. O aplicativo possui
um único runtime Tokio para os serviços; reqwest reutiliza clientes HTTP.
Decodificação de imagens e projeção de JSON usam workers bloqueantes limitados;
a renderização continua no Rayon. quick-xml lê e escreve o subconjunto Point
existente do KML. Serde mantém os padrões tolerantes das configurações e da busca;
thiserror preserva causas estruturadas até a apresentação na interface.
O estado do aplicativo ainda pertence a uma estrutura privada, dividida em
módulos por responsabilidade.

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

Veja [as escolhas de bibliotecas](LIBRARY_OPTIONS.md) para os usos adotados e os
cuidados de compatibilidade. A câmera usa `DVec*`, `DMat*` e `DQuat` em precisão
dupla. Cores usam `Vec4` com SIMD; bytemuck cria views RGBA sem cópias e sem
presumir alinhamento de vetores SIMD.

## Memória e carregamento

O cache de tiles decodificados tem orçamento de 128 MiB, com remoção dos menos
usados recentemente. Texturas, geometria e referências temporárias do renderer
não fazem parte desse limite. Os workers mantêm referências fracas enquanto
ociosos e preservam a política de novas tentativas.

Países, estados e nomes globais carregam na inicialização. Nomes municipais do
Brasil carregam abaixo do limite existente de 300 km; geometria municipal e
metropolitana começa quando uma dessas camadas é ativada. As linhas usam `Arc`
para compartilhar coordenadas. GeoJSON é baixado em arquivos temporários e
projetado um elemento por vez com Serde; no máximo dois trabalhos ocorrem juntos,
e os estados brasileiros são processados sequencialmente.

## Traduções

Os catálogos Fluent ficam em `crates/earthcraft-core/locales/en.ftl` e `pt-BR.ftl`,
embutidos no executável. `Lang::text` fornece textos estáticos já formatados;
`Lang::format` recebe argumentos nomeados para mensagens completas. Há fallback
para inglês e testes de chaves, variáveis e textos originais. Os nomes geográficos
continuam vindos dos serviços. Consulte [o guia de localização](LOCALIZATION.md).
