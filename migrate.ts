import { Database } from "bun:sqlite";
import { readdirSync, readFileSync } from "fs";
import { join } from "path";

const dbPath = (process.env.DATABASE_URL ?? "readd.db").replace(/^sqlite:/, "");
const db = new Database(dbPath);

db.run(`CREATE TABLE IF NOT EXISTS _migrations (
  name TEXT PRIMARY KEY,
  applied_at TEXT NOT NULL DEFAULT (datetime('now'))
)`);

const dir = join(import.meta.dir, "server/migrations");
const files = readdirSync(dir).filter(f => f.endsWith(".sql")).sort();

for (const file of files) {
  if (db.query("SELECT 1 FROM _migrations WHERE name = ?").get(file)) continue;
  const sql = readFileSync(join(dir, file), "utf8");
  db.run(sql);
  db.run("INSERT INTO _migrations (name) VALUES (?)", [file]);
  console.log(`[migrate] applied: ${file}`);
}

console.log("[migrate] done");
