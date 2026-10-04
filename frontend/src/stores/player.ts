import { reactive } from 'vue';
import { api, type Chapter, type Progress } from '../api';

export interface PlayerBook {
  id: number;
  title: string;
  author: string;
  coverPath?: string | null;
  chapters: Chapter[];
}

export const player = reactive({
  book: null as PlayerBook | null,
  chapterIdx: 0,
  /** Where to seek once the chapter's audio has loaded. */
  positionSec: 0,
  playing: false,
  duration: 0,
  currentTime: 0,
  speed: Number(localStorage.getItem('readd_speed')) || 1,
});

/**
 * A chapter counts as listened to when this close to its end: a few seconds,
 * but never more than a tenth of the chapter, so that short ones can be resumed too.
 */
const FINISHED_MARGIN_SEC = 6;

export function isChapterFinished(positionSec: number, durationSec: number | null | undefined): boolean {
  if (!durationSec) return false;
  return positionSec >= durationSec - Math.min(FINISHED_MARGIN_SEC, durationSec / 10);
}

/** Switches to a chapter of the current book and starts playing it. */
export function openChapter(chapterIdx: number, positionSec = 0) {
  player.chapterIdx = chapterIdx;
  player.positionSec = positionSec;
  // Show the target right away, not the numbers of whatever was playing before
  player.currentTime = positionSec;
  player.duration = player.book?.chapters[chapterIdx]?.durationSec ?? 0;
  player.playing = true;
}

export function playBook(book: PlayerBook, chapterIdx = 0, positionSec = 0) {
  const sameChapter = player.book?.id === book.id && player.chapterIdx === chapterIdx;
  player.book = book;
  if (sameChapter) {
    // Already loaded: the audio element keeps its own position
    player.playing = true;
    return;
  }
  openChapter(chapterIdx, positionSec);
}

/**
 * Opens a book where the user left off. Positions saved in this session win over
 * `book.progress`, which is only as fresh as the last fetch of the library.
 */
export function resumeBook(book: PlayerBook & { progress?: Progress | null }, autoplay = true) {
  if (player.book?.id === book.id) return;

  const last = lastPosition(book.id) ?? book.progress;
  const chapterIdx = Math.max(0, last ? book.chapters.findIndex(c => c.filePath === last.chapterPath) : 0);
  const chapter = book.chapters[chapterIdx];
  const positionSec = last && !isChapterFinished(last.positionSec, chapter?.durationSec) ? last.positionSec : 0;

  playBook(book, chapterIdx, positionSec);
  if (!autoplay) player.playing = false;
}

export function currentChapterPath(): string | null {
  if (!player.book) return null;
  return player.book.chapters[player.chapterIdx]?.filePath ?? null;
}

/** Unloads the book; the Player component stops the audio when it sees no chapter. */
export function stopPlayer() {
  player.book = null;
  player.chapterIdx = 0;
  player.positionSec = 0;
  player.playing = false;
  player.duration = 0;
  player.currentTime = 0;
}

// ── Positions known in this session ──────────────────────────────────────────

const chapterPositions = reactive<Record<string, number>>({});
const lastChapterOfBook: Record<number, string> = {};

export function setChapterPos(path: string, time: number) {
  chapterPositions[path] = time;
}

export function getChapterPos(path: string): number {
  return chapterPositions[path] ?? 0;
}

/** Position of a chapter, if this session has seen one (reactive). */
export function knownChapterPos(path: string): number | undefined {
  return chapterPositions[path];
}

function lastPosition(bookId: number): { chapterPath: string; positionSec: number } | undefined {
  const chapterPath = lastChapterOfBook[bookId];
  if (!chapterPath) return undefined;
  return { chapterPath, positionSec: getChapterPos(chapterPath) };
}

/** Forgets everything about the current user's listening. */
export function resetPlayer() {
  stopPlayer();
  for (const path of Object.keys(chapterPositions)) delete chapterPositions[path];
  for (const id of Object.keys(lastChapterOfBook)) delete lastChapterOfBook[Number(id)];
}

// ── Saving progress ──────────────────────────────────────────────────────────

const SAVE_INTERVAL_MS = 2000;
let lastSaveTime = 0;

/** Saves the current position; at most once per interval unless forced. */
export function saveProgress(force = false, overrideTime?: number) {
  const now = Date.now();
  if (!force && now - lastSaveTime < SAVE_INTERVAL_MS) return;
  const path = currentChapterPath();
  if (!player.book || !path) return;
  lastSaveTime = now;

  const time = overrideTime ?? player.currentTime;
  const bookId = player.book.id;
  setChapterPos(path, time);
  lastChapterOfBook[bookId] = path;

  api.progress
    .save(bookId, { chapterPath: path, positionSec: time, chapterDuration: player.duration || undefined })
    .catch(console.error);
}
