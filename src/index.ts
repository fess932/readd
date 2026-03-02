import { Elysia } from "elysia";
import { cors } from "@elysiajs/cors";
import { join } from "path";
import { log, reqId, formatBytes } from "./logger";
import { authRoutes } from "./routes/auth";
import { booksRoutes } from "./routes/books";
import { libraryRoutes } from "./routes/library";
import { progressRoutes } from "./routes/progress";

const PORT = 3000;

// SO_REUSEPORT в Bun позволяет нескольким процессам слушать один порт —
// без явной проверки новый сервер стартует, но запросы идут и к старому.
async function assertPortFree(port: number) {
  const conn = await Bun.connect({
    hostname: "127.0.0.1",
    port,
    socket: { data() {}, open() {}, close() {}, error() {} },
  }).catch(() => null);

  if (conn) {
    conn.end();
    log.error(`Port ${port} is already in use`);
    process.stderr.write(`\x1b[33m  kill \$(lsof -ti :${port})\x1b[0m\n\n`);
    process.exit(1);
  }
}

await assertPortFree(PORT);

// Process-level ошибки — ловим всё что не поймал Elysia
process.on("uncaughtException", (err) => {
  log.error("Uncaught exception", err);
});
process.on("unhandledRejection", (reason) => {
  log.error(
    "Unhandled rejection",
    reason instanceof Error ? reason : new Error(String(reason)),
  );
});

const start = new Date().toISOString();
const reqMeta = new WeakMap<Request, { id: string; start: number }>();

const app = new Elysia()
  .onRequest(({ request }) => {
    const url = new URL(request.url);
    if (url.pathname.startsWith("/uploads")) return;
    const id = reqId();
    reqMeta.set(request, { id, start: Date.now() });
    const contentLength = request.headers.get("content-length");
    const size = contentLength ? ` ${formatBytes(Number(contentLength))}` : "";
    log.req(request.method, url.pathname, id + size);
  })
  .onAfterHandle(({ request, set, response }) => {
    const url = new URL(request.url);
    if (url.pathname.startsWith("/uploads")) return;

    const m = reqMeta.get(request);
    if (!m) return;

    const status = typeof set.status === "number" ? set.status : 200;
    const ms = Date.now() - m.start;

    // Роут вернул { error: '...' } с 4xx/5xx — это не бросает исключение,
    // поэтому onError не вызывается. Логируем здесь.
    const body = response as Record<string, unknown> | null;
    const errMsg =
      status >= 400 && body?.error ? String(body.error) : undefined;

    log.res(
      request.method,
      url.pathname,
      status,
      ms,
      m.id,
      errMsg ? { error: errMsg } : undefined,
    );

    if (status >= 500) {
      log.error(`${request.method} ${url.pathname}`, undefined, {
        status,
        error: errMsg,
      });
    } else if (status >= 400) {
      log.warn(`${request.method} ${url.pathname}`, { status, error: errMsg });
    }
  })
  .onError(({ request, error, code, set }) => {
    const url = new URL(request.url);
    const isApiRoute = url.pathname.startsWith("/api");

    // NOT_FOUND на не-API маршруте — браузер или тулза зашла на localhost:3000 напрямую.
    // Не засоряем лог, просто отвечаем понятно.
    // if (code === "NOT_FOUND" && !isApiRoute) {
    //   set.status = 404;
    //   return "readd API server. Frontend: http://localhost:5173";
    // }

    const m = reqMeta.get(request);
    const id = m?.id ?? "????";
    const ms = m ? Date.now() - m.start : 0;

    const status =
      code === "NOT_FOUND"
        ? 404
        : code === "VALIDATION"
          ? 400
          : code === "PARSE"
            ? 400
            : 500;

    set.status = status;
    const msg = error instanceof Error ? error.message : String(error);

    log.res(request.method, url.pathname, status, ms, id, { code });

    if (code !== "NOT_FOUND") {
      log.error(`${request.method} ${url.pathname}`, error, { code });
    }

    return { error: msg };
  })
  .use(cors())
  .get("/uploads/*", async ({ params, set }) => {
    const file = Bun.file(
      join(import.meta.dir, "../uploads", decodeURIComponent(params["*"])),
    );
    if (!(await file.exists())) {
      set.status = 404;
      return;
    }
    return file;
  })
  .get("/api/health", () => ({
    status: "ok",
    timestamp: new Date().toISOString(),
    started: start,
  }))
  .use(authRoutes)
  .use(booksRoutes)
  .use(libraryRoutes)
  .use(progressRoutes)
  // Продакшен: отдаём собранный фронтенд из dist/
  // В dev этот код не выполняется (папки dist/ нет)
  .get("/*", async ({ params, set }) => {
    const dist = join(import.meta.dir, "../dist");
    const reqPath = params["*"] || "index.html";
    let file = Bun.file(join(dist, reqPath));
    if (!(await file.exists())) file = Bun.file(join(dist, "index.html"));
    if (!(await file.exists())) { set.status = 404; return; }
    return file;
  })
  .listen({
    port: PORT,
    maxRequestBodySize: 2 * 1024 * 1024 * 1024, // 2 GB
  });

log.info("Server started", { port: PORT });
