# Painéis laterais

[English](SIDEBAR.md) | [Português (Brasil)](SIDEBAR.pt-BR.md)

Um seletor horizontal compacto dentro do painel alterna entre **Globo**,
**Camadas** e **Lugares**. Passe o cursor para ver os nomes. Selecionar o item ativo
mantém o painel aberto. O botão **☰** superior recolhe/reabre todo o painel,
incluindo o seletor. Não há opção separada para ocultar a navegação nem botão ×.
A largura padrão é 240 pontos, em vez de 270 mais uma barra lateral de 44 pontos.
O redimensionamento manual e as larguras salvas são preservados.

- **Globo** reúne navegação e rotação automática, busca, régua, capturas e ajuda.
  A régua da barra superior usa o mesmo estado e revela seus controles. Trocar
  de painel preserva medidas e histórico.
- **Camadas** organiza imagens, limites/rótulos/grade, vias/trânsito, meteorologia,
  aeronaves/navios/satélites e céu/iluminação. Os grupos são recolhíveis e mantêm
  estados, credenciais e atribuições junto aos controles correspondentes.
- **Lugares** contém a árvore editável e ações de arquivo. Abrir/Salvar/Salvar como
  ficam visíveis. **Arquivos KML / KMZ começa aberto** em uma área inferior
  reservada; a árvore um pouco menor tem rolagem independente e não oculta essa
  seção. A área de arquivos também rola em painéis fixados pequenos. Edição,
  duplo clique, menus de contexto, visibilidade e arrastar continuam disponíveis.

Use **Fixar** em Lugares para manter sua árvore abaixo de Globo ou Camadas.
Arraste o divisor para redimensionar. Selecionar Lugares usa o painel completo,
sem duplicar a árvore. O mesmo controle desafixa. O marcador da barra superior
revela/foca o editor de nome, inclusive quando fixado.

Seleção, abertura, fixação, largura e altura são salvas em sidebar no settings.json.
Arquivos antigos usam Globo aberto; campos inválidos preservam valores válidos
ou padrões. O antigo rail_visible é ignorado e não é mais gravado. A persistência
assíncrona e os formatos geográficos/documentais são preservados.

Esta etapa não adiciona painéis futuros, registro de camadas ou régua flutuante.
O editor de coordenadas exatas da régua permanece. Busca em Lugares,
virtualização e inspetor separado ficam para depois.
