import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vite';
import http from 'http';

// Отдельный Agent без keep-alive для каждого проксируемого пути.
// Это решает EPIPE (старые keep-alive соединения не переиспользуются)
// без добавления заголовка Connection: close, который обрывает загрузку больших файлов.
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
  plugins: [sveltekit()],
  server: {
    proxy: {
      '/api': makeProxy({ proxyTimeout: 300_000, timeout: 300_000 }),
      '/uploads': makeProxy(),
    },
  },
});
