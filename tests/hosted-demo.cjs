const {chromium,launchOptions}=require('./browser-env.cjs');
const fs=require('node:fs');
const path=require('node:path');
const assert=require('node:assert/strict');
(async()=>{
  const browser=await chromium.launch({...launchOptions,headless:true});
  try{
    const page=await browser.newPage();
    const errors=[];page.on('pageerror',error=>errors.push(error.message));
    await page.addInitScript(()=>{window.fetchAttempts=0;window.fetch=()=>{window.fetchAttempts++;throw Error('Network forbidden in hosted demo test');};});
    await page.route('https://navegante-demo.test/**',async route=>{
      const file=new URL(route.request().url()).pathname.slice(1).replace(/\/$/,'/index.html')||'index.html';
      assert.ok(['index.html','demo/index.html','app.js','style.css','forms.css'].includes(file));
      await route.fulfill({contentType:file.endsWith('.css')?'text/css':file.endsWith('.js')?'text/javascript':'text/html',
        headers:{'Content-Security-Policy':"default-src 'self'; connect-src 'none'; base-uri 'self'; form-action 'self'"},
        body:fs.readFileSync(path.join(__dirname,'..','dist',file))});
    });
    await page.goto('https://navegante-demo.test/demo/');
    await page.waitForFunction(()=>!document.querySelector('#read').disabled);
    assert.equal(await page.locator('#demo-banner').isVisible(),true);
    assert.equal(await page.locator('#demo-toggle').isVisible(),false);
    assert.equal(await page.locator('#bridge-panel').isVisible(),false);
    assert.equal(await page.locator('#live-instructions').isVisible(),true);
    assert.match(await page.locator('#live-instructions .project-download').getAttribute('href'),/Navegante-Web\.exe$/);
    await page.locator('#read').click();await page.waitForFunction(()=>!document.querySelector('#product').disabled);
    await page.locator('#product').selectOption('id-prod-zapping');await page.locator('#product-options').waitFor();
    await page.locator('#prepare').click();await page.locator('#checkout-panel').waitFor();
    await page.locator('#confirm-purchase').check();await page.locator('#checkout').click();
    await page.locator('#payment-panel').waitFor();await page.locator('#poll').click();await page.locator('#load-area').waitFor();
    await page.locator('#confirm-write').check();await page.locator('#load').click();await page.locator('#success').waitFor();
    await page.evaluate(()=>changeDemo(false));
    assert.equal(await page.evaluate(()=>demo.enabled),true,'Hosted demo cannot switch to real mode');
    const result=await page.evaluate(async()=>{demo.enabled=false;try{await request('readers');return 'failed';}catch(error){return error.message;}finally{demo.enabled=true;}});
    assert.match(result,/demonstração/);
    assert.equal(await page.evaluate(()=>window.fetchAttempts),0);
    for(const width of [1280,768,390,360]){
      await page.setViewportSize({width,height:900});
      await page.locator('#live-instructions summary').evaluate(n=>n.parentElement.open=true);
      assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth),false);
    }
    assert.deepEqual(errors,[]);
    console.log('PASS: hosted demo starts automatically, cannot exit to real operations, complete simulated checkout/load, zero fetch calls, desktop app download/instructions and responsive layouts.');
  }finally{await browser.close();}
})().catch(error=>{console.error(error);process.exitCode=1;});
