# Configurações

[English](SETTINGS.md) | [Português (Brasil)](SETTINGS.pt-BR.md)

Abra Configurações pela barra de ferramentas ou pelo menu do aplicativo
(Ctrl/Cmd+,). A janela redimensionável organiza os controles em Geral, Aparência,
Navegação, Globo, Gráficos, Dados e cache e Arquivos. A busca encontra nomes,
rótulos e textos de ajuda traduzidos. As categorias encontradas continuam
editáveis. Restaurar padrões e limpar o cache exigem confirmação na própria janela.

A aparência oferece temas Sistema/Escuro/Claro, fontes instaladas independentes
para interface e mapa, unidades, formato/precisão de coordenadas e escalas de
texto. O tema escuro continua como padrão. Um trabalhador limitado descobre as
fontes e carrega as selecionadas; as fontes incluídas continuam como alternativa
para caracteres ausentes. Fontes indisponíveis geram diagnóstico em inglês sem
apagar a preferência. A troca invalida o texto do mapa em cache.

A navegação ajusta sensibilidade de arraste, rotação e zoom da roda, além de
inverter a roda. Os padrões preservam os controles anteriores. Salve a câmera
atual como vista inicial ou restaure a vista original. Isso afeta a próxima
inicialização, sem mover a câmera atual. Teclado e gesto de pinça são preservados.

Os alvos padrão são 256 MiB para imagens decodificadas e 128 MiB para o cache em
disco. Alterar os limites exige reiniciar o EarthCraft. O alvo ponderado do Moka
permite excessos temporários durante a remoção e não limita a memória total do
processo. Referências de renderização e outros conjuntos de dados são separados.
O limite em disco inclui arquivos gerenciados e SQLite. A limpeza permanece
assíncrona, respeita a época do cache e preserva arquivos não relacionados.

As preferências permanecem no mesmo settings.json e arquivo de idioma, gravadas
pelo trabalhador de persistência. Campos inválidos mantêm valores válidos ou
padrões; números são limitados. A aparência não altera coordenadas ou documentos.
Os testes cobrem busca traduzida, fontes alternativas, persistência, validação
e limites reais do cache. O layout e a qualidade das fontes exigem revisão visual.
