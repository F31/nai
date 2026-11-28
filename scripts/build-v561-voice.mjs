// Use an existing dependency workspace; no install, no node_modules rewriting.
import {createRequire} from 'node:module';
import {fileURLToPath,pathToFileURL} from 'node:url';
import path from 'node:path';
import {readFile,mkdir,writeFile} from 'node:fs/promises';
import {spawnSync} from 'node:child_process';
const root=fileURLToPath(new URL('../',import.meta.url));
const args=process.argv.slice(2);
if(args.length&&!(args.length===2&&args[0]==='--deps-root'))throw Error('usage: build-v561-voice.mjs [--deps-root existing-workspace]');
const deps=path.resolve(args[1]??root),app=createRequire(path.join(deps,'apps/workbench-desktop/package.json'));
const contracts=createRequire(path.join(deps,'packages/contracts-ts/package.json'));
const renderer=createRequire(app.resolve('react-dom/client'));
const typesReact=path.dirname(app.resolve('@types/react/package.json'));
const typesDom=path.dirname(app.resolve('@types/react-dom/package.json'));
let wireRoot=path.dirname(contracts.resolve('@bufbuild/protobuf/wire')),wirePackage;
while(true){try{const candidate=JSON.parse(await readFile(path.join(wireRoot,'package.json'),'utf8'));if(candidate.name==='@bufbuild/protobuf'){wirePackage=candidate;break;}}catch{}const parent=path.dirname(wireRoot);if(parent===wireRoot)throw Error('protobuf package metadata missing');wireRoot=parent;}
const wireTypes=path.resolve(wireRoot,wirePackage.exports['./wire'].import.types);
const wireModule=path.resolve(wireRoot,wirePackage.exports['./wire'].import.default);
const stage=path.join(root,'target-windows/tool-cache/voice-ui-build');await mkdir(stage,{recursive:true});
const source=JSON.parse(await readFile(path.join(root,'apps/workbench-desktop/tsconfig.v561-voice.json'),'utf8'));
const tsconfig={...source,compilerOptions:{...source.compilerOptions,baseUrl:root,paths:{
 'react':[path.join(typesReact,'index.d.ts')],'react/*':[path.join(typesReact,'*')],
 'react-dom/*':[path.join(typesDom,'*')],
 '@bufbuild/protobuf/wire':[wireTypes]
}},include:[path.join(root,'apps/workbench-desktop/ui/v561-voice.tsx')]};
const configFile=path.join(stage,'tsconfig.json');await writeFile(configFile,JSON.stringify(tsconfig));
const checked=spawnSync(process.execPath,[app.resolve('typescript/bin/tsc'),'--noEmit','-p',configFile],{stdio:'inherit'});
if(checked.error)throw checked.error;if(checked.status!==0)process.exit(checked.status??1);
const {build}=await import(pathToFileURL(app.resolve('vite')).href);
const config=(await import(pathToFileURL(path.join(root,'apps/workbench-desktop/vite.v561-voice.config.mjs')).href)).default;
const output=path.resolve(config.root,config.build.outDir),target=path.join(root,'target-windows');
if(!output.startsWith(target+path.sep))throw Error('build output must stay under project target-windows');
await build({...config,configFile:false,plugins:[{name:'one-runtime-and-wire',enforce:'pre',resolveId(id){
 if(id==='react'||id.startsWith('react/'))return renderer.resolve(id);
 if(id==='react-dom'||id==='react-dom/client')return app.resolve(id);
 if(id==='@bufbuild/protobuf/wire')return wireModule;
}}]});
