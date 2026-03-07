import { defineConfig } from 'vite';
import vue from '@vitejs/plugin-vue';
import http from 'http';

const noKeepAliveAgent = new http.Agent({ keepAlive: false });

function makeProxy(extra: object = {}) {
  return {
    target: 'http://localhost:3000',
    agent: noKeepAliveAgent,
    configure(proxy: any) {
      proxy.on('error', (err: Error, _req: any, res: any) => {
        const msg = err?.message ?? 'proxy error';
        console.error(`[proxy] ${msg}`);
        if (!res.headersSent) res.writeHead(502).end(JSON.stringify({ error: msg }));
      });
    },
    ...extra,
  };
}

export default defineConfig({
  plugins: [vue()],
  publicDir: 'static',
  build: {
    outDir: '../../dist',
    emptyOutDir: true,
  },
  server: {
    proxy: {
      '/api': makeProxy({ proxyTimeout: 300_000, timeout: 300_000 }),
      '/uploads': makeProxy(),
    },
  },
});
