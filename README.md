# Navegante Web

Demo online e aplicação Windows para consultar e carregar um passe Navegante com um leitor USB PC/SC, sem base de dados própria. Projeto independente, sem afiliação à TML.

**Site público:** https://navegante-web-xi.vercel.app · **Repositório:** https://github.com/tiagofdias/NaveganteWeb

## Usar a aplicação Windows

A homepage dá destaque ao download da aplicação Windows. A [demo opcional](https://navegante-web-xi.vercel.app/demo/) fica numa página separada e não contacta leitores, não cobra pagamentos nem carrega cartões reais.

A aplicação Windows 2.0 é **nativa, escrita em Rust**, com interface egui e acesso direto ao leitor PC/SC. Não usa Electron, WebView, Edge, Python ou servidor local.

1. [Descarregar o Navegante-Web.exe](https://github.com/tiagofdias/NaveganteWeb/releases/download/v2.0.0/Navegante-Web.exe) e abrir o ficheiro.
2. Escolher **Carregar passe**. A aplicação pede para ligar o leitor e inserir o cartão apenas quando ainda faltam. Com vários leitores, pede para escolher um.
3. Consultar titular, validade, títulos e saldo. Escolher **Carregar um passe** ou **Adicionar saldo**, selecionar a opção e rever o valor.
4. Confirmar o pagamento quando necessário e gravar o carregamento. Aguardar a confirmação antes de retirar o cartão.

Não é necessário instalar runtimes, abrir um terminal, copiar códigos ou iniciar sessão no GitHub/Vercel. A janela é desenhada diretamente pela aplicação; o leitor e os serviços oficiais são usados no PC do utilizador. As operações decorrem numa tarefa separada para manter a interface responsiva. Sessões e dados do cartão permanecem apenas na memória.

**Requisitos:** Windows 10/11 de 64 bits, ligação à Internet, drivers gráficos com OpenGL 3.3 e leitor PC/SC com os respetivos drivers. macOS e Linux ainda não têm aplicação distribuída. Não é necessário o antigo Navegante Link.

**Versão de teste:** o executável ainda não tem assinatura digital de editor. Não se deve desativar as proteções do Windows para o executar. Uma distribuição final ao público deverá incluir assinatura digital.

A preparação para assinar os downloads com identidade verificada está em [docs/signing.md](docs/signing.md). O processo usa SHA-256, timestamp e verificação obrigatória antes de produzir um artefacto assinado. A emissão do certificado continua pendente.
## Compilar a aplicação Rust

Instalar Rust estável e as ferramentas de compilação Windows (MSVC Build Tools ou MinGW para a toolchain GNU):

```powershell
cargo test --locked --manifest-path native/Cargo.toml
cargo clippy --locked --manifest-path native/Cargo.toml --all-targets -- -D warnings
./scripts/build-desktop.ps1
```

Resultado: `release/Navegante-Web.exe`. `native/Cargo.lock` fixa as dependências. O script não inclui ficheiros web nem Python. `--demo` permite testar a interface nativa com dados fictícios; `--self-test` verifica apenas o runtime e enumera os leitores. `tests/native-ui.ps1` testa o percurso completo por acessibilidade Windows, sem operações reais. Definir `NAVEGANTE_TEST_WAIT_DEVICES=1` permite testar também a espera por dispositivos simulados.

## Implementação web anterior (desenvolvimento)


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

`npm run build` prepara apenas a interface estática em `dist/`, configurada por `vercel.json` para a Vercel. A homepage apresenta o download; a demo em `/demo/` inicia automaticamente a simulação e bloqueia operações reais. A demo web ilustra o carregamento; a aplicação Rust tem uma interface própria e um fluxo simplificado. Ver [arquitetura, limites do Hobby e testes](docs/deployment.md).

## O que foi efetivamente validado

| Funcionalidade | Evidência |
| --- | --- |
| Deteção e comunicação PC/SC | Testada com Conceptronic CSMARTID e cartão físico |
| Seleção da aplicação `1TIC.ICA` | Resposta `9000`, FCI de 36 bytes, protocolo T=0 |
| Leitura autenticada | Serviço oficial devolveu cartão, titular, emissão, perfis, contratos e eventos |
| Catálogo e preços | Consultados no serviço oficial para o cartão inserido |
| Configuração do produto | `PUT products/...` aceite com HTTP lógico 200; revisão de preço testada no navegador |
| Interface | Testada em Edge, em larguras de computador e telemóvel, sem erros JavaScript |
| Desktop Rust | Leitura real no serviço oficial; testes de protocolo/compra e fluxo nativo por acessibilidade Windows, sem browser ou servidor local |
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
