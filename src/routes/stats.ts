import Elysia from 'elysia';
import { eq, desc, sql, count } from 'drizzle-orm';
import { db } from '../db';
import { books, users, userLibrary, chapters, progress } from '../db/schema';
import { authMiddleware } from '../middleware/auth';

export const statsRoutes = new Elysia({ prefix: '/api/stats' })
  .use(authMiddleware)
  .get('/', ({ user, set }) => {
    if (!user) { set.status = 401; return { error: 'Unauthorized' }; }

    // --- Global ---
    const [{ totalBooks }] = db.select({ totalBooks: count() }).from(books).all();
    const [{ totalUsers }] = db.select({ totalUsers: count() }).from(users).all();
    const [{ totalSec }] = db
      .select({ totalSec: sql<number>`COALESCE(SUM(${chapters.durationSec}), 0)` })
      .from(chapters)
      .all();

    const topBooks = db
      .select({
        id: books.id,
        title: books.title,
        author: books.author,
        coverPath: books.coverPath,
        libraryCount: sql<number>`COUNT(${userLibrary.userId})`,
      })
      .from(books)
      .leftJoin(userLibrary, eq(books.id, userLibrary.bookId))
      .groupBy(books.id)
      .orderBy(desc(sql`COUNT(${userLibrary.userId})`))
      .limit(5)
      .all();

    const uploaders = db
      .select({
        name: users.name,
        booksCount: count(books.id),
      })
      .from(users)
      .innerJoin(books, eq(books.uploadedById, users.id))
      .groupBy(users.id)
      .orderBy(desc(count(books.id)))
      .all();

    // --- Personal ---
    const [{ myBooksCount }] = db
      .select({ myBooksCount: count() })
      .from(userLibrary)
      .where(eq(userLibrary.userId, user.id))
      .all();

    const [{ myListenedSec }] = db
      .select({ myListenedSec: sql<number>`COALESCE(SUM(${progress.positionSec}), 0)` })
      .from(progress)
      .where(eq(progress.userId, user.id))
      .all();

    const favoriteAuthorRow = db
      .select({
        author: books.author,
        cnt: sql<number>`COUNT(*)`,
      })
      .from(userLibrary)
      .innerJoin(books, eq(userLibrary.bookId, books.id))
      .where(eq(userLibrary.userId, user.id))
      .groupBy(books.author)
      .orderBy(desc(sql`COUNT(*)`))
      .limit(1)
      .get();

    return {
      personal: {
        booksInLibrary: myBooksCount,
        listenedSec: myListenedSec,
        favoriteAuthor: favoriteAuthorRow?.author ?? null,
      },
      global: {
        totalBooks,
        totalUsers,
        totalSec,
        topBooks,
        uploaders,
      },
    };
  });
