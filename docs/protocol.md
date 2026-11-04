# Protocolo e evidência de interoperabilidade

Investigação feita em 2 de novembro de 2026 com o APK fornecido pelo utilizador. Os caminhos abaixo são observados na implementação, não constituem uma API pública garantida pela TML. Mudanças nos serviços podem exigir atualização.

## Endereços identificados

| Serviço | Endereço |
| --- | --- |
| Bilhética TipTop | `https://lisbon.tiptop.spiws.net/tiptop/` |
| API da aplicação | `https://pcgi.tmlmobilidade.pt/app/edge/` |
| API do portal My Navegante | `https://pcgi.tmlmobilidade.pt/my-navegante/api/` |
| Portal de conta | `https://my.navegante.pt` |

As primeiras duas URLs foram identificadas por análise local da função `GetInfo` em `libnativelib.so`, no split ARM64. A URL da API My consta do JavaScript público do portal oficial. Não se reutilizaram valores de configuração que não fossem URLs públicas.

## Cartão físico

A validade dos títulos usa `contract.temporal_validity.status`: `1` = VALID, `2` = EXPIRING, `3` = EXPIRED. Confirmado no bytecode do inicializador e construtor de `i8.k` e no consumidor `S2.g.w`, no APK fornecido. Estes códigos não são a legenda de `issuing.validity.status`. A interface apresenta o estado dos títulos, distingue meses futuros pelos campos de mês/ano e assinala dados insuficientes sem presumir validade. A informação corresponde à última leitura; após uma gravação confirmada, uma nova leitura atualiza os títulos e períodos apresentados.

`J4.o`, `O7.c` e `W4.C0298j0` identificam os AIDs:

- Legado: `315449432E494341` (`1TIC.ICA`).
- Lisboa: `A000000291D62000019101`.

`W4.C0298j0` constrói SELECT com `00 A4 04 00`. O serviço também usa a classe `94`. No cartão testado por contacto, é necessário pedir os dados de resposta explicitamente. Um comando case 4 pode executar e devolver `6Cxx`; repetir OPEN SESSION provoca `6985`. A implementação usa GET RESPONSE para recuperar os dados pendentes sem repetir a abertura.

O serviço mantém o leitor selecionado durante a comunicação e usa acesso exclusivo e reset na ligação. Não escolhe automaticamente dispositivos como a YubiKey detetada neste computador.

## Envelope do gateway

Fontes: `W4.C0320v`, `W4.j1`, `W4.A`.

```json
{
  "id": 201,
  "version": 1,
  "request": {
    "method": "POST",
    "path": "reads?device=tiptop",
    "headers": {"Accept": "application/json", "Content-Type": "application/json"},
    "body": "[2,1,121,0,0,0,0,0,0,0,8,0]"
  }
}
```

O HTTP externo pode responder 200 e o envelope conter outro `status`; ambos precisam de ser tratados. O corpo interno é uma string JSON. A resposta fornece um identificador temporário, `PO_command`, progresso e dados interpretados.

O cliente executa os comandos de leitura e devolve os resultados com `PUT reads/{id}` até receber os dados finais. O catálogo usa `GET products?read_id={id}` e `GET products/{product}?read_id={id}`.

## Formato PO

Fontes: `W4.C0327y0` e `W4.C0298j0.h`.

- Cabeçalho: versão `2` e número de ações.
- Ação `73` (`0x49`): tipo, flags, comprimento de quatro bytes big endian e APDU.
- Resposta `73`: tipo, três bytes zero, código de resultado, comprimento de quatro bytes e resposta APDU incluindo SW1/SW2.
- Ações `121` e `122`: deteção do tipo de cartão e confirmação.

A implementação valida o lote completo antes de o executar. Tipos desconhecidos não são enviados ao cartão. O modo de leitura admite seleção, leitura de ficheiros, abertura/fecho de sessão e uma verificação de PIN cifrado recebida do servidor após abertura bem-sucedida. Não aceita um PIN escolhido pelo navegador e não repete uma verificação recusada.

## Configuração do produto

Fontes: `p066h8.n.a`, `t8.g1.E`, `j8.M`, `j8.x0`, `j8.J`.

`PUT products/{product}?read_id={id}` recebe o ID da configuração, os atributos e o preço escolhido. O ID enviado em cada atributo é o UUID do **valor escolhido**, não o nome do atributo pai. Os valores monetários são números inteiros em cêntimos. Campos opcionais nulos devem ser omitidos; a sua inclusão provocou 400 no serviço testado. O pedido corrigido respondeu 200 e devolveu a configuração atualizada.

## Conta

O portal oficial expõe `POST login` com `{environment: "DEFAULT", email}` para solicitar código e `PUT login` com ambiente, e-mail e código. Também oferece `POST login` com e-mail e palavra-passe.

A sessão web obtida é trocada por `GET navegante/exchange-token` e depois por `POST my-navegante/login` na API da aplicação. Os modelos de troca estão em `C7.e`, `C7.q`, `C7.t` e `C7.w`.

Um User-Agent genérico recebeu 403 nos testes; o cliente identifica a versão de protocolo da aplicação e identifica-se como cliente PC/SC. Isto não substitui a autenticação. O pedido de código do utilizador recebeu 503; a causa interna não foi confirmada.

## Compra e carregamento

Fontes: `p017b8.i`, `p027c8.*`, `t8.C0749y0`, `t8.Q0`, `p066h8.p` e `W4.A`.

- `POST payments/checkout/{sessionId}`: dados de contacto/faturação e método `MBWAY` (ou nulo para produto gratuito). O `sessionId` vem dos dados interpretados do cartão.
- `GET sales/{saleId}/status`: estado da venda e `isLoaded`.
- `POST payments/confirmation/{saleId}`: confirmação e autorização de carregamento (`keyRef`, `request64`, `signature`).
- `GET sales/history` e `GET sales/history/{saleId}`: histórico autenticado e recuperação.
- O gateway aceita o envelope assinado e devolve comandos PO; os resultados seguem para `PUT loads/{id}`.

As chaves de carregamento não são calculadas ou extraídas localmente. A autorização vem do serviço oficial e o cliente apenas a encaminha. O navegador não pode fornecer APDUs ou uma autorização assinada arbitrária.

O carregamento admite um conjunto limitado de comandos Calypso de bilhética. Alteração de PIN/chaves e invalidação de cartão não são suportadas. Uma instrução não suportada interrompe a operação. Pagamento e gravação permanecem por validar em operação real; testes simulados não demonstram que todos os passos de produção sejam compatíveis.

## Referências primárias

- [TML: introdução à APEX](https://go.tmlmobilidade.pt/reference/apex).
- [Eclipse Keyple: aplicação Calypso](https://keyple.org/learn/user-guide/calypso-application/).
- [Eclipse Keyple: instruções Calypso](https://github.com/eclipse-keyple/keyple-card-calypso-java-lib/blob/main/src/main/java/org/eclipse/keyple/card/calypso/CardCommandRef.java).

Estas referências ajudam a confirmar os conceitos e as instruções Calypso. O contrato específico de Navegante foi investigado no APK e nas respostas dos serviços, sem presumir que exemplos genéricos de outros sistemas de transportes fossem compatíveis.
