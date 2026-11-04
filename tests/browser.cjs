// Uses a locally installed Playwright and an existing Edge installation.
// Live card reads are opt-in. No login, OTP request, payment or write is performed.
const { chromium, launchOptions } = require('./browser-env.cjs');
(async()=>{
  const browser=await chromium.launch({...launchOptions,headless:true});
  const page=await browser.newPage();
  try {
  const errors=[];
  page.on('pageerror',error=>errors.push(error.message));
  await page.goto('http://127.0.0.1:8765'); console.log('Page loaded');
  await page.locator('#reader option[value="THRC Smart Card Reader 0"]').waitFor({state:'attached'});
  if(await page.locator('#reader').inputValue()!=='THRC Smart Card Reader 0')throw Error('Reader selection failed');
  await page.setViewportSize({width:390,height:844});
  if(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth))throw Error('Mobile overflow');
  await page.setViewportSize({width:1280,height:900});
  if(process.env.NAVEGANTE_LIVE_READ==='1'){
    await page.locator('#read').click();
    await page.locator('#catalog-panel').waitFor({state:'visible',timeout:120000});
    console.log('Live card read passed');
    await page.locator('#product').selectOption('id-prod-navegante-metro');
    await page.locator('#product-options').waitFor({state:'visible'});
    await page.locator('#prepare').click();
    try { await page.locator('#checkout-panel').waitFor({state:'visible'}); }
    catch(error) {throw Error('Price review failed: '+await page.locator('#catalog-status').textContent());}
    if(await page.locator('#checkout').isDisabled())throw Error('Physical-card checkout must allow guests');
    if(await page.locator('#login-form').count())throw Error('No account form in physical-card flow');
    await page.setViewportSize({width:390,height:844});
    if(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth))throw Error('Mobile data overflow');
    console.log('PASS: real card read, official catalogue, product configuration, price review, guest checkout');
  }
  if(errors.length)throw Error('Browser errors: '+errors.length);
  console.log('PASS: desktop/mobile page, no JavaScript errors');
  } finally { await browser.close(); }
})().catch(error=>{console.error(error.message);process.exitCode=1;});
