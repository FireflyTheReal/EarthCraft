# Fundo celeste v1

[English](CELESTIAL_BACKGROUND.md) | [Português (Brasil)](CELESTIAL_BACKGROUND.pt-BR.md)

Camadas → Fundo celeste e Configurações → Globo ativam a Via Láctea atrás da Terra.
O padrão é desativado; ative explicitamente para usar o fundo. Durante carregamento/falha, as estrelas procedurais existentes
continuam disponíveis. Configurações mostra diagnóstico em inglês e nova tentativa.
Desativar restaura o caminho original e seus fixtures. Imagens, iluminação,
oclusão da Terra e solicitações de tiles permanecem iguais.

O mapa difuso celeste de 4096×2048 vem de
[NASA Deep Star Maps 2020, SVS 4851](https://svs.gsfc.nasa.gov/4851/), sem o primeiro
plano brilhante Hipparcos/Tycho. Não adicionamos estrelas procedurais sobre ele.
É uma imagem direcional, sem catálogo selecionável, busca de objetos, HiPS ou
navegação física pelo universo.

O manifesto em assets/sky registra URLs, hashes SHA-256, versão, créditos,
convenção e avaliação de uso. O JPEG de 2,5 MB é derivado SDR com Reinhard/sRGB,
sem preservar a fidelidade HDR. O EXR original fica na NASA; o script Python/FFmpeg
verifica e prepara o arquivo offline. Não há download/decodificação EXR em execução.
Os créditos NASA/Gaia aparecem no globo e nas Configurações. Capturas PNG recebem
metadados Source do frame exibido, sem copiar o framebuffer compartilhado ou
sobrepor interface à imagem limpa.

Core transforma raios fixos à Terra em ICRF/J2000 pelo modelo IAU 2006/2000A do
sofars 0.6.1, incluindo precessão/nutação/rotação. TT usa a tabela de segundos
intercalares; UT1 é aproximado por UTC e o movimento polar é zero. Serve para um
fundo visual 4K, não astrometria precisa. O relógio atual usa intervalos de um
minuto; não há simulador de tempo. RA 0h fica no centro, cresce para a esquerda
e o norte fica acima. Interpolação bilinear envolve longitude e limita os polos.
As licenças SOFA/sofars estão incluídas e disponíveis nas Configurações.

O modo realista reduz o fundo perto da superfície diurna entre altitudes solares
de −18° e −6°, com visibilidade total a partir de 100 km. O modo cinematográfico
ignora essa redução explicitamente. É uma política simples de visibilidade, não
um novo modelo de dispersão atmosférica. A atmosfera existente continua na frente.

Data decodifica uma imagem opcional em seu serviço limitado e desperta a UI em
sucesso/falha. A entrada tem limite de 8 MiB, dimensões fixas e alocação do decoder
limitada a 64 MiB. A imagem RGB compartilhada ocupa 24 MiB; buffers temporários e
bytes comprimidos são adicionais. Não entra no MBTiles de imagens terrestres.

Render amostra na CPU; o renderizador GPU experimental usa a mesma correção de
céu na CPU após leitura. Não é necessária textura celeste na GPU. Chaves de
apresentação rejeitam resultados antigos ao alterar fundo/modo/tempo. O fallback
da CPU permanece. Isso não certifica todos os dispositivos/plataformas.

Testes cobrem referência SOFA, orientação RA, emendas/polos, tempo, visibilidade,
decodificação assíncrona limitada, persistência, créditos PNG e pixels terrestres
inalterados. O teste explícito de paridade GPU também inclui o fundo real.
Qualidade HDR/8K+, catálogos, constelações e explorador astronômico ficam para
etapas posteriores. Consulte CONTRIBUTING.md para verificações visuais.
