import { join } from "path";

const root = import.meta.dir;

const backend = Bun.spawn(["bun", "src/index.ts"], {
  stdout: "inherit",
  stderr: "inherit",
  cwd: root,
});

// Если бэкенд упал — показываем понятную ошибку и выходим
backend.exited.then((code) => {
  if (code !== 0) {
    process.stderr.write(
      `\n\x1b[31m[dev] Бэкенд упал (exit ${code}).\x1b[0m\n` +
      `\x1b[33m      Если порт 3000 занят: kill \$(lsof -ti :3000)\x1b[0m\n\n`
    );
    frontend.kill();
    process.exit(1);
  }
});

const frontend = Bun.spawn(["bun", join(root, "node_modules/vite/bin/vite.js")], {
  stdout: "inherit",
  stderr: "inherit",
  cwd: join(root, "src/frontend"),
});

setTimeout(() => Bun.spawn(["open", "http://localhost:5173"]), 2000);

function shutdown() {
  backend.kill();
  frontend.kill();
  process.exit(0);
}

process.on("SIGINT", shutdown);
process.on("SIGTERM", shutdown);

await Promise.all([backend.exited, frontend.exited]);
