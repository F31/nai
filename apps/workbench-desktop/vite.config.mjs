import {defineConfig} from 'vite';
import {fileURLToPath} from 'node:url';
export default defineConfig({root:fileURLToPath(new URL('./',import.meta.url)),resolve:{alias:{'@nai/contracts':fileURLToPath(new URL('../../packages/contracts-ts/src/index.ts',import.meta.url))},dedupe:['react','react-dom']},build:{outDir:'dist',emptyOutDir:true,rollupOptions:{input:fileURLToPath(new URL('./index.html',import.meta.url))}}});
