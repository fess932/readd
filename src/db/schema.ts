import { sqliteTable, text, integer, real, primaryKey } from 'drizzle-orm/sqlite-core';
import { sql } from 'drizzle-orm';

export const users = sqliteTable('users', {
  id: integer('id').primaryKey({ autoIncrement: true }),
  name: text('name').notNull().unique(),
  isAdmin: integer('is_admin', { mode: 'boolean' }).notNull().default(false),
  createdAt: text('created_at').notNull().default(sql`(datetime('now'))`),
});

export const books = sqliteTable('books', {
  id: integer('id').primaryKey({ autoIncrement: true }),
  title: text('title').notNull(),
  author: text('author').notNull(),
  narrator: text('narrator'),
  coverPath: text('cover_path'),
  filePath: text('file_path').notNull(),
  durationSec: integer('duration_sec'),
  fingerprint: text('fingerprint').unique(),
  uploadedById: integer('uploaded_by_id').notNull().references(() => users.id),
  createdAt: text('created_at').notNull().default(sql`(datetime('now'))`),
});

export const chapters = sqliteTable('chapters', {
  id: integer('id').primaryKey({ autoIncrement: true }),
  bookId: integer('book_id').notNull().references(() => books.id),
  filePath: text('file_path').notNull(),
  sortOrder: integer('sort_order').notNull(),
  durationSec: real('duration_sec'),
});

export const userLibrary = sqliteTable('user_library', {
  userId: integer('user_id').notNull().references(() => users.id),
  bookId: integer('book_id').notNull().references(() => books.id),
  addedAt: text('added_at').notNull().default(sql`(datetime('now'))`),
}, (t) => [primaryKey({ columns: [t.userId, t.bookId] })]);

export const progress = sqliteTable('progress', {
  userId: integer('user_id').notNull().references(() => users.id),
  bookId: integer('book_id').notNull().references(() => books.id),
  chapterPath: text('chapter_path').notNull(),
  positionSec: real('position_sec').notNull().default(0),
  updatedAt: text('updated_at').notNull().default(sql`(datetime('now'))`),
}, (t) => [primaryKey({ columns: [t.userId, t.bookId, t.chapterPath] })]);
