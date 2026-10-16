# Exibição e entrada de coordenadas

[English](COORDINATE_DISPLAY.md) | [Português (Brasil)](COORDINATE_DISPLAY.pt-BR.md)

Configurações → Aparência → Coordenadas oferece **DD**, **DMS**, **DDM**, **UTM/UPS** e **MGRS**.
O padrão é DMS com duas casas decimais nos segundos. A precisão vale para o formato
selecionado; trocar de formato escolhe sua precisão padrão. Casas decimais afetam
apenas a exibição e o texto copiado, preservando valores geográficos e KML.
Dígitos MGRS por eixo indicam o tamanho da célula, não a exatidão das observações.

Cursor, centro da câmera e menu do botão direito seguem a seleção. Clique em uma
leitura da barra inferior para copiar coordenadas sem o rótulo traduzido. A área
do cursor mantém a precisão escolhida, ajustando a fonte quando necessário; a dica
mostra o valor completo. Formatos angulares mantêm a posição da vírgula estável,
inclusive quando o cursor sai do globo.

A busca reconhece os formatos independentemente da preferência de exibição:

| Formato | Exemplo |
| --- | --- |
| DD | `37.7749, -122.4194` |
| DMS | `37°46′29.64″N, 122°25′09.84″W` |
| DDM | `37°46.494′N, 122°25.164′W` |
| UTM | `18 N 585664.121 4511315.422` ou `18N 585664.121 4511315.422` |
| UPS | `UPS N 2000000 2000000` |
| MGRS | `18TWL8566411315` |

UTM usa um **hemisfério N/S** explícito, não a letra de faixa de latitude.
A entrada angular é latitude primeiro e aceita aspas ASCII ou símbolos de minuto
e segundo. Minutos e segundos devem ser menores que 60. Sinais contraditórios,
valores não finitos e coordenadas fora dos limites geram um erro local em inglês.
Consultas de coordenadas reconhecidas não acionam sugestões/geocodificação remotas;
nomes de lugares e endereços mantêm o comportamento anterior.

Core trata formatos, arredondamento, análise e validação. Data persiste
`coordinate_display` com leitura tolerante no `settings.json` existente;
arquivos antigos usam o novo padrão. O aplicativo desenha controles, ajusta texto
e copia valores. O editor existente da régua mantém seus valores decimais exatos.

A conversão usa [mgrs-convert](https://docs.rs/mgrs-convert/0.1.0/mgrs_convert/).
Testes locais detectaram rejeição de +180° e um panic em coordenadas MGRS equatoriais
válidas no geoconvert 1.0.2 sugerido pelo planejamento. A substituição evita manter
uma implementação privada de projeções. Os testes cobrem referências fixas,
polos, antimeridiano, zonas especiais, hemisférios, arredondamento DMS, entradas
inválidas e compatibilidade das preferências.
