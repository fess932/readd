import Elysia, { t } from 'elysia';
import { eq, desc, sql } from 'drizzle-orm';
import { parseFile } from 'music-metadata';
import { join, basename, extname } from 'path';
import { mkdir, rm } from 'fs/promises';
import { createWriteStream } from 'fs';
import busboy from 'busboy';
import { db } from '../db';
import { books, users, chapters, userLibrary, progress } from '../db/schema';
import { authMiddleware } from '../middleware/auth';
import { log, formatBytes } from '../logger';

const UPLOADS_DIR = join(import.meta.dir, '../../uploads');

const AUDIO_EXT = /\.(mp3|m4a|m4b|ogg|flac|wav|aac|opus)$/i;
const IMAGE_EXT = /\.(jpg|jpeg|png|webp|avif)$/i;
const COVER_NAME = /^(cover|folder|front|artwork|thumb)/i;

const CYR: Record<string, string> = {
  а:'a',б:'b',в:'v',г:'g',д:'d',е:'e',ё:'yo',ж:'zh',з:'z',и:'i',й:'y',
  к:'k',л:'l',м:'m',н:'n',о:'o',п:'p',р:'r',с:'s',т:'t',у:'u',ф:'f',
  х:'kh',ц:'ts',ч:'ch',ш:'sh',щ:'shch',ъ:'',ы:'y',ь:'',э:'e',ю:'yu',я:'ya',
};

function sanitizeFilename(raw: string): string {
  const name = basename(raw);           // убираем вложенный путь
  const ext  = extname(name).toLowerCase();
  const stem = name.slice(0, name.length - ext.length);

  const latin = stem
    .toLowerCase()
    .split('')
    .map(c => CYR[c] ?? c)
    .join('')
    .replace(/[^a-z0-9]+/g, '-')        // всё кроме латиницы/цифр → дефис
    .replace(/^-+|-+$/g, '');           // дефисы по краям

  return (latin || 'file') + ext;
}

function fingerprintFromMeta(files: { name: string; size: number }[]): string {
  const entries = files
    .filter(f => AUDIO_EXT.test(f.name))
    .map(f => `${sanitizeFilename(f.name)}:${f.size}`)
    .sort()
    .join('|');
  return Bun.hash(entries).toString(16);
}

function fingerprint(files: File[]): string {
  return fingerprintFromMeta(files.map(f => ({ name: f.name, size: f.size })));
}


function detectCover(files: File[]): File | undefined {
  const byName = (f: File) => COVER_NAME.test(basename(f.name));
  return (
    files.find(f => IMAGE_EXT.test(f.name) && byName(f)) ??
    files.find(f => IMAGE_EXT.test(f.name))
  );
}

function detectAudio(files: File[]): File[] {
  return files
    .filter(f => AUDIO_EXT.test(f.name))
    .sort((a, b) => a.name.localeCompare(b.name, undefined, { numeric: true }));
}

export const booksRoutes = new Elysia({ prefix: '/api/books' })
  .use(authMiddleware)

  // Preflight: проверить дубль по именам+размерам файлов, без загрузки
  .post('/check', ({ body, set }) => {
    const fp = fingerprintFromMeta(body.files);
    const duplicate = db.select({ id: books.id, title: books.title })
      .from(books).where(eq(books.fingerprint, fp)).get();
    if (duplicate) {
      set.status = 409;
      return { error: `Книга уже загружена: «${duplicate.title}»` };
    }
    return { ok: true, fingerprint: fp };
  }, {
    body: t.Object({
      files: t.Array(t.Object({ name: t.String(), size: t.Number() })),
    }),
  })

  .get('/', () => {
    return db
      .select({
        id: books.id,
        title: books.title,
        author: books.author,
        narrator: books.narrator,
        coverPath: books.coverPath,
        filePath: books.filePath,
        uploadedBy: users.name,
        createdAt: books.createdAt,
        chaptersCount: sql<number>`(select count(*) from chapters where chapters.book_id = ${books.id})`,
        totalSec:      sql<number | null>`(select sum(duration_sec) from chapters where chapters.book_id = ${books.id})`,
      })
      .from(books)
      .leftJoin(users, eq(books.uploadedById, users.id))
      .orderBy(desc(books.createdAt))
      .all();
  })
  .post('/', async ({ request, user, set }) => {
    if (!user) { set.status = 401; return { error: 'Unauthorized' }; }

    const dir = `book-${Date.now()}`;
    await mkdir(join(UPLOADS_DIR, dir), { recursive: true });

    // --- Стриминг multipart прямо на диск через busboy ---
    // request.formData() буферизует всё в RAM — для 1ГБ это проблема.
    // busboy пишет каждый файл потоково, пик RAM — несколько КБ на чанк.
    const fields: Record<string, string> = {};
    const saved: { relPath: string; name: string; size: number }[] = [];
    const usedNames = new Set<string>();

    await new Promise<void>((resolve, reject) => {
      const bb = busboy({ headers: Object.fromEntries(request.headers) });

      // Считаем открытые writeStream'ы — resolve только когда
      // busboy закончил парсинг И все файлы дозаписаны на диск.
      let pending = 0;
      let bbDone = false;

      function tryResolve() {
        if (bbDone && pending === 0) resolve();
      }

      bb.on('field', (name, val) => { fields[name] = val; });

      bb.on('file', (_field, stream, info) => {
        let safe = sanitizeFilename(info.filename);
        if (usedNames.has(safe)) {
          const ext = extname(safe);
          safe = `${safe.slice(0, -ext.length)}-${usedNames.size}${ext}`;
        }
        usedNames.add(safe);

        const absPath = join(UPLOADS_DIR, dir, safe);
        const ws = createWriteStream(absPath);
        let size = 0;
        pending++;

        stream.on('data', (chunk: Buffer) => { size += chunk.length; });
        stream.on('error', reject);
        ws.on('error', reject);
        ws.on('finish', () => {
          saved.push({ relPath: `${dir}/${safe}`, name: safe, size });
          pending--;
          tryResolve();
        });
        stream.pipe(ws);
      });

      bb.on('finish', () => { bbDone = true; tryResolve(); });
      bb.on('error', reject);

      // Bun ReadableStream → busboy
      const reader = request.body!.getReader();
      const pump = (): void => {
        reader.read().then(({ done, value }) => {
          if (done) { bb.end(); return; }
          bb.write(value);
          pump();
        }).catch(reject);
      };
      pump();
    });

    const title    = fields['title']    ?? null;
    const author   = fields['author']   ?? null;
    const narrator = fields['narrator'] ?? null;

    if (!title || !author) {
      await rm(join(UPLOADS_DIR, dir), { recursive: true, force: true });
      set.status = 400;
      return { error: 'title и author обязательны' };
    }

    const totalSize = saved.reduce((s, f) => s + f.size, 0);
    log.info('upload received', {
      user: user.name, title,
      files: saved.length, size: formatBytes(totalSize),
    });

    const audioSaved = saved
      .filter(f => AUDIO_EXT.test(f.name))
      .sort((a, b) => a.name.localeCompare(b.name, undefined, { numeric: true }));

    if (audioSaved.length === 0) {
      await rm(join(UPLOADS_DIR, dir), { recursive: true, force: true });
      log.warn('upload rejected: no audio', { user: user.name });
      set.status = 400;
      return { error: 'Аудиофайлы не найдены' };
    }

    const coverSaved =
      saved.find(f => IMAGE_EXT.test(f.name) && COVER_NAME.test(f.name)) ??
      saved.find(f => IMAGE_EXT.test(f.name));

    // Fingerprint по имени+размеру (без содержимого)
    const fp = fingerprintFromMeta(saved.map(f => ({ name: f.name, size: f.size })));
    const duplicate = db.select({ id: books.id, title: books.title })
      .from(books).where(eq(books.fingerprint, fp)).get();
    if (duplicate) {
      await rm(join(UPLOADS_DIR, dir), { recursive: true, force: true });
      log.warn('upload rejected: duplicate', { existing: duplicate.title });
      set.status = 409;
      return { error: `Книга уже загружена: «${duplicate.title}»` };
    }

    // Читаем длительности параллельно с таймаутом 4 сек на файл.
    // music-metadata на VBR MP3 сканирует весь файл — для 1ГБ это минуты.
    // Если не успел — duration останется null и заполнится при воспроизведении.
    const DURATION_TIMEOUT = 4000;
    const metaStart = Date.now();
    const durations = await Promise.all(
      audioSaved.map(f =>
        Promise.race([
          parseFile(join(UPLOADS_DIR, f.relPath), { duration: true })
            .then(m => m.format.duration ?? null)
            .catch(() => null),
          new Promise<null>(r => setTimeout(() => r(null), DURATION_TIMEOUT)),
        ])
      )
    );
    const timedOut = durations.filter(d => d === null).length;
    log.info('durations read', {
      metaMs: Date.now() - metaStart,
      ...(timedOut > 0 && { timedOut: `${timedOut} файлов — заполнятся при воспроизведении` }),
    });

    const [book] = db.insert(books).values({
      title, author, narrator: narrator || null,
      filePath: audioSaved[0].relPath,
      coverPath: coverSaved?.relPath ?? null,
      fingerprint: fp,
      uploadedById: user.id,
    }).returning().all();

    for (let i = 0; i < audioSaved.length; i++) {
      db.insert(chapters).values({
        bookId: book.id,
        filePath: audioSaved[i].relPath,
        sortOrder: i,
        durationSec: durations[i],
      }).run();
    }

    const totalSec = durations.reduce<number | null>(
      (sum, d) => d === null ? sum : (sum ?? 0) + d, null
    );

    log.info('book created', {
      id: book.id, title: book.title,
      chapters: audioSaved.length,
      duration: totalSec ? `${Math.round(totalSec / 60)} мин` : 'unknown',
    });
    return { ...book, chaptersCount: audioSaved.length, totalSec };
  }, {
    // Явно отключаем встроенный Elysia-парсер.
    // Без этого Elysia запускает multipart-парсер по Content-Type
    // и конкурирует с busboy за request.body stream — иногда выигрывает и вешает запрос.
    parse: async () => null,
  })
  .delete('/:id', async ({ params, user, set }) => {
    if (!user) {
      set.status = 401;
      return { error: 'Unauthorized' };
    }
    if (!user.isAdmin) {
      set.status = 403;
      return { error: 'Admin only' };
    }

    const id = Number(params.id);
    const existing = db.select().from(books).where(eq(books.id, id)).get();
    if (!existing) {
      set.status = 404;
      return { error: 'Book not found' };
    }

    db.delete(progress).where(eq(progress.bookId, id)).run();
    db.delete(userLibrary).where(eq(userLibrary.bookId, id)).run();
    db.delete(chapters).where(eq(chapters.bookId, id)).run();
    db.delete(books).where(eq(books.id, id)).run();

    // Удаляем папку с файлами книги
    const dir = existing.filePath.split('/')[0];
    if (dir) {
      const dirPath = join(UPLOADS_DIR, dir);
      await rm(dirPath, { recursive: true, force: true });
      log.info('book files removed', { dir: dirPath });
    }

    log.info('book deleted', { id, title: existing.title });
    return { ok: true };
  })

  // Пересчёт длительностей глав без duration (для старых книг)
  .post('/scan-durations', async ({ user, set }) => {
    if (!user?.isAdmin) { set.status = 403; return { error: 'Admin only' }; }

    const missing = db.select()
      .from(chapters)
      .where(sql`${chapters.durationSec} is null`)
      .all();

    log.info('scan-durations started', { count: missing.length });
    let updated = 0;

    await Promise.all(missing.map(async (ch) => {
      const path = join(UPLOADS_DIR, ch.filePath);
      try {
        const meta = await parseFile(path, { duration: true });
        const dur = meta.format.duration;
        if (dur) {
          db.update(chapters).set({ durationSec: dur }).where(eq(chapters.id, ch.id)).run();
          updated++;
        }
      } catch { /* файл недоступен */ }
    }));

    log.info('scan-durations done', { updated, skipped: missing.length - updated });
    return { updated, skipped: missing.length - updated };
  });
