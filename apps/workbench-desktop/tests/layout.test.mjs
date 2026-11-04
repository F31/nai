import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
import path from 'node:path';
const root=path.resolve(import.meta.dirname,'..');
test('workbench owns its entry, build, dependencies and native window',async()=>{
 const pkg=JSON.parse(await fs.readFile(path.join(root,'package.json'),'utf8'));assert.equal(pkg.name,'@nai/workbench-desktop');
 const native=JSON.parse(await fs.readFile(path.join(root,'tauri.conf.json'),'utf8'));assert.equal(native.app.windows[0].url,'index.html');assert.ok(native.build.devUrl.endsWith('/index.html'));
 for(const file of ['index.html','vite.config.mjs','tsconfig.json']){const source=await fs.readFile(path.join(root,file),'utf8');assert.doesNotMatch(source,/apps\/desktop|\.\.\/desktop|src\/v43|v43\.html/);}
 await assert.rejects(fs.access(path.resolve(root,'../desktop')));const entry=await fs.readFile(path.join(root,'index.html'),'utf8');assert.match(entry,/\/ui\/main\.tsx/);
});
test('production index points to existing owned assets and retains React mount',async()=>{
 const dist=path.join(root,'dist'),html=await fs.readFile(path.join(dist,'index.html'),'utf8');const assets=[...html.matchAll(/(?:src|href)="([^"\s]+\.(?:js|css))"/g)].map(m=>m[1]);assert.ok(assets.length>=2);
 for(const asset of assets){const file=path.resolve(dist,'.'+asset);assert.ok(file.startsWith(dist+path.sep));await fs.access(file);}
 assert.match(html,/id="root"/);await assert.rejects(fs.access(path.join(dist,'v43.html')));const main=await fs.readFile(path.join(root,'ui/main.tsx'),'utf8');assert.match(main,/createRoot/);
});
