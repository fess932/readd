import { Database } from "bun:sqlite";
import { drizzle } from "drizzle-orm/bun-sqlite";
import { join } from "path";
import * as schema from "./schema";

const sqlite = new Database(join(import.meta.dir, "../../readd.db"));
sqlite.run("PRAGMA journal_mode = WAL;");
sqlite.run("PRAGMA synchronous = NORMAL;");
sqlite.run("PRAGMA foreign_keys = ON;");
sqlite.run("PRAGMA busy_timeout = 5000;");
sqlite.run("PRAGMA cache_size = -64000;");
sqlite.run("PRAGMA temp_store = MEMORY;");

export const db = drizzle(sqlite, { schema });
