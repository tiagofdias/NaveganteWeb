# Publicação e acesso ao leitor

## Arquitetura

O mesmo site serve todos os utilizadores. A Vercel distribui apenas quatro ficheiros estáticos: HTML, JavaScript e duas folhas de estilos. Não recebe comandos USB, códigos de ligação, dados de cartões ou pedidos de pagamento. O JavaScript efetua as operações reais diretamente contra a ponte no PC de quem está a usar o site.

```text
Navegador (site HTTPS na Vercel)
        │ aprovação na aplicação + origem HTTPS autorizada
        ▼
127.0.0.1:8765 (server.py no PC do utilizador)
        ├── Windows PC/SC → leitor USB → cartão
        └── serviços oficiais de bilhética/pagamento
```

Cada aplicação aceita uma única origem HTTPS exata e gera uma credencial aleatória por arranque. A rota /api/pair só devolve essa credencial depois de aprovação explícita numa janela nativa; não são pedidos códigos ao utilizador. CORS, verificação de Host e o cabeçalho `X-Navegante-Token` são exigidos para acesso remoto. A credencial só fica na memória do processo e da página; não existe base de dados, cookie ou armazenamento no navegador. Uma visita ao site não faz pedidos à ponte antes do clique de ligação. Operações reais no serviço são serializadas para impedir submissões simultâneas.

O utilizador tem de autorizar o acesso local/do dispositivo no navegador. A implementação usa `targetAddressSpace: 'loopback'` e pedidos CORS normais, sem desativar proteções do navegador. Referência: [Chrome — Local Network Access](https://developer.chrome.com/docs/modern-web-guidance/guides/security/local-network-access).

## Vercel Hobby

Verificado em 4 de novembro de 2026: o [Hobby custa 0 USD/mês](https://vercel.com/pricing), é [destinado a uso pessoal e não comercial](https://vercel.com/docs/plans/hobby) e tem limites de utilização. A documentação indica que normalmente, ao ultrapassar limites, é necessário aguardar a reposição, em vez de comprar capacidade adicional no Hobby. Não foi selecionado Pro, base de dados, armazenamento pago ou domínio comprado. O custo da compra de títulos ao operador não é um custo de alojamento.

## Build

```powershell
npm ci
npm run build
```

`scripts/build.mjs` cria `dist/` e ativa a ligação remota apenas na cópia publicada do HTML. A interface servida por `python server.py` continua a funcionar diretamente em localhost. `vercel.json` define o build estático e a política CSP; nenhum ficheiro Python é publicado como função serverless.

## Testes

```powershell
python -m unittest discover -s tests
node tests/demo.cjs
node tests/browser.cjs
npm run build
node tests/hosted-bridge.cjs
```

Os testes do navegador usam Edge no Windows quando disponível. Também é possível definir `PLAYWRIGHT_BROWSER_EXECUTABLE` ou instalar o Chromium do Playwright com `npx playwright install chromium`. O teste `tests/browser.cjs` só faz uma leitura física se `NAVEGANTE_LIVE_READ=1` for definido explicitamente. Não inicia pagamentos nem gravações físicas.

Os APKs, código descompilado, ferramentas de análise, arte abandonada, credenciais e ficheiros temporários não fazem parte do repositório ou do build público.

## Aplicação Windows

O executável portátil é criado em Windows com Python 3.13 e a versão fixada em requirements-build.txt. Para reproduzir: instalar as dependências de build e executar scripts/build-desktop.ps1. O ficheiro release/Navegante-Link.exe inclui Python e Tcl/Tk; não abre consola. O arranque normal nunca aceita automaticamente o emparelhamento. O parâmetro de teste --self-test verifica o runtime e o transporte numa porta efémera com origem fictícia, sem leituras ou gravações de cartões.

A versão publicada ainda não está assinada digitalmente. A assinatura de editor continua pendente para distribuição final ao público; não se contornam proteções do sistema operativo.
