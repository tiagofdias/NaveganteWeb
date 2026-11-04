# Perfis e descontos Navegante

Investigação em 3 de novembro de 2026. A consulta e a ativação de novos descontos ainda não estão implementadas na interface. O percurso abaixo foi aprofundado para a mudança de perfil Normal para +65.

## Normal para +65: percurso confirmado no APK

### 1. Obter uma sessão de leitura real

A leitura devolve `data.sessionId`. É este identificador, não o número visível do cartão ou o `read_id`, que a app usa neste percurso. Evidência: `t8.Y0` passa `C0521e.f10128b` (`sessionId`) a `g1.e`; `t8.C0745w0` chama `F7.r.k`, que chama `G7.C0129l.a`.

### 2. Consultar o tipo de benefício reconhecido

`GET https://pcgi.tmlmobilidade.pt/app/edge/gratuity/{sessionId}` devolve o campo `gratuity`. O enum `H7.c` contém `SUB18`, `SUB23`, `PLUS65`, `PLUS65GL` e `NONE`.

Uma consulta real ao cartão ligado devolveu `{"gratuity":"NONE"}`. Isto só prova que essa consulta não disponibilizou um benefício; não fornece a causa nem comprova a idade do titular.

### 3. Consultar as condições detalhadas

`POST .../gratuity/{sessionId}`, sem corpo de dados do titular, devolve:

| Campo | Informação apresentada na app |
| --- | --- |
| `age` | Verificação de idade/data de nascimento |
| `dataComplete` | Dados completos e NIF válido/confirmado |
| `singleActiveCard` | Apenas um cartão ativo; pode ser necessário cancelar cartões antigos |

Evidência: `G7.C0133p`, `G7.r`, `F7.r.d`, `t8.K0`, `y8.h`, `B4.j` e os recursos `personalization_terms_*`.

A página de termos usa o tipo de benefício para apresentar “Idade +65 verificada”. A informação sobre documentos em falta vem do serviço/portal; não existe neste pedido um campo em que o cliente declare uma idade alternativa.

As duas tentativas de obter uma nova leitura para consultar estes detalhes falharam por timeout do servidor de bilhética. Portanto, ainda não foi recebida a resposta detalhada deste cartão. Não se deve atribuir o `NONE` à idade, ao NIF ou a duplicação de cartões sem essa evidência.

### 4. Corrigir dados pelo canal oficial, quando necessário

O botão “Atualizar Dados” de `RenewTermsFragment` abre `UpdateUserDataFragment`. O WebView carrega a URL de configuração `O7.b.f3904c` com `sessionId` na query string. A URL identificada na configuração nativa é `https://mais-agil.navegante.pt/customer-data/eligibility`.

Esta página é um serviço oficial externo. A documentação exigida e a verificação de identidade devem seguir as indicações desse serviço. A integração web ainda não reproduz esse percurso e não deve enviar uma data de nascimento ou NIF inventados para tentar alterar o resultado.

### 5. Pedir e executar a personalização assinada

Depois de concluídas as condições:

1. `GET .../personalizations/{sessionId}` obtém `keyRef`, `request64`, `signature` (`G7.C0129l.d`, `F7.r.f`, `t8.J0`).
2. `PersonalizationFirstStepFragment` encaminha esse pedido assinado para o SDK (`t8.b1`, `p066h8.n.g`, parâmetro de etapa `false`).
3. `GET .../personalizations/{sessionId}/load` obtém a autorização da segunda etapa (`G7.C0129l.c`, `F7.r.e`, `t8.H0`).
4. `PersonalizationSecondStepFragment` executa o segundo pedido assinado (`t8.T0`, `p066h8.n.g`, parâmetro de etapa `true`).
5. É necessário voltar a ler o cartão e confirmar os perfis e o catálogo após a operação.

O payload assinado deve ser interpretado sem o alterar. O SDK trata caminhos `personalization` e `loading_image` além dos carregamentos normais, conforme `W4.A`. A implementação Python atual só suporta o fluxo assinado `loads`; aceitar aqueles caminhos exige implementar e testar uma operação separada de personalização. Não basta reutilizar a rota de compra nem mudar o valor de um perfil em JSON.

### Autorização e erros

O pedido de elegibilidade real funcionou sem login. Isso não prova que todos os passos ou todos os titulares dispensem outros meios de verificação. Não foram reutilizadas credenciais da app, falsificados resultados de elegibilidade ou pedidos assinados.

`p066h8.n.g` trata especificamente o erro de servidor `IS_NOT_ELIGIBLE`. Uma recusa desse tipo precisa de resolução dos dados/direito junto do serviço oficial; não pode ser convertida em sucesso pela interface.

Não foi enviado nenhum comando de personalização ou de alteração de desconto ao cartão durante esta investigação.

## Descontos já atribuídos

O resultado da leitura contém `profiles`, com perfil e validade, e o catálogo é pedido ao serviço oficial para essa leitura. Os preços não são reduzidos localmente. Um desconto atribuído ao titular e reconhecido pelo serviço deve refletir-se nos produtos/preços que esse serviço disponibiliza; é necessário testar com cartões que tenham esses perfis.

## Ativar ou renovar um perfil

O APK contém:

- `G7.C0129l.a`: consulta `GET gratuity/{identificador}`.
- `G7.C0129l.k`: verificação de condições com `POST gratuity/{identificador}`.
- `H7.c`: tipos `SUB18`, `SUB23`, `PLUS65`, `PLUS65GL` e `NONE`.
- `H7.h`: resultado de verificação de idade, dados completos e cartão ativo único.
- `G7.C0129l.d`: pedido assinado em `GET personalizations/{identificador}`.
- `G7.C0129l.c`: pedido assinado em `GET personalizations/{identificador}/load`.
- `p066h8.n.g`: envio da personalização assinada ao SDK e tratamento de inelegibilidade.

Estes caminhos são evidência do fluxo da app, não uma API pública documentada. É ainda necessário confirmar os identificadores exigidos, a autorização, a sequência dos pedidos e a escrita PC/SC para personalizações. A implementação atual de carregamento aceita apenas autorizações para `loads`; não suporta personalização.

## Percurso proposto

1. Mostrar os perfis e respetiva validade obtidos na leitura.
2. Consultar a elegibilidade no serviço oficial e apresentar a resposta, sem atribuir descontos por decisão local.
3. Encaminhar eventuais atualizações de dados ou comprovativos para o canal oficial aplicável.
4. Quando o serviço disponibilizar a autorização assinada, executar a personalização pelo leitor numa operação distinta da compra.
5. Ler novamente o cartão para confirmar o perfil e atualizar o catálogo/preços.

O percurso de carregamento normal continua sem login obrigatório. As condições de acesso à personalização devem ser verificadas separadamente, sem presumir que uma conta é sempre necessária ou sempre dispensável.

## Regras oficiais consultadas

- [Navegante no Google Play](https://play.google.com/store/apps/details?id=pt.card4b.navegante&hl=pt): descreve a verificação de elegibilidade e ativação de gratuitidade.
- [CP: Passe Navegante](https://www.cp.pt/info/w/navegante-pass): indica a ativação/atualização do desconto +65 na app.
- [Metro: descontos e condições](https://www.metrolisboa.pt/comprar/): inclui Circula PT, gratuitidade jovem e +65. Para Circula PT, indica pedidos/renovações nos espaços de atendimento e documentação comprovativa.

Não foi confirmado que todos os descontos disponíveis no sistema tenham ativação remota. Não devem ser apresentados botões de ativação para perfis cujo fluxo ainda não esteja integrado e validado.
