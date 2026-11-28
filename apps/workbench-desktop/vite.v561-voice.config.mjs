import {fileURLToPath} from 'node:url';
export default {root:fileURLToPath(new URL('./',import.meta.url)),resolve:{dedupe:['react','react-dom']},build:{outDir:'../../target-windows/v561-voice-ui',emptyOutDir:true,rollupOptions:{input:fileURLToPath(new URL('./v561-voice.html',import.meta.url))}}};
