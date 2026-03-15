// Build script: bundles src/extension.ts -> out/extension.js
// Source: code.visualstudio.com/api/working-with-extensions/bundling-extension
const esbuild = require('esbuild');
const production = process.argv.includes('--production');

esbuild.build({
  entryPoints: ['src/extension.ts'],
  bundle: true,
  outfile: 'out/extension.js',
  external: ['vscode'],  // CRITICAL: vscode is provided by extension host, never bundle it
  format: 'cjs',
  platform: 'node',
  minify: production,
  sourcemap: !production,
}).catch(() => process.exit(1));
