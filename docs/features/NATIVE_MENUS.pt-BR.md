# Menus do desktop

[English](NATIVE_MENUS.md) | [Português (Brasil)](NATIVE_MENUS.pt-BR.md)

O macOS usa a barra de menus nativa. Windows e Linux oferecem o mesmo catálogo
de comandos pelo botão **EarthCraft** na barra superior; a integração de menus
nativos continua exclusiva do macOS.

| Menu | Comandos disponíveis |
| --- | --- |
| EarthCraft | Sobre, Configurações, Sair; macOS mantém Ocultar/Ocultar outros/Mostrar todos |
| Arquivo | Abrir KML/KMZ, Salvar, Salvar como, Salvar captura |
| Editar | Desfazer e Refazer, respeitando editores de texto com foco |
| Visualizar | Painel lateral, controles de navegação, grade, atmosfera, luz solar, norte para cima, vista global |
| Adicionar | Marcador no centro da câmera, pasta na pasta selecionada ou em Meus lugares |
| Ferramentas | Medir distâncias, Explorar |
| Janela | Tela cheia, Minimizar; macOS registra o menu Janela do sistema |
| Ajuda | Controles/atalhos, documentação, relato de problemas; macOS registra o menu Ajuda do sistema |

As marcações acompanham o estado atual, inclusive alterações no painel lateral
ou nas Configurações. A captura fica desativada até existir uma imagem exibida.
Desfazer/Refazer seguem o histórico da régua ou o editor de texto com foco.
A régua alterna a mesma medição e exibe seus controles; marcador e pasta exibem
o editor de nome existente.

Configurações usa Cmd+, no macOS e Ctrl+, nos demais sistemas. Adicionar marcador
usa Cmd/Ctrl+Shift+P; Adicionar pasta, Cmd/Ctrl+Shift+N; e Painel lateral,
Cmd/Ctrl+Alt+B. Os atalhos de arquivos e histórico permanecem iguais. A Ajuda
lista os atalhos com os modificadores da plataforma.

Callbacks enfileiram ações tipadas e solicitam redesenho. O aplicativo aplica
alterações, abre diálogos nativos e envia tarefas de arquivo na atualização, na
thread da janela. Os objetos de menu permanecem nessa thread até o encerramento.
Os dois catálogos traduzem rótulos de menus, janelas e ajuda. Links só abrem ao clicar.

Os menus expõem comandos funcionais, sem itens inativos para editores de projetos,
passeios ou geometrias ainda planejados. Comandos específicos de visibilidade dos
painéis Lugares/Camadas podem acompanhar a entrega do painel lateral depois.
