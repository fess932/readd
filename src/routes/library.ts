import Elysia from 'elysia';
import { eq, and, desc, asc } from 'drizzle-orm';
import { db } from '../db';
import { books, users, userLibrary, chapters, progress } from '../db/schema';
import { authMiddleware } from '../middleware/auth';

export const libraryRoutes = new Elysia({ prefix: '/api/library' })
  .use(authMiddleware)
  .get('/', ({ user, set }) => {
    if (!user) { set.status = 401; return { error: 'Unauthorized' }; }

    const myBooks = db
      .select({
        id: books.id,
        title: books.title,
        author: books.author,
        narrator: books.narrator,
        coverPath: books.coverPath,
        filePath: books.filePath,
        uploadedBy: users.name,
        addedAt: userLibrary.addedAt,
      })
      .from(userLibrary)
      .innerJoin(books, eq(userLibrary.bookId, books.id))
      .leftJoin(users, eq(books.uploadedById, users.id))
      .where(eq(userLibrary.userId, user.id))
      .orderBy(desc(userLibrary.addedAt))
      .all();

    return myBooks.map(book => {
      const bookChapters = db
        .select({ id: chapters.id, filePath: chapters.filePath, sortOrder: chapters.sortOrder, durationSec: chapters.durationSec })
        .from(chapters)
        .where(eq(chapters.bookId, book.id))
        .orderBy(asc(chapters.sortOrder))
        .all();

      const prog = db.select()
        .from(progress)
        .where(and(eq(progress.userId, user.id), eq(progress.bookId, book.id)))
        .orderBy(desc(progress.updatedAt))
        .get() ?? null;

      return { ...book, chapters: bookChapters, progress: prog };
    });
  })
  .post('/:bookId', ({ params, user, set }) => {
    if (!user) { set.status = 401; return { error: 'Unauthorized' }; }

    const bookId = Number(params.bookId);
    const existing = db.select().from(books).where(eq(books.id, bookId)).get();
    if (!existing) { set.status = 404; return { error: 'Book not found' }; }

    const alreadyAdded = db.select().from(userLibrary)
      .where(and(eq(userLibrary.userId, user.id), eq(userLibrary.bookId, bookId))).get();
    if (!alreadyAdded) {
      db.insert(userLibrary).values({ userId: user.id, bookId }).run();
    }
    return { ok: true };
  })
  .get('/:bookId', ({ params, user, set }) => {
    if (!user) { set.status = 401; return { error: 'Unauthorized' }; }

    const bookId = Number(params.bookId);
    const row = db.select({
      id: books.id, title: books.title, author: books.author,
      narrator: books.narrator, coverPath: books.coverPath,
      filePath: books.filePath, uploadedBy: users.name, addedAt: userLibrary.addedAt,
    })
      .from(userLibrary)
      .innerJoin(books, eq(userLibrary.bookId, books.id))
      .leftJoin(users, eq(books.uploadedById, users.id))
      .where(and(eq(userLibrary.userId, user.id), eq(userLibrary.bookId, bookId)))
      .get();

    if (!row) { set.status = 404; return { error: 'Not found' }; }

    const bookChapters = db
      .select({ id: chapters.id, filePath: chapters.filePath, sortOrder: chapters.sortOrder, durationSec: chapters.durationSec })
      .from(chapters).where(eq(chapters.bookId, bookId)).orderBy(asc(chapters.sortOrder)).all();

    const prog = db.select().from(progress)
      .where(and(eq(progress.userId, user.id), eq(progress.bookId, bookId)))
      .orderBy(desc(progress.updatedAt))
      .get() ?? null;

    return { ...row, chapters: bookChapters, progress: prog };
  })
  .delete('/:bookId', ({ params, user, set }) => {
    if (!user) { set.status = 401; return { error: 'Unauthorized' }; }

    const bookId = Number(params.bookId);
    db.delete(userLibrary)
      .where(and(eq(userLibrary.userId, user.id), eq(userLibrary.bookId, bookId)))
      .run();
    return { ok: true };
  });
