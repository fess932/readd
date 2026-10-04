import { join } from "path";

const root = import.meta.dir;

function run(step: string, cmd: string[], cwd = root) {
  const result = Bun.spawnSync(cmd, { stdout: "inherit", stderr: "inherit", cwd });
  if (result.exitCode !== 0) {
    process.stderr.write(`\n\x1b[31m[build] ${step} failed.\x1b[0m\n\n`);
    process.exit(1);
  }
}

// The server applies database migrations itself on startup.
run("Frontend build", ["bun", "run", "build"], join(root, "frontend"));
run("Rust build", ["cargo", "build", "--release", "--manifest-path", join(root, "server/Cargo.toml")]);

process.stdout.write("\x1b[32m[build] Done.\x1b[0m\n");
