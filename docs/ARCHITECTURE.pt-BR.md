# Arquitetura

[English](ARCHITECTURE.md) | [Português (Brasil)](ARCHITECTURE.pt-BR.md)

O EarthCraft usa um workspace Cargo com um aplicativo e três bibliotecas.
Esta reorganização mantém os recursos, controles, serviços e textos existentes.

| Pacote | Responsabilidade |
| --- | --- |
| `apps/earthcraft` | Janela, egui, entrada do usuário, estado, diálogos e coordenação |
| `crates/earthcraft-core` | Matemática do globo, câmera, idioma e valores de placemarks |
| `crates/earthcraft-data` | Tiles, busca, limites geográficos, cache, configurações e KML |
| `crates/earthcraft-render` | Renderização por CPU, overlays geográficos e coordenação de frames |

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
a renderização continua no Rayon, com `wide` calculando quatro raios `f64`
independentes por grupo. Conversão geográfica e seleção de tiles/LOD continuam
escalares; grupos incompletos não desenham pixels nem solicitam tiles adicionais.
A biblioteca GeoRust `kml`, baseada em quick-xml,
lê e escreve KML. Uma etapa privada de preparação XML remove Points vazios,
fecha contêineres incompletos e envolve coordenadas antigas em Point, evitando
panics e loops no EOF. Não há um parser KML antigo em paralelo. O aplicativo
continua importando um marcador na primeira coordenada de cada Placemark e
exportando Points, sem adicionar renderização de linhas/polígonos ou KMZ.
CDATA mantém seu texto literal; coordenadas/metadados inválidos retornam erros
tipados, sem a filtragem numérica e o unescape extra do parser antigo.
Serde mantém os padrões tolerantes das configurações e da busca;
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
dupla. `dcamera::rh::view::look_to_mat3` constrói a rotação da vista, e
`dcamera::rh::proj::directx::perspective_infinite` constrói a projeção. Matrizes
ficam em cache por basis. A vista usa X à direita, Y para cima e -Z para frente;
a conversão final inverte Y para a tela. O nome `directx` indica a convenção de
profundidade, sem trocar o backend gráfico. O renderer mantém o campo de visão,
o picking analítico de raios/esfera e a verificação do horizonte, sem clipping
pela profundidade projetada.

Amostragem, interpolação, iluminação e clamping mantêm RGB em `Vec4` com SIMD até
a conversão para bytes; a quarta componente fica zerada e o alfa final permanece
opaco. `lerp_monotonic` mantém a forma da interpolação; operações podem ser
fundidas em algumas plataformas, respeitando a tolerância existente dos fixtures.
Bytemuck cria views RGBA sem cópias nem suposições de alinhamento SIMD.

## Memória e carregamento

O cache de tiles decodificados usa `moka::sync::Cache`, com alvo ponderado de
256 MiB e política LRU. O peso é o tamanho do buffer RGBA. O limite é best-effort:
pode haver excesso temporário enquanto a manutenção remove entradas. A métrica
`resident_bytes()` é uma estimativa e pode atrasar em relação às alterações.
Texturas, geometria, metadados e referências temporárias ficam fora dessa métrica.
Os 256 MiB não são alocados inteiros na inicialização. Depois da visão global,
sobram cerca de 171 MiB para detalhes, reduzindo recarregamentos ao revisitar áreas.

Tiles prontos são compartilhados por `Arc`, sem adquirir o mutex de requisições.
O mapa separado de carregamentos/falhas mantém a deduplicação, o máximo de 48
requisições e a espera de 15 segundos antes de tentar novamente. Tiles grandes
demais ou com falhas não entram no cache de pixels. Os workers mantêm referências
fracas enquanto ociosos, e cada conclusão atualiza a geração e repinta a interface.

O renderer processa blocos de oito linhas com Rayon, compartilhando consultas e
referências de tiles entre linhas vizinhas. Essas referências são liberadas ao
terminar o bloco, sem manter outro cache entre frames.

As formas projetadas de bordas e nomes ficam em um cache de uma única vista.
A chave inclui a base completa da câmera, posição/tamanho da viewport, altitude,
DPI, camadas administrativas, idioma e escala do texto. Cada patch de dados
invalida o cache; conclusão de tiles e repaints de outros painéis reutilizam as
formas e galleys. Régua e placemarks continuam dinâmicos. A projeção ainda ocorre
na thread da interface quando a vista muda.

Na reconstrução, a ordenação estável por prioridade e rank permanece. Um R*-tree
de `rstar` indexa os retângulos já aceitos, com interseção inclusiva: bordas que
se tocam também são rejeitadas. Padding, cores e limite de 500 nomes não mudam.
O índice é somente de tela, sem alterar visibilidade geográfica ou carregamento;
é descartado após reconstruir. Para conjuntos pequenos, uma varredura linear
pode ser mais rápida; o benchmark separa esse custo do ganho de reutilização.

Um coordenador de earthcraft-render chama o renderer fora da thread da interface, usando
o pool existente do Rayon. Há no máximo um frame ativo, uma solicitação pendente
substituível e um resultado concluído. Novas posições de câmera substituem o
trabalho pendente. Mudanças de provedor, data histórica, tamanho/aspecto ou efeitos
descartam resultados incompatíveis. Durante movimento contínuo, a vista concluída
compatível aparece enquanto a mais recente é calculada; bordas, nomes e escala
usam a câmera desse frame para permanecer alinhados. A interface recebe um repaint
ao concluir e mantém a atualização de texturas, projeção de overlays e conversão
RGBA, que serão otimizadas separadamente.

O coordenador não segura o mutex durante rasterização ou admissão de tiles. O
aquecimento global aguarda sua ociosidade para dar prioridade aos tiles visíveis.
Fechar a janela descarta solicitações pendentes sem esperar pelo frame ativo;
o coordenador libera seus recursos ao terminar esse trabalho. Não cria um pool
por frame nem ocupa os workers limitados de parsing/decodificação do Tokio.

`TileWarmup` percorre uma vez por sessão os níveis 0–4 de satélite, dos mais
simples aos mais detalhados: 341 tiles RGBA de 256×256 somam cerca de 85 MiB.
Eles usam o mesmo cache em memória de 256 MiB e o cache em disco. O aquecimento
só admite trabalho quando há menos de duas requisições em andamento, deixando
quatro dos seis workers disponíveis para a vista atual. A aplicação o executa
depois das requisições visíveis, com nova verificação em 100 ms, e pausa durante
interação, abaixo dos 14.000 km iniciais ou em outra camada. Tiles prontos,
carregando ou com falha recente são ignorados; falhas continuam com a política
normal de novas tentativas sob demanda. O plano termina com a aplicação.

Depois do aquecimento, girar a visão global evita esperar por novos tiles de
satélite. Na primeira execução o download continua progressivo. Telas maiores,
zoom próximo ou remoção de tiles da memória podem exigir novos carregamentos.
Rasterização, envio da textura e desenho dos limites ainda consomem tempo.
Ruas, topografia e imagens históricas permanecem sob demanda; o serviço público
OSM [proíbe prefetch](https://operations.osmfoundation.org/policies/tiles/).

As configurações oferecem **Limpar cache em disco** para imagens baixadas.
`DiskCache`, na camada de dados, remove somente os diretórios dos quatro provedores.
Configurações, idioma, cache de limites e arquivos pessoais são preservados.
A operação é assíncrona, com botão desabilitado até a conclusão e resultado na
barra de status. Tiles decodificados permanecem na memória; o plano de aquecimento
é cancelado para a sessão. Novas áreas ou a próxima inicialização podem baixar
tiles novamente.
O tamanho ao lado do botão soma os bytes dos arquivos desses mesmos diretórios,
excluindo configurações, limites e espaço adicional do sistema de arquivos.
A medição usa um worker de bloqueio, coordenado com a limpeza, e atualiza no
máximo a cada cinco segundos enquanto a janela está aberta, além de após limpar.
Não há varredura no loop de pixels nem na thread da interface.

Um bloqueio assíncrono coordena leitura, escrita e limpeza nesta instância.
Cada requisição registra a geração do cache em disco ao entrar na fila; a limpeza
avança essa geração, impedindo que trabalhos anteriores restaurem arquivos removidos.
Links simbólicos dos provedores são removidos sem seguir seus destinos. Falhas
na limpeza são exibidas em inglês. Isso não coordena múltiplas instâncias do app.
O cache persistente ainda usa arquivos de tiles; MBTiles e um limite de tamanho
em disco são etapas propostas, ainda não implementadas.

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

## Limite entre interface e bibliotecas

`earthcraft-render::overlays` contém projeção de bordas, seleção e ordenação
estável dos nomes, ocupação de retângulos com rstar e visibilidade geográfica
com geo-index. As APIs usam emath, sem depender de egui. A aplicação mede fontes,
atribui cores e traços, cria formas egui e mantém o cache dessas formas.
O coordenador de frames fica em render e recebe callbacks de repaint/admissão.
A interpolação de voo da câmera fica em `earthcraft-core::flight`; ranking,
deduplicação e combinação das sugestões ficam em `earthcraft-data::search`.
Temporização dos eventos, entrada, texturas e diálogos nativos continuam no app.
