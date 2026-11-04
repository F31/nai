import {spawnSync} from 'node:child_process';
import path from 'node:path';
const root=process.cwd();
for(const [file,args,cwd] of [['packages/contracts-ts/node_modules/typescript/bin/tsc',['-p','packages/contracts-ts/tsconfig.json'],root],['apps/workbench-desktop/node_modules/typescript/bin/tsc',['--noEmit','-p','apps/workbench-desktop/tsconfig.json'],root],['apps/workbench-desktop/node_modules/vite/bin/vite.js',['build','--config',path.join(root,'apps/workbench-desktop/vite.config.mjs')],root]]){const r=spawnSync(process.execPath,[path.join(root,file),...args],{cwd,stdio:'inherit',timeout:180000});if(r.error)throw r.error;if(r.status!==0)process.exit(r.status??1);}

