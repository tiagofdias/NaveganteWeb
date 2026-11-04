const $ = id => document.getElementById(id);
const state = {busy:false, session:null, card:null, details:null, quote:null, purchase:null, catalogReady:false, showDetails:false, showReader:false, catalog:null, metropolitanActive:false, showOtherPasses:false};
const euros = cents => new Intl.NumberFormat('pt-PT',{style:'currency',currency:'EUR'}).format(cents/100);
const remoteMode=document.body.dataset.remote==='true';
const hostedDemo=document.body.dataset.hostedDemo==='true';
const bridgeClient={token:'',ready:false};
const bridgeAddress='http://127.0.0.1:8765';
// Demo operations are handled entirely in memory, before any network request.
const demo = {enabled:false, card:null, scenario:null, quote:null, sale:null};
function demoCalendar(now=new Date()) {
  const parts=new Intl.DateTimeFormat('en-CA',{year:'numeric',month:'numeric',day:'numeric',timeZone:'Europe/Lisbon'}).formatToParts(now);
  return {year:Number(parts.find(p=>p.type==='year').value),month:Number(parts.find(p=>p.type==='month').value),day:Number(parts.find(p=>p.type==='day').value)};
}
function demoTitle(product,year,month) {
  const name=product==='id-prod-navegante-metro'?'navegante Metropolitano':'navegante Municipal Lisboa';
  return {product_detail:{product:{id:product,name}},contract:[{id:'contract.name',value:name},
    {id:'contract.temporal_validity.month',value_id:String(month)},{id:'contract.temporal_validity.year',value_id:String(year)},
    {id:'contract.temporal_validity.date',value_id:`${year}-${String(month).padStart(2,'0')}-${new Date(Date.UTC(year,month,0)).getUTCDate()}`},
    {id:'contract.temporal_validity.status',value_id:'1'}]};
}
function makeDemoCard(scenario) {
  const {year,month}=demoCalendar();
  const card={media:{cardNumber:'DEMO 0000 0000'},holder:[{id:'holder.name',value:'Alex · Cartão demo'}],
    issuing:[{id:'issuing.issuing_date',value:`${year}-01-01`},{id:'issuing.expiration_date',value:`${year+5}-01-01`}],
    profiles:[{profile:[{id:'profile.name',value:'Normal'}]}],events:[],
    contracts:[{product_detail:{product:{id:'id-prod-zapping',name:'Zapping'}},contract:[{id:'contract.amount',value_id:'1235'}]}]};
  if(scenario==='valid')card.contracts.unshift(demoTitle('id-prod-navegante-metro',year,month));
  if(scenario==='expired'){
    const previous=new Date(Date.UTC(year,month-2,15));
    const title=demoTitle('id-prod-navegante-metro',previous.getUTCFullYear(),previous.getUTCMonth()+1);
    title.contract.find(f=>f.id==='contract.temporal_validity.status').value_id='3';card.contracts.unshift(title);
  }
  return card;
}
function demoProduct(product,now=new Date()) {
  if(product==='id-prod-zapping')return {id:product,name:'Zapping',attributes:[{id:'product_occasional',values:[{id:'demo-balance-1000',value:'1000'},{id:'demo-balance-2000',value:'2000'}]}],
    prices:[1000,2000].map(amount=>({id:'demo-balance-'+amount,net:amount,tax:0}))};
  if(!['id-prod-navegante-metro','id-prod-navegante-lisboa'].includes(product))throw Error('Produto de demonstração desconhecido.');
  const {year,month,day}=demoCalendar(now);
  // Demo follows the usual monthly sale window; live availability stays server-driven.
  const dates=[new Date(Date.UTC(year,day>=26?month:month-1,15))];
  const values=dates.map(d=>({id:`demo-month-${d.getUTCFullYear()}-${d.getUTCMonth()+1}`,value:String(d.getUTCMonth()+1),year:d.getUTCFullYear()}));
  return {id:product,name:product==='id-prod-navegante-metro'?'navegante Metropolitano':'navegante Municipal Lisboa',
    attributes:[{id:'product_monthly',values}],prices:values.map(v=>{
      const duplicate=(demo.card?.contracts||[]).some(item=>{
        const get=id=>(item.contract||[]).find(f=>f.id==='contract.temporal_validity.'+id)?.value_id;
        const existing=item.product_detail?.product?.id;
        return Number(get('year'))===v.year&&get('month')===v.value&&(existing===product||existing==='id-prod-navegante-metro');
      });
      return {id:v.id,net:product==='id-prod-navegante-metro'?4000:3000,tax:0,blockedReason:duplicate?'Este período já está carregado no cartão demo.':null};
    })};
}
async function demoRequest(path,body={}) {
  await new Promise(resolve=>setTimeout(resolve,path==='read'?450:180));
  if(path==='readers')return {readers:['Leitor virtual · Demo']};
  if(path==='read'){
    const scenario=$('demo-scenario').value;
    if(!demo.card||demo.scenario!==scenario){demo.card=makeDemoCard(scenario);demo.scenario=scenario;demo.quote=null;demo.sale=null;}
    if(demo.sale?.state==='loaded')demo.sale=null;
    return {session:'demo-memory',card:structuredClone(demo.card)};
  }
  if(path==='catalog')return {products:[{id:'id-prod-navegante-metro',name:'navegante Metropolitano'},{id:'id-prod-navegante-lisboa',name:'navegante Municipal Lisboa'},{id:'id-prod-zapping',name:'Zapping'}]};
  if(path==='product')return demoProduct(body.product);
  if(path==='prepare'){
    const details=demoProduct(body.product),price=details.prices.find(p=>p.id===body.variant);
    if(!price||price.blockedReason)throw Error(price?.blockedReason||'Seleciona uma opção de demonstração.');
    demo.quote={product:body.product,variant:body.variant,name:details.name,amount:price.net+price.tax,currency:'EUR'};
    return {...demo.quote};
  }
  if(path==='checkout'){
    if(!demo.quote||demo.sale)throw Error('Reinicia a demo para criar outro pedido.');
    if(body.confirmedAmount!==demo.quote.amount)throw Error('Confirma o valor simulado.');
    demo.sale={saleId:'DEMO-0001',state:'pending',canLoad:false};return {...demo.sale};
  }
  if(path==='payment'){
    if(!demo.sale)throw Error('Ainda não existe um pagamento fictício.');
    if(demo.sale.state==='pending')demo.sale={...demo.sale,state:'paid',canLoad:true};
    return {...demo.sale};
  }
  if(path==='load'){
    if(!demo.sale?.canLoad||!body.confirmWrite||!demo.card)throw Error('Confirma primeiro o pagamento e a gravação simulados.');
    if(demo.quote.product==='id-prod-zapping'){
      const balance=demo.card.contracts.find(isStoredValue).contract.find(f=>f.id==='contract.amount');
      balance.value_id=String(Number(balance.value_id)+demo.quote.amount);
    }else{
      const [,year,month]=/^demo-month-(\d+)-(\d+)$/.exec(demo.quote.variant);
      demo.card.contracts.push(demoTitle(demo.quote.product,Number(year),Number(month)));
    }
    demo.sale={...demo.sale,state:'loaded',canLoad:false};demo.quote=null;return {...demo.sale};
  }
  if(path==='recover'){
    if(!demo.sale||body.saleId!==demo.sale.saleId)throw Error('Na demo, cria um pedido fictício e usa a referência DEMO-0001.');
    return {session:'demo-memory',saleId:demo.sale.saleId};
  }
  throw Error('Operação indisponível na demonstração.');
}
const labels = {
  'holder.name':'Nome', 'issuing.issuing_date':'Data de emissão', 'issuing.expiration_date':'Validade do cartão',
  'issuing.validity.status':'Estado do cartão', 'profile.id':'Perfil', 'profile.end_validity':'Validade do perfil',
  'profile.validity.status':'Estado do perfil', 'profile.name':'Perfil', 'contract.name':'Título',
  'contract.temporal_validity.date':'Período de validade', 'contract.temporal_validity.status':'Estado do título',
  'event.type_validation':'Tipo de utilização', 'event.operator':'Operador', 'event.product_name':'Título',
  'event.line':'Linha', 'event.stop':'Paragem', 'event.usage':'Utilização'
};
function node(tag, text, className) { const n=document.createElement(tag); if(text!=null)n.textContent=text; if(className)n.className=className; return n; }
function fields(dl, pairs) { dl.replaceChildren(); for(const [key,value] of pairs) {dl.append(node('dt',key),node('dd',value??'Não disponível'));} }
function fieldValue(field) { return field.value ?? field.value_id ?? field.name ?? 'Não disponível'; }
function holderName(holder) {
  // Some cards return an unused second name slot as a literal "0".
  // Ignore complete placeholder fields, never strip digits from a real name.
  return [...new Set((holder||[]).filter(f=>f.id==='holder.name')
    .map(f=>typeof f.value==='string'?f.value.replace(/\0/g,'').trim():'')
    .filter(value=>value&&!/^0+$/.test(value)))].join(' ');
}
function displayCardField(field) {
  const value=fieldValue(field);
  if(['issuing.issuing_date','issuing.expiration_date','profile.start_validity','profile.end_validity','contract.temporal_validity.date'].includes(field.id)
      && /^\d{4}-\d{2}-\d{2}$/.test(String(value))) {
    const [year,month,day]=value.split('-');
    return `${day}/${month}/${year}`;
  }
  return value;
}
function usefulCardFields(items, kind) {
  const allowed={contract:['contract.name','contract.temporal_validity.date'],
    profile:['profile.name','profile.end_validity'],
    events:['event.type_validation','event.operator','event.product_name','event.line','event.stop','event.usage']};
  return (items||[]).filter(f=>allowed[kind].includes(f.id)).flatMap(f=>{
    const value=String(f.value??f.value_id??'').replace(/\0/g,'').trim();
    if(!value||/^0+$/.test(value)||/^id[-_]/i.test(value))return [];
    // Numeric service codes have no passenger-facing meaning without a legend.
    if(['event.type_validation','event.operator','event.usage'].includes(f.id)&&/^\d+$/.test(value))return [];
    return [[labels[f.id]||{'contract.amount':'Saldo'}[f.id],displayCardField(f)]];
  });
}
async function request(path, body) {
  if(demo.enabled)return demoRequest(path,body);
  if(hostedDemo)throw Error('Esta página é apenas uma demonstração. Descarrega o projeto para utilizar um cartão real no teu PC.');
  if(remoteMode&&!bridgeClient.token&&path!=='pair')throw Error('Abre o Navegante Link no teu PC e escolhe «Ligar leitor».');
  const controller=new AbortController(), timeout=setTimeout(()=>controller.abort(),150000);
  try {
    const headers=remoteMode?{'X-Navegante-Token':bridgeClient.token}:{};
    const options={signal:controller.signal,headers,credentials:'omit',cache:'no-store'};
    if(remoteMode)options.targetAddressSpace='loopback';
    if(body!==undefined){options.method='POST';headers['Content-Type']='application/json';options.body=JSON.stringify(body);}
    const response=await fetch((remoteMode?bridgeAddress:'')+'/api/'+path,options);
    const data=await response.json();
    if(remoteMode&&response.status===403){bridgeClient.ready=false;bridgeClient.token='';throw Error(path==='pair'?(data.error||'Autoriza a ligação na aplicação Navegante Link.'):'A ligação terminou. Abre o Navegante Link e liga novamente o leitor.');}
    if(!response.ok)throw new Error(data.error||'Não foi possível concluir o pedido.');
    return data;
  } catch(error) {
    if(remoteMode&&error instanceof TypeError){bridgeClient.ready=false;bridgeClient.token='';throw Error('Não foi possível ligar ao leitor. Abre o Navegante Link e permite a ligação ao teu PC quando o navegador pedir.');}
    if(error.name==='AbortError')throw new Error('O pedido demorou demasiado. Se pediste um pagamento, consulta a referência do pedido antes de repetir.');
    throw error;
  } finally {clearTimeout(timeout);}
}
function stage() {return state.purchase?(state.purchase.state==='loaded'?'complete':'payment'):state.quote?'checkout':state.card?'choose':'read';}
function presentation() {
  const view=stage(), hasCard=Boolean(state.card);
  document.body.dataset.demo=String(demo.enabled);
  $('demo-banner').hidden=!demo.enabled;
  $('demo-toggle').textContent=demo.enabled?'Sair da demo':'Experimentar demo';
  $('demo-toggle').setAttribute('aria-pressed',String(demo.enabled));
  $('demo-toggle').hidden=hostedDemo;
  $('live-instructions').hidden=!hostedDemo;
  $('demo-scenario-field').hidden=!demo.enabled||hasCard||Boolean(state.purchase);
  $('bridge-panel').hidden=!remoteMode||demo.enabled;
  $('bridge-form').hidden=bridgeClient.ready;
  $('bridge-guide').hidden=bridgeClient.ready;
  $('bridge-disconnect').hidden=!bridgeClient.ready;
  document.querySelector('.local-badge').replaceChildren(node('span',null,'status-dot'),document.createTextNode(demo.enabled?'Modo demonstração':remoteMode?(bridgeClient.ready?'Ligado ao teu PC':'Leitor por ligar'):'Ligação local'));
  document.body.dataset.stage=view;
  $('main').setAttribute('aria-busy',String(state.busy));
  $('activity').hidden=!state.busy;
  $('activity').lastChild.textContent=view==='read'?'A ler o passe…':view==='choose'&&!state.catalogReady?'A consultar os carregamentos disponíveis…':'A processar o pedido…';
  $('card-context').hidden=!hasCard&&!state.purchase;
  const preview=document.querySelector('.pass-wrap');
  if(hasCard&&preview.parentElement!==$('selected-card'))$('selected-card').append(preview);
  if(!hasCard&&preview.parentElement=== $('selected-card'))document.querySelector('.card-panel').insertBefore(preview,document.querySelector('.card-panel-bottom'));
  $('selected-card').hidden=!hasCard;
  $('recovered-identity').hidden=hasCard||!state.purchase;
  $('pass-status').hidden=!hasCard;
  if(!hasCard&&state.purchase){$('context-name').textContent='Pedido recuperado';$('context-number').textContent=state.purchase.saleId||'';$('context-contract').textContent='Seleciona o leitor para gravar o carregamento.';}
  $('reader-workspace').hidden=hasCard||state.purchase?!state.showReader:false;
  $('result').hidden=!hasCard;
  $('catalog-panel').hidden=view!=='choose';
  $('checkout-panel').hidden=view!=='checkout';
  $('payment-panel').hidden=!state.purchase;
  $('catalog-retry').hidden=state.busy||!hasCard||state.catalogReady||view!=='choose';
  $('toggle-reader').textContent='Voltar ao início';
  const copy={read:['Lê. Carrega. Segue.','Liga o leitor e insere o teu passe para começar.'],choose:['O que vamos carregar?','O teu passe já foi lido. Escolhe um título ou adiciona saldo.'],checkout:['Tudo pronto para continuar.','Confere o carregamento e indica os dados para o pagamento.'],payment:['O teu pedido, passo a passo.','Acompanha o pagamento e grava o carregamento no passe.'],complete:['Boa viagem.','O carregamento foi confirmado. O teu passe está pronto.']};
  if(demo.enabled){copy.read[1]='Escolhe um cartão fictício e experimenta o carregamento sem leitor.';copy.choose[1]='Este cartão é fictício. Escolhe um passe ou adiciona saldo para experimentar.';copy.checkout[1]='Confere o exemplo. Os dados e o pagamento são apenas uma simulação.';copy.payment[1]='Simula a confirmação do pagamento e o carregamento.';copy.complete=['Demo concluída.','Experimentaste o fluxo completo, sem cobranças ou alterações reais.'];}
  $('read').firstChild.textContent=demo.enabled?'Simular leitura do passe ':'Ler o meu passe ';
  $('poll').textContent=demo.enabled?'Simular confirmação do pagamento':'Atualizar estado do pedido';
  $('load').textContent=demo.enabled?'Simular carregamento':'Gravar carregamento no passe';
  $('confirm-write').nextElementSibling.textContent=demo.enabled?'Quero simular a gravação neste cartão fictício.':'O passe usado nesta compra está no leitor. Quero gravar o carregamento.';
  $('success').querySelector('h3').textContent=demo.enabled?'Demonstração concluída.':'Pronto para a próxima viagem.';
  $('success').querySelector('p').textContent=demo.enabled?'Nenhuma cobrança foi efetuada e nenhum cartão real foi alterado. Consulta novamente o cartão demo para ver o resultado.':'O serviço Navegante confirmou o carregamento. Podes retirar o passe do leitor.';
  $('screen-title').textContent=copy[view][0];
  $('screen-description').textContent=copy[view][1];
  $('screen-title').setAttribute('tabindex','-1');
}
function focusStage() {
  const target={read:'reader-workspace',choose:'catalog-panel',checkout:'checkout-panel',payment:'payment-panel',complete:'payment-panel'}[stage()];
  const heading=$(target)?.querySelector('h2');
  if(heading){heading.setAttribute('tabindex','-1');heading.focus({preventScroll:true});}
  $('main').scrollIntoView({behavior:matchMedia('(prefers-reduced-motion: reduce)').matches?'instant':'smooth',block:'start'});
}
function controls() {
  presentation();
  const currentStep=state.purchase?(state.purchase.canLoad||state.purchase.state==='paid'||state.purchase.state==='loaded'?3:2):state.quote?2:state.card?1:0;
  document.querySelectorAll('.journey li').forEach((step,index)=>{
    const complete=index<currentStep||state.purchase?.state==='loaded';
    step.classList.toggle('done',complete);
    step.classList.toggle('current',index===currentStep&&!complete);
    if(index===currentStep&&!complete)step.setAttribute('aria-current','step');
    else step.removeAttribute('aria-current');
  });
  const locked=state.busy||Boolean(state.purchase);
  $('read').disabled=locked||!$('reader').value||(remoteMode&&!demo.enabled&&!bridgeClient.ready);
  $('bridge-connect').disabled=state.busy;
  $('bridge-disconnect').disabled=state.busy||Boolean(state.purchase&&state.purchase.state!=='loaded');
  $('reader').disabled=state.busy||demo.enabled; $('refresh').disabled=state.busy;
  $('demo-toggle').disabled=state.busy||Boolean(!demo.enabled&&state.purchase&&state.purchase.state!=='loaded');
  $('demo-reset').disabled=state.busy;$('demo-scenario').disabled=state.busy;
  for(const id of ['product','variant','prepare']) $(id).disabled=locked;
  $('product').disabled=locked||!state.catalogReady;
  $('prepare').disabled=locked||!selectedPrice();
  for(const id of ['recover','poll','new-read']) $(id).disabled=state.busy;
  for(const id of ['catalog-retry','back-products','toggle-reader']) $(id).disabled=state.busy;
  $('toggle-reader').disabled=state.busy||Boolean(state.purchase&&state.purchase.state!=='loaded');
  $('recover').disabled=state.busy||Boolean(state.purchase&&state.purchase.state!=='loaded');
  $('checkout').disabled=state.busy||!state.quote||Boolean(state.purchase);
  $('load').disabled=state.busy||!$('confirm-write').checked||!state.purchase?.canLoad;
}
async function action(target, task) {
  if(state.busy)return;
  const before=stage();
  $(target).removeAttribute('data-error');
  state.busy=true; controls();
  try { await task(); } catch(error) {
    const visibleTarget=state.purchase&&['checkout-status','recover-status'].includes(target)?'payment-status':target==='status'&&state.card&&!state.showReader?'catalog-status':target;
    $(visibleTarget).textContent=error.message; $(visibleTarget).setAttribute('data-error','true');
    if(!state.card&&!state.purchase)state.showReader=true;
  }
  finally {const changed=before!==stage()&&document.body.dataset.stage!==stage();state.busy=false;controls();if(changed)focusStage();}
}
function clearReading() {
  state.session=null;state.card=null;state.details=null;state.quote=null;
  state.catalogReady=false;state.showDetails=false;state.showReader=false;
  state.catalog=null;state.metropolitanActive=false;state.showOtherPasses=false;
  for(const id of ['result','catalog-panel','checkout-panel','product-options'])$(id).hidden=true;
  $('pass-name').textContent='Um cartão. Infinitas possibilidades.';
  $('pass-number').textContent='O TEU CARTÃO FÍSICO';
  $('pass-balances').hidden=true;
}
function resetInterface() {
  state.purchase=null;clearReading();
  $('checkout-form').hidden=false;$('checkout-form').reset();$('confirm-write').checked=false;
  $('recover-form').reset();
  for(const id of ['status','catalog-status','checkout-status','payment-status','recover-status']){$(id).textContent='';$(id).removeAttribute('data-error');}
  if(demo.enabled){$('receipt-email').value='demo@example.invalid';$('phone').value='912345678';}
  controls();
}
async function changeDemo(enabled) {
  if(hostedDemo)enabled=true;
  if(state.busy||(!demo.enabled&&state.purchase&&state.purchase.state!=='loaded'))return;
  demo.enabled=enabled;demo.card=null;demo.quote=null;demo.sale=null;demo.scenario=null;
  resetInterface();
  if(!remoteMode||demo.enabled||bridgeClient.ready)await action('status',refresh);
  else {$('reader').replaceChildren(new Option('Liga primeiro o leitor do teu PC',''));$('status').textContent='Abre o Navegante Link e liga o leitor ou experimenta a demo.';controls();}
  focusStage();
}
async function refresh() {
  const previous=$('reader').value;
  const {readers}=await request('readers');
  $('reader').replaceChildren(new Option('Seleciona o leitor…',''));
  readers.forEach(name=>$('reader').add(new Option(name,name)));
  $('reader').value=readers.includes(previous)?previous:(readers.find(name=>/^THRC /i.test(name))||(readers.length===1?readers[0]:''));
  $('status').textContent=demo.enabled?'Leitor virtual pronto. Não precisas de ligar nenhum equipamento.':readers.length?'Insere o passe e inicia a leitura.':'Nenhum leitor encontrado. Liga o teu leitor e atualiza a lista.';
  if(demo.enabled)$('reader').value='Leitor virtual · Demo';
}
function renderCard(card) {
  renderPassStatus(card);
  $('result-title').textContent='O teu passe.';
  const name=holderName(card.holder);
  $('pass-name').textContent=name||'Cartão Navegante';
  $('pass-number').textContent=card.media?.cardNumber||'Cartão lido';
  $('context-name').textContent=name||'Cartão Navegante';
  $('context-number').textContent=card.media?.cardNumber?'N.º '+card.media.cardNumber:'Cartão lido';
  const contracts=(card.contracts||[]).map(item=>item.product_detail?.product?.name||(item.contract||[]).find(f=>f.id==='contract.name')?.value).filter(Boolean);
  $('context-contract').textContent=contracts.length?contracts.join(' · '):'Sem títulos indicados na leitura';
  const issuing=card.issuing||[];
  const summary=issuing.filter(f=>['issuing.issuing_date','issuing.expiration_date'].includes(f.id)&&String(f.value??f.value_id??'').trim()).map(f=>[labels[f.id],displayCardField(f)]);
  const cardExpiry=issuing.find(f=>f.id==='issuing.expiration_date');
  for(const item of card.profiles||[]){
    for(const [label,value] of usefulCardFields(item.profile,'profile')){
      if(label==='Validade do perfil'&&cardExpiry&&value===displayCardField(cardExpiry))continue;
      if(!summary.some(([key,existing])=>key===label&&existing===value))summary.push([label,value]);
    }
  }
  $('details').replaceChildren();
  for(const [label,value] of summary){const group=node('div');group.append(node('dt',label),node('dd',value));$('details').append(group);}
  const sections=$('card-sections');sections.replaceChildren();
  const visibleSections=[];
  for(const [key,title,child] of [['events','Últimas utilizações','events']]) {
    const section=node('section'); section.append(node('h3',title));
    const items=card[key]||[];
    let count=0;
    for(const item of items) {
      const pairs=usefulCardFields(item[child],child);
      if(child==='contract'&&!pairs.some(([label])=>label==='Título')&&item.product_detail?.product?.name)pairs.unshift(['Título',item.product_detail.product.name]);
      if(!pairs.length)continue;
      const dl=node('dl');fields(dl,pairs);section.append(dl);count++;
    }
    if(count){sections.append(section);visibleSections.push(title.toLowerCase());}
  }
  const disclosure=sections.closest('details');
  disclosure.hidden=!visibleSections.length;
  disclosure.querySelector('summary').replaceChildren(document.createTextNode('Consultar '+visibleSections.join(', ')),node('span','+'));
  $('technical').closest('details').hidden=true;
  $('technical').textContent='Leitura autenticada concluída pelo serviço oficial. Nenhuma alteração de título ou saldo solicitada.';
  const validity=(card.issuing||[]).find(f=>f.id==='issuing.validity.status');
  if(validity)$('technical').textContent+='\nEstado técnico de validade do suporte (issuing.validity.status): '+fieldValue(validity)+'. Legenda deste código ainda não confirmada; não indica se existe um passe mensal válido.';
  $('result').hidden=false;
}
function renderPassStatus(card, now=new Date()) {
  renderBalances(card);
  const current=new Intl.DateTimeFormat('en-CA',{year:'numeric',month:'2-digit',timeZone:'Europe/Lisbon'}).formatToParts(now);
  const currentMonth=Number(current.find(p=>p.type==='year').value)*12+Number(current.find(p=>p.type==='month').value);
  const titles=(card.contracts||[]).filter(item=>!isStoredValue(item)).map(item=>{
    const data=item.contract||[];
    const get=id=>data.find(f=>f.id==='contract.'+id);
    const raw=id=>{const f=get(id);return f?.value_id??f?.value;};
    const name=get('name')?fieldValue(get('name')):item.product_detail?.product?.name||'Título no cartão';
    const expiry=String(raw('temporal_validity.date')||'');
    const monthly=/metropolitano|municipal/i.test(name)||/^id-prod-navegante-metro$/.test(item.product_detail?.product?.id||'');
    const expiryMonth=monthly&&/^\d{4}-\d{2}-\d{2}$/.test(expiry)?expiry.slice(0,7).split('-'):null;
    const month=Number(raw('temporal_validity.month')||(expiryMonth?.[1]??''));
    const yearText=String(raw('temporal_validity.year')||(expiryMonth?.[0]??''));
    const year=/^\d{4}(?:-|$)/.test(yearText)?Number(yearText.slice(0,4)):NaN;
    const period=month>=1&&month<=12&&Number.isInteger(year)?year*12+month:null;
    // APK i8.k: VALID=1, EXPIRING=2, EXPIRED=3 (contract, not card status).
    const rawStatus=String(raw('temporal_validity.status')??'').toUpperCase();
    const status=({'1':'VALID','2':'EXPIRING','3':'EXPIRED'})[rawStatus]||rawStatus;
    let kind=['VALID','EXPIRING'].includes(status)?'active':status==='EXPIRED'?'expired':'unknown';
    if(period!==null)kind=period>currentMonth?'future':period<currentMonth?'expired':kind==='expired'?'expired':'active';
    const validity=/^\d{4}-\d{2}-\d{2}$/.test(expiry)?'Válido até '+expiry.split('-').reverse().join('/'):period!==null?new Intl.DateTimeFormat('pt-PT',{month:'long',year:'numeric',timeZone:'UTC'}).format(new Date(Date.UTC(year,month-1,15))):get('temporal_validity.date')?String(fieldValue(get('temporal_validity.date'))):'';
    return {name,kind,validity,metropolitan:/metropolitano/i.test(name)||item.product_detail?.product?.id==='id-prod-navegante-metro'};
  });
  state.metropolitanActive=titles.some(t=>t.metropolitan&&t.kind==='active');
  const active=titles.some(t=>t.kind==='active'),future=titles.some(t=>t.kind==='future'),unknown=titles.some(t=>t.kind==='unknown');
  const kind=active?'active':unknown?'unknown':future?'future':'empty';
  const copy={active:['Passe carregado e válido','Existe um título válido no cartão, de acordo com a última leitura.'],future:['Carregado para um próximo mês','O título está no cartão, mas o seu mês de validade ainda não começou.'],empty:[titles.length?'Sem título válido neste momento':'Sem carregamento no cartão',titles.length?'Os títulos devolvidos na leitura já expiraram.':'O serviço não devolveu títulos carregados neste cartão.'],unknown:['Título encontrado — confirma a validade','Existe um título no cartão, mas os dados devolvidos não permitem confirmar se está válido agora.']};
  if(!titles.length&&(card.contracts||[]).some(isStoredValue))copy.empty=['Sem passe mensal carregado','O saldo de ocasionais é apresentado abaixo.'];
  $('pass-status').dataset.kind=kind;
  $('pass-status-title').textContent=copy[kind][0];
  $('pass-status-description').textContent=copy[kind][1];
  $('pass-titles').replaceChildren();
  for(const title of titles){const row=node('div',null,'pass-title');row.append(node('strong',title.name),node('span',{active:'Válido agora',future:'Ainda não válido',expired:'Expirado',unknown:'Validade por confirmar'}[title.kind]));if(title.validity)row.append(node('small',title.validity));$('pass-titles').append(row);}
}
function isStoredValue(item) {
  const product=item.product_detail?.product||{};
  const name=(item.contract||[]).find(f=>f.id==='contract.name')?.value||product.name||'';
  return product.id==='id-prod-zapping'||/zapping|pr[eé][ -]?pago/i.test(name)||(item.contract||[]).some(f=>f.id==='contract.amount');
}
function renderBalances(card) {
  const rows=(card.contracts||[]).filter(isStoredValue);
  $('pass-balances').replaceChildren();
  for(const item of rows){
    const field=(item.contract||[]).find(f=>f.id==='contract.amount');
    // The official app formats contract.amount.value_id as integer euro cents.
    const raw=field?.value_id;
    const cents=raw!==null&&raw!==undefined&&/^\d+$/.test(String(raw))?Number(raw):null;
    const row=node('div',null,'pass-balance');
    const name=item.product_detail?.product?.name||(item.contract||[]).find(f=>f.id==='contract.name')?.value||'Ocasionais / Zapping';
    row.append(node('span',name),node('strong',Number.isSafeInteger(cents)?euros(cents):'Saldo não disponível'));
    $('pass-balances').append(row);
  }
  $('pass-balances').hidden=!rows.length;
}
function catalogOptions(group, prefix='') {
  for(const product of group.products||[]) {
    if(state.metropolitanActive&&!state.showOtherPasses&&isMunicipal(product))continue;
    $('product').add(new Option((prefix?prefix+' · ':'')+product.name,product.id));
  }
  for(const child of group.groups||[]) catalogOptions(child,child.name||prefix);
}
async function read() {
  clearReading();$('status').textContent=demo.enabled?'A simular a leitura do cartão…':'A ler o cartão com o serviço Navegante… Mantém o passe inserido.';
  controls();
  const data=await request('read',{reader:$('reader').value});
  state.session=data.session;state.card=data.card;renderCard(data.card);
  $('status').textContent='Leitura concluída. A consultar os produtos disponíveis…';
  // Move on as soon as the card read completes; catalogue checks continue in step 2.
  controls();focusStage();
  await loadCatalog();
  $('status').textContent='Passe lido. Podes consultar os dados e escolher um carregamento.';
}
async function loadCatalog() {
  $('catalog-status').removeAttribute('data-error');
  $('catalog-status').textContent='A consultar os produtos disponíveis…';
  $('product').replaceChildren(new Option('A consultar carregamentos…',''));
  const catalog=await request('catalog',{session:state.session});
  state.catalog=catalog;
  populateCatalog();
  state.catalogReady=true;
  $('catalog-panel').hidden=false;
}
function isMunicipal(product) {
  return /municipal/i.test(product.name)||/^id-prod-navegante-(alcochete|almada|amadora|barreiro|cascais|lisboa|loures|mafra|moita|montijo|odivelas|oeiras|palmela|seixal|sesimbra|setubal|sintra|vfxira)$/.test(product.id);
}
function populateCatalog() {
  $('product').replaceChildren(new Option('Seleciona um produto…',''));catalogOptions(state.catalog);
  $('other-passes').hidden=!state.metropolitanActive;
  $('other-passes').textContent=state.showOtherPasses?'Ocultar passes municipais':'Ver passes municipais / outros períodos';
  $('other-passes').setAttribute('aria-expanded',String(state.showOtherPasses));
  $('catalog-status').textContent=state.metropolitanActive?'Já tens Metropolitano válido. Consulta outro período ou adiciona saldo. Os Municipais ficam nas opções secundárias.':'';
  if($('product').options.length===1)$('catalog-status').textContent='O serviço não devolveu carregamentos disponíveis nesta seleção.';
  if(demo.enabled)$('catalog-status').textContent=state.metropolitanActive?'O mês já carregado fica bloqueado. Os restantes valores e períodos são exemplos fictícios.':'Valores e períodos fictícios, apenas para experimentar.';
}
function variantText(price) {
  const attribute=(state.details?.attributes||[]).find(a=>(a.values||[]).some(v=>v.id===price.id));
  const value=attribute?.values.find(v=>v.id===price.id)?.value;
  let text=price.description||value||'Opção';
  if(attribute?.id==='product_monthly'&&Number(value)>=1&&Number(value)<=12)text=new Intl.DateTimeFormat('pt-PT',{month:'long',timeZone:'Europe/Lisbon'}).format(new Date(Date.UTC(2026,Number(value)-1,15)));
  if(attribute?.id==='product_occasional')text='Saldo de '+euros(Number(value));
  return text+' · '+euros(Number(price.net)+Number(price.tax));
}
function selectedPrice() {return (state.details?.prices||[]).find(p=>p.id===$('variant').value&&!p.blockedReason);}
function showPrice() {const p=selectedPrice();$('price').textContent=p?euros(Number(p.net)+Number(p.tax)):'';state.quote=null;$('checkout-panel').hidden=true;}
function showPayment(data) {
  state.purchase={...state.purchase,...data};$('payment-panel').hidden=false;
  $('sale-reference').textContent=data.saleId||state.purchase.saleId||'A recuperar pelo histórico';
  const messages={pending:'Aguarda a confirmação na tua aplicação MB WAY e atualiza o estado.',paid:'Pagamento confirmado. O carregamento pode ser gravado no passe.',loaded:'O serviço Navegante confirmou o carregamento.',canceled:'O pagamento foi cancelado.',refunded:'O pagamento foi reembolsado.',refund_pending:'O reembolso está pendente.',uncertain:'A gravação ficou por confirmar. Atualiza o estado sem repetir o pagamento ou a gravação.',unknown:'O estado não está confirmado. Consulta a referência do pedido antes de iniciar outro pedido.'};
  $('payment-status').textContent=messages[state.purchase.state]||'Pedido em processamento.';
  if(demo.enabled){const simulated={pending:'Pagamento fictício criado. Clica em «Simular confirmação do pagamento» para continuar.',paid:'Pagamento simulado confirmado. Podes experimentar o carregamento.',loaded:'Carregamento fictício concluído. Nenhum cartão real foi alterado.'};$('payment-status').textContent=simulated[state.purchase.state]||'Estado da demonstração por confirmar.';}
  $('payment-title').textContent=state.purchase.state==='loaded'?'Boa viagem.':'A acompanhar o pedido.';
  if(demo.enabled)$('payment-title').textContent=state.purchase.state==='loaded'?'Demo concluída.':'O teu pedido fictício.';
  $('load-area').hidden=!state.purchase.canLoad||state.purchase.state==='loaded';
  $('success').hidden=state.purchase.state!=='loaded';
  if(state.purchase.state==='loaded'){
    $('pass-balances').hidden=true;
    $('pass-status').dataset.kind='active';
    $('pass-status-title').textContent=demo.enabled?'Carregamento simulado concluído':'Carregamento confirmado no cartão';
    $('pass-status-description').textContent=demo.enabled?'Consulta novamente o cartão fictício para ver o resultado.':'O serviço confirmou a gravação. Consulta novamente o passe para ver o título e a validade atualizados.';
    $('pass-titles').replaceChildren();
  }
}
$('reader').addEventListener('change',()=>{if(!state.purchase)clearReading();controls();});
$('refresh').addEventListener('click',()=>action('status',refresh));
$('toggle-reader').addEventListener('click',()=>{
  if(state.busy||(state.purchase&&state.purchase.state!=='loaded'))return;
  state.purchase=null;clearReading();
  $('pass-balances').hidden=true;
  $('checkout-form').hidden=false;
  $('confirm-purchase').checked=false;$('confirm-write').checked=false;
  $('status').textContent=$('reader').value?'Insere o passe e inicia a leitura.':'Seleciona um leitor para começar.';
  controls();focusStage();
});
$('catalog-retry').addEventListener('click',()=>action('catalog-status',loadCatalog));
$('other-passes').addEventListener('click',()=>{if(state.busy||!state.catalog)return;state.showOtherPasses=!state.showOtherPasses;state.details=null;state.quote=null;$('product-options').hidden=true;populateCatalog();controls();});
$('back-products').addEventListener('click',()=>{if(state.purchase)return;state.quote=null;controls();focusStage();});
$('read').addEventListener('click',()=>action('status',read));
$('product').addEventListener('change',()=>action('catalog-status',async()=>{
  $('product-options').hidden=true;$('checkout-panel').hidden=true;state.quote=null;state.details=null;
  if(!$('product').value)return;
  $('catalog-status').textContent='A consultar opções e preços…';
  state.details=await request('product',{session:state.session,product:$('product').value});
  $('product-name').textContent=state.details.name;$('variant').replaceChildren();
  for(const price of state.details.prices||[])if(!price.blockedReason)$('variant').add(new Option(variantText(price),price.id));
  showPrice();$('product-options').hidden=!$('variant').options.length;
  $('prepare').disabled=!selectedPrice();$('catalog-status').textContent=$('variant').options.length?'':(state.details.prices||[]).find(p=>p.blockedReason)?.blockedReason||'Não existem opções disponíveis para este produto.';
}));
$('variant').addEventListener('change',()=>{showPrice();controls();});
$('prepare').addEventListener('click',()=>action('catalog-status',async()=>{
  state.quote=await request('prepare',{session:state.session,product:$('product').value,variant:$('variant').value});
  fields($('quote-details'),[['Cartão',state.card?.media?.cardNumber],['Título',state.quote.name],['Opção',$('variant').selectedOptions[0]?.textContent],['Total',euros(state.quote.amount)]]);
  $('checkout').textContent=demo.enabled?'Simular pagamento de '+euros(state.quote.amount):state.quote.amount?'Pedir pagamento de '+euros(state.quote.amount)+' por MB WAY':'Pedir carregamento gratuito';
  $('phone-field').hidden=state.quote.amount===0;$('phone').required=state.quote.amount!==0;
  $('confirm-purchase').checked=false;$('checkout-panel').hidden=false;
  $('checkout-status').textContent=demo.enabled?'Os dados de exemplo já estão preenchidos. Não será enviado nenhum recibo nem pedido de pagamento.':'O e-mail abaixo serve apenas para o recibo.';
}));
$('checkout-form').addEventListener('submit',event=>{event.preventDefault();action('checkout-status',async()=>{
  if(!$('confirm-purchase').checked||!state.quote)return;
  $('checkout-status').textContent=demo.enabled?'A criar o pagamento fictício…':'A criar o pedido. Aguarda e não repitas o pagamento.';
  state.purchase={state:'submitting'};
  try {
    const data=await request('checkout',{session:state.session,confirmedAmount:state.quote.amount,email:$('receipt-email').value.trim(),phone:$('phone').value.trim(),nif:$('nif').value.trim()});
    showPayment(data);$('checkout-form').hidden=true;$('checkout-status').textContent='Pedido criado. Guarda a referência apresentada abaixo.';
  } catch(error) {showPayment({state:'unknown'});throw error;}
});});
$('poll').addEventListener('click',()=>action('payment-status',async()=>{
  $('payment-status').textContent='A consultar o serviço Navegante…';
  showPayment(await request('payment',{session:state.session}));
}));
$('confirm-write').addEventListener('change',controls);
$('load').addEventListener('click',()=>action('payment-status',async()=>{
  if(!$('confirm-write').checked)return;
  $('payment-status').textContent=demo.enabled?'A simular o carregamento no cartão fictício…':'A gravar o carregamento. Mantém o passe no leitor até à confirmação.';
  try {showPayment(await request('load',{session:state.session,reader:$('reader').value,confirmWrite:true}));}
  catch(error) {state.purchase.canLoad=false;$('load-area').hidden=true;throw error;}
}));
$('new-read').addEventListener('click',()=>action('status',async()=>{state.purchase=null;$('payment-panel').hidden=true;$('checkout-form').hidden=false;await read();}));
$('recover-form').addEventListener('submit',event=>{event.preventDefault();action('recover-status',async()=>{
  const recovered=await request('recover',{saleId:$('recover-reference').value.trim(),reader:$('reader').value});
  clearReading();state.session=recovered.session;state.purchase={};showPayment({...recovered,state:'pending'});
  showPayment(await request('payment',{session:state.session}));
});});
window.addEventListener('beforeunload',event=>{if(!demo.enabled&&state.purchase&&state.purchase.state!=='loaded'){event.preventDefault();event.returnValue='';}});
$('demo-toggle').addEventListener('click',()=>changeDemo(!demo.enabled));
$('demo-reset').addEventListener('click',()=>changeDemo(true));
$('bridge-form').addEventListener('submit',event=>{event.preventDefault();action('bridge-status',async()=>{
  $('bridge-status').textContent='Autoriza a ligação na janela do Navegante Link…';
  try {
    const info=await request('pair',{client:'navegante-web'});
    if(info.service!=='navegante-local'||info.version!==1||typeof info.token!=='string'||!/^[A-Za-z0-9_-]{20,128}$/.test(info.token))throw Error('Atualiza o Navegante Link e tenta novamente.');
    bridgeClient.token=info.token;bridgeClient.ready=true;
    await refresh();$('bridge-status').textContent='Ligado ao leitor do teu PC. Podes inserir o passe e iniciar a leitura.';
  }catch(error){bridgeClient.token='';bridgeClient.ready=false;throw error;}
});});
$('bridge-disconnect').addEventListener('click',()=>{
  if(state.busy||(state.purchase&&state.purchase.state!=='loaded'))return;
  bridgeClient.token='';bridgeClient.ready=false;resetInterface();
  $('reader').replaceChildren(new Option('Liga primeiro o leitor do teu PC',''));
  $('bridge-status').textContent='Ligação terminada neste navegador.';controls();
});
// Keep one physical-card preview throughout the flow, including its details.
const passPreview=document.querySelector('.pass');
passPreview.insertBefore($('pass-status'),passPreview.querySelector('.pass-bottom'));
const balances=node('div',null,'pass-balances');balances.id='pass-balances';balances.hidden=true;
passPreview.insertBefore(balances,passPreview.querySelector('.pass-bottom'));
passPreview.insertBefore($('result'),passPreview.querySelector('.pass-bottom'));
const reducedMotion=matchMedia('(prefers-reduced-motion: reduce)');
function resetCardMotion(){passPreview.style.removeProperty('--card-tilt-x');passPreview.style.removeProperty('--card-tilt-y');}
passPreview.addEventListener('pointermove',event=>{
  if(event.pointerType!=='mouse'||reducedMotion.matches||!state.card)return;
  const rect=passPreview.getBoundingClientRect();
  passPreview.style.setProperty('--card-tilt-x',((.5-(event.clientY-rect.top)/rect.height)*5).toFixed(2)+'deg');
  passPreview.style.setProperty('--card-tilt-y',(((event.clientX-rect.left)/rect.width-.5)*7).toFixed(2)+'deg');
});
passPreview.addEventListener('pointerleave',resetCardMotion);
passPreview.addEventListener('pointercancel',resetCardMotion);
reducedMotion.addEventListener('change',resetCardMotion);
if(hostedDemo)changeDemo(true);
else if(remoteMode){$('reader').replaceChildren(new Option('Liga primeiro o leitor do teu PC',''));$('status').textContent='Abre o Navegante Link e liga o leitor ou experimenta a demo.';controls();}
else action('status',refresh);
