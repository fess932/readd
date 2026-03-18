import { join } from "path";

const root = import.meta.dir;

// 1. Apply pending migrations
const migrate = Bun.spawnSync(["bun", "run", join(root, "migrate.ts")], {
  stdout: "inherit",
  stderr: "inherit",
});
if (migrate.exitCode !== 0) {
  process.stderr.write("\n\x1b[31m[build] Migrations failed.\x1b[0m\n\n");
  process.exit(1);
}

// 2. Build frontend
const frontend = Bun.spawnSync(["bun", "run", "build"], {
  stdout: "inherit",
  stderr: "inherit",
  cwd: join(root, "src/frontend"),
});
if (frontend.exitCode !== 0) {
  process.stderr.write("\n\x1b[31m[build] Frontend build failed.\x1b[0m\n\n");
  process.exit(1);
}

// 4. Build Rust release binary
const server = Bun.spawnSync(
  [
    "cargo",
    "build",
    "--release",
    "--manifest-path",
    join(root, "server/Cargo.toml"),
  ],
  {
    stdout: "inherit",
    stderr: "inherit",
  },
);
if (server.exitCode !== 0) {
  process.stderr.write("\n\x1b[31m[build] Rust build failed.\x1b[0m\n\n");
  process.exit(1);
}

process.stdout.write("\x1b[32m[build] Done.\x1b[0m\n");
