const fs=require('node:fs');
let playwright;
try{playwright=require('playwright');}
catch{playwright=require('../analysis/tools/browser/node_modules/playwright');}
const edge=process.env.PLAYWRIGHT_BROWSER_EXECUTABLE||'C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe';
module.exports={chromium:playwright.chromium,launchOptions:fs.existsSync(edge)?{executablePath:edge}:{}};
