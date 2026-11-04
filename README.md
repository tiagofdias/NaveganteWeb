# Navegante Web

Aplicação web local para consultar e carregar um passe Navegante com um leitor USB PC/SC, sem base de dados própria. Projeto independente, sem afiliação à TML.

## Usar o site online com o leitor USB

O site pode ser usado por qualquer pessoa com **Windows, Python 3.13 e um leitor PC/SC compatível**. Cada pessoa usa o seu próprio cartão e uma ponte no seu próprio PC; não existe uma conta de proprietário a limitar o acesso.

1. Descarregar este repositório em **Code → Download ZIP** e extrair a pasta.
2. Ligar o leitor e abrir `start-bridge.cmd`. Se o comando `python` não existir, instalar Python e ativar a opção de o adicionar ao PATH.
3. Colar no terminal o endereço HTTPS do site que se pretende autorizar. A ponte apresenta um código temporário; manter o terminal aberto.
4. No site, colar esse código em **Ligar ao meu PC** e permitir o acesso local/do dispositivo quando o navegador pedir.
5. Inserir o cartão, ler, escolher o carregamento e seguir os passos de pagamento e gravação.

Alternativa pelo terminal:

```powershell
python server.py --web-origin https://ENDERECO-DO-SITE.vercel.app
```

O domínio deve coincidir exatamente com o endereço aberto no navegador. Não são aceites wildcards nem outros sites. O código muda a cada arranque, só fica em memória e não é enviado para a Vercel. A ponte escuta apenas em `127.0.0.1:8765` e conserva as operações de cartão e bilhética no PC do utilizador. As operações reais continuam a depender dos serviços oficiais e da elegibilidade de cada cartão.

**Compatibilidade:** ponte Windows PC/SC; ligação HTTPS → localhost verificada em Edge. Preferir Edge ou Chrome atualizados, permitindo a ligação local ao site. macOS, Linux e outros navegadores não foram validados. Sem a ponte, está disponível o modo demo.

## Executar

Requisitos: Windows, Python 3.13 e leitor instalado. A aplicação usa apenas a biblioteca padrão do Python; não precisa de npm, pip ou Java para funcionar.

```powershell
python server.py
```

Abrir **http://127.0.0.1:8765**. Para terminar o serviço, usar Ctrl+C no terminal. O serviço escuta apenas no endereço local.

1. Selecionar o Conceptronic CSMARTID, apresentado neste computador como `THRC Smart Card Reader 0`.
2. Inserir o passe e escolher **Ler o passe**. A leitura consulta o serviço oficial de bilhética.
3. Selecionar um produto e uma opção. **Rever carregamento** configura o produto no serviço e apresenta o preço confirmado; não inicia um pagamento.
4. Confirmar cartão, título, opção e valor. Para produtos pagos, indicar um número português MB WAY e pedir o pagamento explicitamente. Não é necessário iniciar sessão — tal como na aplicação oficial, basta encostar o cartão.
5. Aprovar na aplicação MB WAY e atualizar o estado. Só após a autorização oficial fica disponível **Gravar carregamento no passe**.
6. Manter o cartão inserido e aguardar a confirmação de carregamento pelo serviço oficial.

## Modo demo

Escolher **Experimentar demo** na navegação para testar sem leitor ou cartão físico. O aviso de demonstração permanece visível e todos os dados são fictícios.

- Cenários: sem passe mensal, Metropolitano válido ou passe expirado, todos com saldo Zapping de exemplo.
- Produtos, preços, pagamento e gravação são simulados no navegador. Na demo, a oferta mensal segue a janela habitual: mês atual até dia 25, mês seguinte a partir de dia 26, com data de Lisboa. No modo real, os períodos continuam a ser definidos pela API oficial, sem impor este calendário localmente.
- Os dados do formulário já vêm preenchidos com exemplos. Não é enviado MB WAY, e-mail ou comando ao cartão.
- Depois de simular o carregamento, consultar novamente o cartão mostra o saldo ou título fictício atualizado.
- **Reiniciar demo** repõe o cenário. **Sair da demo** limpa os dados fictícios e volta à leitura real. A entrada na demo fica indisponível durante um pedido real em curso.
- Durante a demonstração, não há pedidos a `/api/`, base de dados ou armazenamento persistente. Ao entrar/sair do modo real, a aplicação pode consultar a lista de leitores.

Verificação de isolamento e fluxo completo: `npm ci` e `node tests/demo.cjs` (usa Edge, ou um navegador instalado pelo Playwright).

## Publicar a interface

`npm run build` prepara apenas a interface estática em `dist/`, configurada por `vercel.json` para a Vercel. A publicação mantém a ligação ao leitor através da ponte local, não executa PC/SC na cloud. Ver [arquitetura, limites do Hobby e testes](docs/deployment.md).

## O que foi efetivamente validado

| Funcionalidade | Evidência |
| --- | --- |
| Deteção e comunicação PC/SC | Testada com Conceptronic CSMARTID e cartão físico |
| Seleção da aplicação `1TIC.ICA` | Resposta `9000`, FCI de 36 bytes, protocolo T=0 |
| Leitura autenticada | Serviço oficial devolveu cartão, titular, emissão, perfis, contratos e eventos |
| Catálogo e preços | Consultados no serviço oficial para o cartão inserido |
| Configuração do produto | `PUT products/...` aceite com HTTP lógico 200; revisão de preço testada no navegador |
| Interface | Testada em Edge, em larguras de computador e telemóvel, sem erros JavaScript |
| Pagamento MB WAY | Utilizador confirmou um pagamento real sem login; testes de regressão com serviços simulados |
| Gravação | Implementada a partir do APK e coberta por testes simulados; não confundir pagamento com gravação confirmada |
| Recuperação por referência | Implementada com referência de venda; não requer conta |

Os testes automáticos não iniciam pagamentos nem enviam comandos de gravação a cartões reais. A leitura abre e fecha uma sessão Calypso e verifica o acesso aos registos usando a resposta fornecida pelo serviço oficial. Não é necessário iniciar sessão numa conta Navegante para carregar o cartão físico.

## Sem base de dados

- Cartões, configurações, tokens e referências ficam apenas na memória do processo.
- Sessões de leitura expiram após 15 minutos. Pedidos de compra ficam disponíveis em memória por até duas horas.
- Não se usa localStorage, sessionStorage, cookies de autenticação ou ficheiros de transações.
- Ao fechar ou reiniciar o serviço, perde-se o estado local. Para recuperar um pedido pendente, usa a referência de venda guardada após o pagamento.

Uma falha durante o pedido de pagamento bloqueia uma nova submissão nessa leitura. Uma falha após começar a escrita deixa a operação por confirmar e impede uma repetição automática. O estado `carregado` só é apresentado depois de o serviço oficial devolver `isLoaded: true`.

## Estrutura

- `server.py`: serviço HTTP local e validação da origem dos pedidos.
- `bridge.py`: domínio autorizado e código temporário de emparelhamento com o site.
- `bridge_launcher.py` / `start-bridge.cmd`: arranque interativo da ponte para outros utilizadores.
- `pcsc.py`: ligação Windows PC/SC, controlo do leitor e adaptação T=0.
- `tiptop.py`: protocolo de comandos do serviço de bilhética e carregamentos assinados.
- `official.py`: checkout, confirmação de pagamento e estado da venda (sem conta necessária).
- `commerce.py`: configuração e validação de preços em cêntimos inteiros.
- `service.py`: percurso da sessão, compra, gravação e recuperação.
- `sessions.py`: estado temporário em memória.
- `web/`: interface em HTML, CSS e JavaScript, sem dependências externas.
- `tests/`: testes sem rede/cartões e teste opcional de navegador com leitura física.
- `docs/protocol.md`: origem das integrações e limitações técnicas.

## Testes

```powershell
python -m unittest discover -s tests -v
node --check web/app.js
```

Os 20 testes unitários não enviam mensagens, não iniciam pagamentos nem acedem a um leitor. Incluem sessões expiradas, comandos inválidos, prevenção de escrita no modo de leitura, preços em cêntimos, submissão duplicada, gravação interrompida e checkout sem conta.

O teste `tests/browser.cjs` usa Playwright instalado apenas em `analysis/tools/browser/` e o Edge existente. A leitura real requer `NAVEGANTE_LIVE_READ=1`. Esse teste lê o cartão e configura um produto para revisão; não inicia sessão, não envia códigos, não paga e não grava títulos.

## APK e ferramentas de investigação

O pacote `navegante®-2026.10.19.apks`, fornecido pelo utilizador, foi analisado com JADX 1.5.6. Algumas classes precisaram do modo de descompilação simples. Os APKs, fontes descompiladas, bibliotecas e ferramentas de análise estão excluídos do Git e não são necessários para executar a aplicação.

Não foram incorporadas chaves de acesso ou credenciais embutidas da aplicação oficial. O servidor de bilhética é contactado por TLS; as autorizações assinadas de carregamento vêm dos serviços oficiais, após confirmação do pagamento.
