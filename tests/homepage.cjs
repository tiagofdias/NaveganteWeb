const {chromium,launchOptions}=require('./browser-env.cjs');
const fs=require('node:fs');
const path=require('node:path');
const assert=require('node:assert/strict');
(async()=>{
  const browser=await chromium.launch({...launchOptions,headless:true});
  try{
    const page=await browser.newPage();
    const errors=[];page.on('pageerror',error=>errors.push(error.message));
    await page.addInitScript(()=>{window.fetchAttempts=0;window.fetch=()=>{window.fetchAttempts++;throw Error('Unexpected network request');};});
    await page.route('https://navegante-home.test/**',async route=>{
      const file=new URL(route.request().url()).pathname.slice(1).replace(/\/$/,'/index.html')||'index.html';
      assert.ok(['index.html','landing.css','demo/index.html','app.js','style.css','forms.css'].includes(file));
      await route.fulfill({contentType:file.endsWith('.css')?'text/css':file.endsWith('.js')?'text/javascript':'text/html',
        headers:{'Content-Security-Policy':"default-src 'self'; connect-src 'none'; base-uri 'self'; form-action 'self'"},
        body:fs.readFileSync(path.join(__dirname,'..','dist',file))});
    });
    await page.goto('https://navegante-home.test');
    assert.equal(await page.locator('script').count(),0,'Homepage does not load the demo runtime');
    assert.equal(await page.locator('#read').count(),0,'Reader workflow is not the homepage');
    assert.match(await page.locator('#download-desktop').getAttribute('href'),/v2\.1\.1\/Navegante-Web\.exe$/);
    for(const width of [1440,1024,768,390,360]){
      await page.setViewportSize({width,height:1000});
      assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth),false,'Overflow at '+width);
    }
    await page.locator('.faq summary').first().click();
    assert.equal(await page.locator('.faq details').first().getAttribute('open'),'');
    await page.locator('.demo-link').click();
    await page.waitForFunction(()=>!document.querySelector('#read').disabled);
    assert.equal(new URL(page.url()).pathname,'/demo/');
    assert.equal(await page.locator('#demo-banner').isVisible(),true);
    assert.equal(await page.evaluate(()=>window.fetchAttempts),0);
    await page.locator('.brand').click();
    await page.locator('#download-desktop').waitFor();
    assert.deepEqual(errors,[]);
    console.log('PASS: desktop-first homepage, valid download, no demo runtime on home, five responsive sizes, FAQ, optional demo route and navigation back home.');
  }finally{await browser.close();}
})().catch(error=>{console.error(error);process.exitCode=1;});
