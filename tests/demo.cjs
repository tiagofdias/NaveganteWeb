// Browser-level isolation test. No official service or card is contacted.
const {chromium,launchOptions}=require('./browser-env.cjs');
const assert=require('node:assert/strict');

(async()=>{
  const browser=await chromium.launch({...launchOptions,headless:true});
  try {
    const page=await browser.newPage({viewport:{width:1280,height:900}});
    const calls=[],errors=[];
    page.on('pageerror',error=>errors.push(error.message));
    await page.route('**/api/**',async route=>{
      const op=new URL(route.request().url()).pathname.split('/').pop();calls.push(op);
      if(op!=='readers')throw Error('Unexpected live API request: '+op);
      await route.fulfill({json:{readers:['THRC Smart Card Reader 0']}});
    });
    const ready=()=>page.waitForFunction(()=>!document.querySelector('#read').disabled);
    const catalogue=()=>page.waitForFunction(()=>!document.querySelector('#product').disabled);
    const reset=async scenario=>{
      await page.locator('#demo-reset').click();await ready();
      await page.locator('#demo-scenario').selectOption(scenario);
      await page.locator('#read').click();await catalogue();
    };
    const layouts=async()=>{
      for(const width of [1280,768,390,360]){
        await page.setViewportSize({width,height:900});
        assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth),false,'Horizontal overflow at '+width);
      }
      await page.setViewportSize({width:1280,height:900});
    };
    await page.goto('http://127.0.0.1:8765');await ready();
    assert.deepEqual(calls,['readers']);
    await page.locator('#demo-toggle').click();await ready();
    assert.equal(await page.locator('#demo-banner').isVisible(),true);
    assert.match(await page.locator('#reader').inputValue(),/Demo/);
    await layouts();
    await page.locator('#read').click();await catalogue();
    assert.match(await page.locator('#pass-name').textContent(),/demo/);
    assert.match(await page.locator('#pass-balances').textContent(),/12,35/);
    await page.locator('#product').selectOption('id-prod-zapping');
    await page.locator('#product-options').waitFor();
    await page.locator('#prepare').click();await page.locator('#checkout-panel').waitFor();
    assert.equal(await page.locator('#receipt-email').inputValue(),'demo@example.invalid');
    assert.match(await page.locator('#checkout').textContent(),/Simular/);
    await layouts();
    await page.locator('#confirm-purchase').check();await page.locator('#checkout').click();
    await page.locator('#payment-panel').waitFor();
    assert.equal(await page.locator('#sale-reference').textContent(),'DEMO-0001');
    await page.locator('#poll').click();await page.locator('#load-area').waitFor();
    await page.locator('#confirm-write').check();await page.locator('#load').click();
    await page.locator('#success').waitFor();await layouts();
    await page.locator('#new-read').click();await catalogue();
    assert.match(await page.locator('#pass-balances').textContent(),/22,35/,'Simulated top-up must update the virtual balance');

    await reset('valid');
    assert.equal(await page.locator('#pass-status').getAttribute('data-kind'),'active');
    await page.locator('#product').selectOption('id-prod-navegante-metro');
    await page.waitForFunction(()=>document.querySelector('#main').getAttribute('aria-busy')==='false');
    const day=await page.evaluate(()=>demoCalendar().day);
    assert.equal(await page.locator('#variant option').count(),day<26?0:1,'Already loaded month is excluded and next month starts on day 26');
    if(day<26)assert.equal(await page.locator('#prepare').isDisabled(),true);
    const calendarCases=await page.evaluate(()=>{
      demo.card=null;
      return ['2026-11-04T12:00:00Z','2026-11-25T23:59:59Z','2026-11-26T00:00:00Z','2026-12-26T12:00:00Z'].map(time=>{
        const value=demoProduct('id-prod-navegante-metro',new Date(time)).attributes[0].values;
        return value.map(v=>`${v.year}-${v.value}`);
      });
    });
    assert.deepEqual(calendarCases,[['2026-11'],['2026-11'],['2026-12'],['2027-1']],'Sale window, cutoff and year rollover');

    await reset('expired');
    assert.equal(await page.locator('#pass-status').getAttribute('data-kind'),'empty');
    assert.deepEqual(calls,['readers'],'Demo must make zero API requests');
    assert.equal(await page.evaluate(()=>localStorage.length+sessionStorage.length),0,'Demo stays in memory');
    await page.locator('#demo-toggle').click();await ready();
    assert.equal(await page.locator('#demo-banner').isVisible(),false);
    assert.equal(await page.locator('body').getAttribute('data-stage'),'read');
    assert.equal(await page.locator('#receipt-email').inputValue(),'');
    assert.deepEqual(calls,['readers','readers'],'Only explicit exit may refresh real readers');
    await page.evaluate(()=>{state.purchase={state:'pending',saleId:'LIVE-EXAMPLE'};controls();});
    assert.equal(await page.locator('#demo-toggle').isDisabled(),true,'An active live purchase cannot be replaced by demo');
    assert.deepEqual(errors,[]);
    console.log('PASS: demo scenarios, simulated payment/write, updated virtual balance, duplicate filtering, responsive layout, mode exit and zero live requests during demo.');
  } finally {await browser.close();}
})().catch(error=>{console.error(error);process.exitCode=1;});
