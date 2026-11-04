# Publicação da demo e aplicação Windows

## Experiência pública

- O site https://navegante-web-xi.vercel.app inicia automaticamente uma demo com dados fictícios.
- O site não procura leitores, não inicia pagamentos e não grava cartões reais.
- A página explica como descarregar a aplicação Windows e usar o mesmo percurso com um cartão real.
- A aplicação é o mesmo HTML, CSS e JavaScript, apresentado numa janela própria do Microsoft Edge. Não é um conector que obrigue a voltar ao site.

## Arquitetura

```text
Vercel → interface estática → simulação no navegador

Navegante-Web.exe (Windows)
  ├── mesma interface numa janela própria
  └── servidor local numa porta livre de 127.0.0.1
       ├── Windows PC/SC → leitor USB → cartão
       └── serviços oficiais de bilhética/pagamento
```

O build público ativa `data-hosted-demo="true"`. A função de pedidos bloqueia operações reais nesse modo e a CSP usa `connect-src 'none'`. Não existe opção de sair para um modo real na Vercel. Não são publicados backends Python ou segredos na cloud.

O executável contém Python e os quatro ficheiros da interface. Requer Windows 64 bits, Microsoft Edge e drivers do leitor. Abre o Edge em modo de aplicação, com um perfil temporário próprio, sem usar o perfil pessoal do utilizador. O servidor local usa uma porta livre e não aceita origens remotas. Ao fechar a janela, espera por uma operação de cartão em curso antes de terminar o serviço. Não usa base de dados própria nem guarda sessões de cartões em ficheiros.

O código da ponte emparelhada permanece para desenvolvimento avançado e regressão, mas não faz parte do percurso público atual. O antigo Navegante Link não é necessário.

## Compilar

Interface pública:

```powershell
npm ci
npm run build
```

Aplicação Windows, a partir de Windows com Python 3.13:

```powershell
python -m pip install -r requirements-build.txt
./scripts/build-desktop.ps1
```

Resultado: `release/Navegante-Web.exe`. A versão de PyInstaller está fixada em `requirements-build.txt`. A aplicação é portátil e não pede Python ao utilizador. O teste `--self-test` verifica o runtime gráfico, a página incluída e a enumeração de leitores, sem ler ou alterar cartões.

**Assinatura:** o executável ainda não tem assinatura digital de editor; é distribuído como versão de teste. A assinatura continua pendente para distribuição final ao público. Não se contornam proteções do Windows.

## Verificação

```powershell
python -m unittest discover -s tests
npm run build
node tests/hosted-demo.cjs
node tests/demo.cjs
node tests/browser.cjs
```

O teste da demo publicada verifica arranque automático, bloqueio do modo real e zero chamadas de rede, incluindo no percurso de pagamento e gravação simulados. O teste desktop verifica que a aplicação apresenta exatamente o HTML local, numa janela própria e com porta efémera. Os testes financeiros usam serviços simulados; não iniciam pagamentos ou gravações físicas.

## Vercel Hobby

Verificado em 4 de novembro de 2026: o [Hobby custa 0 USD/mês](https://vercel.com/pricing), é [destinado a uso pessoal e não comercial](https://vercel.com/docs/plans/hobby) e está sujeito a limites de utilização. A publicação é estática, sem base de dados ou armazenamento pago. O custo de títulos de transporte é independente do alojamento.
