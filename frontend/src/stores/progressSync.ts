// Delivery of listening positions to the server, surviving a lost connection or a closed tab.
//
// Every position is written to localStorage first and removed once the server has it.
// Whatever is left over is sent again when the network returns and when the app starts.
// The server keeps the position that was *listened to* last (see `listenedAt`), so a
// late delivery from a device that was offline cannot overwrite newer listening elsewhere.

import { api, ApiError, type LatestPosition } from '../api';
import { auth } from './auth';

interface PendingSave {
  userId: number;
  bookId: number;
  chapterPath: string;
  positionSec: number;
  chapterDuration?: number;
  /** When the listener was at this position, unix ms. */
  listenedAt: number;
}

const STORAGE_KEY = 'readd_outbox';

/** One pending save per chapter: a newer position replaces the older one. */
const keyOf = (s: PendingSave) => `${s.userId}:${s.bookId}:${s.chapterPath}`;

function loadOutbox(): Record<string, PendingSave> {
  try {
    return JSON.parse(localStorage.getItem(STORAGE_KEY) ?? '{}');
  } catch {
    return {};
  }
}

function storeOutbox(outbox: Record<string, PendingSave>) {
  localStorage.setItem(STORAGE_KEY, JSON.stringify(outbox));
}

/** Drops `save` from the outbox, unless a newer position has replaced it meanwhile. */
function forget(save: PendingSave) {
  const outbox = loadOutbox();
  if (outbox[keyOf(save)]?.listenedAt !== save.listenedAt) return;
  delete outbox[keyOf(save)];
  storeOutbox(outbox);
}

type ServerAheadHandler = (bookId: number, latest: LatestPosition) => void;
let onServerAhead: ServerAheadHandler | null = null;

/** Called when the server knows of listening newer than what this device just reported. */
export function setServerAheadHandler(handler: ServerAheadHandler) {
  onServerAhead = handler;
}

async function send(save: PendingSave): Promise<void> {
  try {
    const { latest } = await api.progress.save(save.bookId, {
      chapterPath: save.chapterPath,
      positionSec: save.positionSec,
      chapterDuration: save.chapterDuration,
      listenedAt: save.listenedAt,
    });
    forget(save);
    if (latest.listenedAt > save.listenedAt) onServerAhead?.(save.bookId, latest);
  } catch (err) {
    // The book or chapter is gone: retrying will never help. Anything else
    // (no network, server down, expired login) stays queued for the next attempt.
    const rejected = err instanceof ApiError && err.status >= 400 && err.status < 500 && err.status !== 401;
    if (rejected) forget(save);
  }
}

/** Records a position and tries to deliver it right away. */
export function queueSave(save: Omit<PendingSave, 'userId' | 'listenedAt'>) {
  if (!auth.user) return;
  const pending: PendingSave = { ...save, userId: auth.user.id, listenedAt: Date.now() };
  const outbox = loadOutbox();
  outbox[keyOf(pending)] = pending;
  storeOutbox(outbox);
  void send(pending);
}

/**
 * Delivers the current user's undelivered positions. Await it before reading progress
 * from the server, so that what this device listened to offline is taken into account.
 */
export async function flushOutbox(): Promise<void> {
  const userId = auth.user?.id;
  const mine = Object.values(loadOutbox()).filter(s => s.userId === userId);
  await Promise.all(mine.map(send));
}

window.addEventListener('online', () => void flushOutbox());
