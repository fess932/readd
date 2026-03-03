import Elysia, { t } from 'elysia';
import { eq, and, desc } from 'drizzle-orm';
import { db } from '../db';
import { progress, books, chapters } from '../db/schema';
import { authMiddleware } from '../middleware/auth';

export const progressRoutes = new Elysia({ prefix: '/api/progress' })
  .use(authMiddleware)

  // Последняя прослушиваемая книга (для восстановления при входе)
  .get('/last', ({ user, set }) => {
    if (!user) { set.status = 401; return { error: 'Unauthorized' }; }

    const row = db
      .select({
        bookId: progress.bookId,
        chapterPath: progress.chapterPath,
        positionSec: progress.positionSec,
        title: books.title,
        author: books.author,
        coverPath: books.coverPath,
        updatedAt: progress.updatedAt,
      })
      .from(progress)
      .innerJoin(books, eq(progress.bookId, books.id))
      .where(eq(progress.userId, user.id))
      .orderBy(desc(progress.updatedAt))
      .get();

    return row ?? null;
  })

  // Прогресс всех глав книги
  .get('/:bookId', ({ params, user, set }) => {
    if (!user) { set.status = 401; return { error: 'Unauthorized' }; }

    return db
      .select({ chapterPath: progress.chapterPath, positionSec: progress.positionSec })
      .from(progress)
      .where(and(eq(progress.userId, user.id), eq(progress.bookId, Number(params.bookId))))
      .all();
  })

  // Сохранить позицию конкретной главы
  .post('/:bookId', ({ params, body, user, set }) => {
    if (!user) { set.status = 401; return { error: 'Unauthorized' }; }

    const bookId = Number(params.bookId);
    const { chapterPath, positionSec, chapterDuration } = body;
    const now = new Date().toISOString().replace('T', ' ').slice(0, 19);

    const existing = db.select().from(progress)
      .where(and(eq(progress.userId, user.id), eq(progress.bookId, bookId), eq(progress.chapterPath, chapterPath))).get();

    if (existing) {
      db.update(progress)
        .set({ positionSec, updatedAt: now })
        .where(and(eq(progress.userId, user.id), eq(progress.bookId, bookId), eq(progress.chapterPath, chapterPath)))
        .run();
    } else {
      db.insert(progress).values({ userId: user.id, bookId, chapterPath, positionSec }).run();
    }

    // Обновляем длительность главы если аудио её сообщило
    if (chapterDuration && chapterDuration > 0) {
      db.update(chapters)
        .set({ durationSec: chapterDuration })
        .where(eq(chapters.filePath, chapterPath))
        .run();
    }

    return { ok: true };
  }, {
    body: t.Object({
      chapterPath: t.String(),
      positionSec: t.Number(),
      chapterDuration: t.Optional(t.Number()),
    }),
  });
