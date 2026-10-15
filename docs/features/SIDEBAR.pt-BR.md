# Painéis laterais

[English](SIDEBAR.md) | [Português (Brasil)](SIDEBAR.pt-BR.md)

A barra de navegação à esquerda alterna entre **Explorar**, **Camadas** e
**Lugares**. Clicar no botão ativo recolhe o painel, mantendo a barra disponível.
O botão **☰** da barra superior também fecha/reabre o painel. O **☰** no cabeçalho
do painel oculta a barra de navegação de forma independente; abas de texto
mantêm a alternância entre painéis disponível.

- **Explorar** reúne navegação, resultados de busca, régua, exportação de capturas
  e ajuda dos controles. O botão de régua da barra superior alterna a mesma
  medição e exibe seus controles. Trocar de painel preserva medidas e desfazer/refazer.
- **Camadas** reúne imagens de base/Wayback e as sobreposições geográficas e ao
  vivo existentes, com estados, credenciais e atribuições.
- **Lugares** contém a árvore editável de pastas e suas ações de arquivo. A árvore
  cresce com o painel, substituindo a área fixa de 128 pontos. Abrir, Salvar e
  Salvar como permanecem visíveis; opções de caminho e de soltar arquivos ficam
  em Arquivos KML / KMZ. Duplo clique, menus de contexto, visibilidade e arrastar
  para pastas mantêm seu funcionamento.

Use **Fixar** em Lugares para manter sua árvore abaixo de Explorar ou Camadas.
Arraste o divisor horizontal para redimensionar essa área. Selecionar Lugares
usa o painel inteiro, sem duplicar a árvore. Desafixe pelo cabeçalho para liberar
espaço. O atalho de marcador exibe seu editor de nome, inclusive quando fixado.
As áreas de conteúdo e Lugares fixado têm rolagem independente.

A seleção do painel, abertura, visibilidade da barra, fixação, largura e altura
fixada são salvas em `sidebar` no `settings.json` existente. Arquivos antigos
usam Explorar, painel aberto e barra visível; campos inválidos ou desconhecidos
preservam valores válidos ou padrões. A gravação usa o serviço assíncrono existente.

Esta é a primeira etapa do painel lateral para a versão 0.2.0. Não acrescenta
itens futuros à barra, modifica a fidelidade KML, cria um registro de camadas nem
introduz uma régua flutuante. O editor existente de coordenadas exatas da régua
continua acessível pelo painel. Busca em Lugares, virtualização da árvore e um
inspetor de objetos separado ficam para etapas futuras.
