// Mimir web UI. `npm run build` emits exactly dist/{index.html,app.js,app.css}, embedded by the Rust binary (build.rs).
// `npm run dev` serves on :7441 and proxies /api and /match to the server, adding the bearer token server-side.
import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';
import { readFileSync } from 'node:fs';
import { homedir } from 'node:os';

const TARGET = process.env.MIMIR_URL || 'http://127.0.0.1:7440';

function token() {
  if (process.env.MIMIR_TOKEN) return process.env.MIMIR_TOKEN;
  try {
    const env = readFileSync(`${homedir()}/.config/mimir/env`, 'utf8');
    const m = env.match(/^\s*(?:export\s+)?MIMIR_TOKEN\s*=\s*["']?([^"'\r\n]+)["']?/m);
    return m ? m[1].trim() : '';
  } catch {
    return '';
  }
}

const rule = {
  target: TARGET,
  changeOrigin: true,
  configure: (p) => p.on('proxyReq', (req) => {
    const t = token();
    if (t) req.setHeader('Authorization', `Bearer ${t}`);
  }),
};

export default defineConfig({
  plugins: [svelte()],
  build: {
    outDir: 'dist',
    cssCodeSplit: false,
    modulePreload: { polyfill: false },
    rollupOptions: {
      output: {
        entryFileNames: 'app.js',
        chunkFileNames: 'app-[name].js',
        assetFileNames: 'app.[ext]',
      },
    },
  },
  server: { port: 7441, host: true, proxy: { '/api': rule, '/match': rule } },
});
