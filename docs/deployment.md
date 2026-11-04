# Publicação da demo e aplicação Windows

## Experiência pública

- A homepage https://navegante-web-xi.vercel.app apresenta a aplicação desktop e dá prioridade ao download. Não carrega o runtime da demo.
- A demo opcional em https://navegante-web-xi.vercel.app/demo/ inicia automaticamente a simulação com dados fictícios.
- O site não procura leitores, não inicia pagamentos e não grava cartões reais.
- A página explica como descarregar a aplicação Windows e usar o mesmo percurso com um cartão real.
- A aplicação 2.0 é escrita em Rust, com interface nativa egui/eframe e acessibilidade Windows. Não usa Electron, WebView, browser, Python ou servidor HTTP local.

## Arquitetura

```text
Vercel → interface estática → simulação no navegador

Navegante-Web.exe (Rust, Windows x64)
  ├── interface nativa → decisões do utilizador
  └── tarefa de operações, serializadas
       ├── winscard.dll / PC/SC → leitor USB → cartão
       └── HTTPS → serviços oficiais de bilhética/pagamento
```

O build público cria a homepage de download em `dist/index.html` e a demo em `dist/demo/index.html`, onde ativa `data-hosted-demo="true"`. A função de pedidos bloqueia operações reais nesse modo e a CSP usa `connect-src 'none'`. Não existe opção de sair para um modo real na Vercel. Não são publicados backends Python ou segredos na cloud.

O percurso começa em «Carregar passe». A aplicação deteta o leitor, aguarda o cartão e avança automaticamente para os dados. Não consulta produtos até o utilizador escolher carregar um passe ou adicionar saldo. Com vários leitores, solicita uma escolha. Preço, confirmação MB WAY e gravação são passos distintos.

Requer Windows 10/11 x64, OpenGL 3.3, Internet e drivers do leitor. Não usa base de dados nem guarda dados do cartão em ficheiros. A referência pode ser copiada pelo utilizador para recuperar um pedido depois de reabrir. Bloqueia pagamentos duplicados e períodos já cobertos; só aceita gravações provenientes de autorização oficial assinada e impede repetir uma gravação de resultado incerto. Fechar durante uma operação é impedido; um pedido pendente exige guardar a referência antes de sair.

O código da ponte emparelhada permanece para desenvolvimento avançado e regressão, mas não faz parte do percurso público atual. O antigo Navegante Link não é necessário.

## Compilar

Interface pública:

```powershell
npm ci
npm run build
```

Aplicação Windows, com Rust estável e MSVC Build Tools ou MinGW para a toolchain GNU:

```powershell
cargo test --locked --manifest-path native/Cargo.toml
cargo clippy --locked --manifest-path native/Cargo.toml --all-targets -- -D warnings
./scripts/build-desktop.ps1
```

Resultado: `release/Navegante-Web.exe`, portátil. `native/Cargo.lock` fixa as dependências. `--demo` usa dados fictícios; `--self-test` apenas enumera leitores. `--read-check "nome exato do leitor"` executa uma leitura real explícita, sem compra ou gravação. O código Python/web anterior permanece como referência e para regressão; não integra o executável Rust.

**Assinatura:** o executável ainda não tem assinatura digital de editor; é distribuído como versão de teste. A assinatura continua pendente para distribuição final ao público. Não se contornam proteções do Windows.

## Verificação

```powershell
python -m unittest discover -s tests
cargo test --locked --manifest-path native/Cargo.toml
powershell -NoProfile -File tests/native-ui.ps1 -Executable release/Navegante-Web.exe
$env:NAVEGANTE_TEST_WAIT_DEVICES='1'
powershell -NoProfile -File tests/native-ui.ps1 -Executable release/Navegante-Web.exe
npm run build
node tests/homepage.cjs
node tests/hosted-demo.cjs
node tests/demo.cjs
node tests/browser.cjs
```

O teste da demo web verifica isolamento e zero chamadas de rede. O teste nativo usa Windows UI Automation para verificar espera e avanço automático, dados, decisão, saldo, compra simulada, proteção ao fechar e ausência de processos de browser/servidor. Os testes financeiros Rust usam um servidor de teste local e não contactam os serviços oficiais nem iniciam pagamentos ou gravações físicas.

## Vercel Hobby

Verificado em 4 de novembro de 2026: o [Hobby custa 0 USD/mês](https://vercel.com/pricing), é [destinado a uso pessoal e não comercial](https://vercel.com/docs/plans/hobby) e está sujeito a limites de utilização. A publicação é estática, sem base de dados ou armazenamento pago. O custo de títulos de transporte é independente do alojamento.
