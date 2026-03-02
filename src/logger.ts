const R = '\x1b[0m';
const DIM = '\x1b[2m';
const BOLD = '\x1b[1m';

const C = {
  debug: '\x1b[36m',   // cyan
  info:  '\x1b[34m',   // blue
  warn:  '\x1b[33m',   // yellow
  error: '\x1b[31m',   // red
  http:  '\x1b[35m',   // magenta
  ok:    '\x1b[32m',   // green
};

function ts() {
  return `${DIM}${new Date().toTimeString().slice(0, 12)}${R}`;
}

function level(l: keyof typeof C) {
  return `${C[l]}${BOLD}${l.toUpperCase().padEnd(5)}${R}`;
}

function meta(obj: Record<string, unknown>) {
  return Object.entries(obj)
    .filter(([, v]) => v !== undefined && v !== null && v !== '')
    .map(([k, v]) => `${DIM}${k}=${R}${v}`)
    .join('  ');
}

export function formatBytes(n: number): string {
  if (n < 1024) return `${n} B`;
  if (n < 1024 ** 2) return `${(n / 1024).toFixed(1)} KB`;
  return `${(n / 1024 ** 2).toFixed(1)} MB`;
}

export const log = {
  debug: (msg: string, fields?: Record<string, unknown>) => {
    if (!process.env.DEBUG) return;
    process.stdout.write(`${ts()}  ${level('debug')}  ${msg}${fields ? '  ' + meta(fields) : ''}\n`);
  },

  info: (msg: string, fields?: Record<string, unknown>) => {
    process.stdout.write(`${ts()}  ${level('info')}  ${msg}${fields ? '  ' + meta(fields) : ''}\n`);
  },

  warn: (msg: string, fields?: Record<string, unknown>) => {
    process.stderr.write(`${ts()}  ${level('warn')}  ${msg}${fields ? '  ' + meta(fields) : ''}\n`);
  },

  error: (msg: string, err?: unknown, fields?: Record<string, unknown>) => {
    const e = err instanceof Error ? err : undefined;
    const stack = e?.stack?.split('\n').slice(1, 5).map(l => `         ${DIM}${l.trim()}${R}`).join('\n');
    const f = { ...(e ? { err: e.message } : {}), ...fields };
    process.stderr.write(
      `${ts()}  ${level('error')}  ${msg}${Object.keys(f).length ? '  ' + meta(f) : ''}\n` +
      (stack ? stack + '\n' : '')
    );
  },

  // HTTP лог: ← входящий, → исходящий
  req: (method: string, path: string, id: string, extra?: Record<string, unknown>) => {
    const line = `${ts()}  ${C.http}←${R}  ${BOLD}${method.padEnd(6)}${R} ${path}  ${DIM}#${id}${R}`;
    process.stdout.write(line + (extra ? '  ' + meta(extra) : '') + '\n');
  },

  res: (method: string, path: string, status: number, ms: number, id: string, extra?: Record<string, unknown>) => {
    const color = status < 300 ? C.ok : status < 400 ? C.warn : C.error;
    const line = `${ts()}  ${C.http}→${R}  ${BOLD}${method.padEnd(6)}${R} ${path}  ${color}${BOLD}${status}${R}  ${DIM}${ms}ms${R}  ${DIM}#${id}${R}`;
    process.stdout.write(line + (extra ? '  ' + meta(extra) : '') + '\n');
  },
};

// Генератор коротких ID для запросов
export function reqId() {
  return Math.random().toString(36).slice(2, 6).toUpperCase();
}
