import {build,loadConfigFromFile} from 'vite';
import path from 'node:path';import {fileURLToPath} from 'node:url';
const root=fileURLToPath(new URL('../../../',import.meta.url));
const out=fileURLToPath(new URL('build/',import.meta.url));
process.env.NODE_ENV='production';
const loaded=await loadConfigFromFile({command:'build',mode:'production'},path.join(root,'vite.config.ts'));
// Prevent the visualizer from overwriting the repository's tracked stats.html.
loaded.config.plugins=loaded.config.plugins.filter(plugin=>plugin?.name!=='visualizer');
await build({...loaded.config,configFile:false,root,build:{...loaded.config.build,outDir:out}});
