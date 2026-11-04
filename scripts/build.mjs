import {mkdir,readFile,writeFile,copyFile} from 'node:fs/promises';
import {fileURLToPath} from 'node:url';
const root=fileURLToPath(new URL('../',import.meta.url));
await mkdir(new URL('../dist/',import.meta.url),{recursive:true});
for(const file of ['app.js','style.css','forms.css'])await copyFile(`${root}web/${file}`,`${root}dist/${file}`);
const html=await readFile(`${root}web/index.html`,'utf8');
await writeFile(`${root}dist/index.html`,html.replace('<body data-stage="read">','<body data-stage="read" data-remote="true">'));
console.log('Static site built in dist/. Real operations connect to the paired local PC/SC bridge.');
