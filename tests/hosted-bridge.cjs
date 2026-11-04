// Real browser CORS/LNA and loopback transport; hardware/service operations mocked.
const {chromium,launchOptions}=require('./browser-env.cjs');
const {spawn}=require('node:child_process');
const fs=require('node:fs');
const path=require('node:path');
const assert=require('node:assert/strict');
const origin='https://navegante-bridge.test';
const token='TEST-ONLY-pairing-code-012345678901234567890';

(async()=>{
  const source=`
import json
from http.server import ThreadingHTTPServer
import server
from bridge import BridgeAccess
class Readers:
 def list_readers(self): return ['THRC Mock Reader 0']
class Service:
 def run(self, operation, data):
  if operation == 'read': return {'session':'TEST-session','card':{'media':{'cardNumber':'TEST-0001'},'holder':[{'id':'holder.name','value':'Example User'}],'contracts':[]}}
  if operation == 'catalog': return {'products':[{'id':'example','name':'Example product'}]}
  raise AssertionError('Unexpected operation: '+operation)
class Quiet(server.Handler):
 def log_message(self, *args): pass
server.PCSC=Readers
server.APP=Service()
http=ThreadingHTTPServer(('127.0.0.1',0),Quiet)
approvals=iter([False,True])
http.bridge=BridgeAccess('${origin}','${token}',approve=lambda:next(approvals,False))
print(json.dumps({'port':http.server_port}),flush=True)
http.serve_forever()
`;
  const child=spawn(process.env.PYTHON||'python',['-u','-c',source],{cwd:path.join(__dirname,'..'),stdio:['ignore','pipe','pipe'],windowsHide:true});
  let browser;
  try{
    const port=await new Promise((resolve,reject)=>{
      const timer=setTimeout(()=>reject(Error('Bridge test startup timed out')),10000);
      child.once('error',reject);
      child.stdout.once('data',chunk=>{clearTimeout(timer);resolve(JSON.parse(String(chunk).trim()).port);});
      child.once('exit',code=>{clearTimeout(timer);reject(Error('Bridge test process exited: '+code));});
    });
    browser=await chromium.launch({...launchOptions,headless:true});
    const context=await browser.newContext();
    const cdp=await browser.newBrowserCDPSession();
    const contexts=await cdp.send('Target.getBrowserContexts');
    await cdp.send('Browser.setPermission',{browserContextId:contexts.browserContextIds[0],permission:{name:'loopback-network'},setting:'granted',origin});
    const page=await context.newPage();
    const calls=[],errors=[];
    page.on('pageerror',error=>errors.push(error.message));
    page.on('request',r=>{if(r.url().startsWith(`http://127.0.0.1:${port}`))calls.push(new URL(r.url()).pathname);});
    await page.route(origin+'/**',async route=>{
      const file=new URL(route.request().url()).pathname.slice(1)||'index.html';
      // Exercise the optional bridge independently of the demo-only public build.
      const content=fs.readFileSync(path.join(__dirname,'..','dist',file==='index.html'?'demo/index.html':file),'utf8').replace('data-hosted-demo="true"','data-remote="true"').replaceAll('http://127.0.0.1:8765',`http://127.0.0.1:${port}`);
      await route.fulfill({contentType:file.endsWith('.css')?'text/css':file.endsWith('.js')?'text/javascript':'text/html',
        headers:{'Content-Security-Policy':`default-src 'self'; connect-src 'self' http://127.0.0.1:${port}; base-uri 'self'; form-action 'self'`},body:content});
    });
    await page.goto(origin);
    assert.equal(calls.length,0,'Hosted visits must not access the local bridge automatically');
    await page.locator('#bridge-connect').click();
    await page.locator('#bridge-status[data-error=true]').waitFor();
    await page.locator('#bridge-connect').click();
    await page.waitForFunction(()=>!document.querySelector('#bridge-disconnect').hidden);
    assert.equal(await page.locator('#bridge-code').count(),0,'No manual pairing code in consumer UI');
    await page.locator('#reader').selectOption('THRC Mock Reader 0');
    await page.locator('#read').click();
    await page.waitForFunction(()=>!document.querySelector('#product').disabled);
    assert.equal(await page.locator('#pass-name').textContent(),'Example User');
    assert.ok(calls.includes('/api/read')&&calls.includes('/api/catalog'));
    assert.equal(await page.evaluate(()=>localStorage.length+sessionStorage.length),0);
    for(const width of [1280,390,360]){
      await page.setViewportSize({width,height:900});
      assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth),false);
    }
    const before=calls.length;
    await page.locator('#demo-toggle').click();
    await page.waitForFunction(()=>!document.querySelector('#read').disabled);
    await page.locator('#read').click();
    await page.waitForFunction(()=>!document.querySelector('#product').disabled);
    assert.equal(calls.length,before,'Demo must stay isolated from the paired reader');
    assert.deepEqual(errors,[]);
    console.log('PASS: hosted HTTPS UI, CORS/LNA loopback pairing, invalid token rejection, remote read/catalog, no persistent token, responsive layout, isolated demo. Hardware and official APIs mocked.');
  }finally{
    if(browser)await browser.close();
    child.kill();
  }
})().catch(error=>{console.error(error);process.exitCode=1;});
