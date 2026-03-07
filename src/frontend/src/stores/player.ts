import { reactive } from 'vue';

export interface Chapter {
  id: number;
  filePath: string;
  sortOrder: number;
  durationSec: number | null;
}

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
  positionSec: 0,
  playing: false,
  duration: 0,
  currentTime: 0,
});

export function playBook(book: PlayerBook, chapterIdx = 0, positionSec = 0) {
  player.book = book;
  player.chapterIdx = chapterIdx;
  player.positionSec = positionSec;
  player.playing = true;
}

export function currentChapterPath(): string | null {
  if (!player.book) return null;
  return player.book.chapters[player.chapterIdx]?.filePath ?? null;
}

const chapterPositions = reactive<Record<string, number>>({});

export function setChapterPos(path: string, time: number) {
  chapterPositions[path] = time;
}

export function getChapterPos(path: string): number {
  return chapterPositions[path] ?? 0;
}

let lastSaveTime = 0;

export function saveProgress(force = false, overrideTime?: number) {
  const now = Date.now();
  if (!force && now - lastSaveTime < 2000) return;
  lastSaveTime = now;
  const path = currentChapterPath();
  if (!player.book || !path) return;
  // Capture eagerly; fall back to last known in-memory pos if audio hasn't loaded yet (currentTime = 0)
  const time = overrideTime ?? (player.currentTime || getChapterPos(path));
  const bookId = player.book.id;
  const duration = player.duration;
  setChapterPos(path, time);
  import('../api').then(({ api }) => {
    api.progress.save(bookId, {
      chapterPath: path,
      positionSec: time,
      chapterDuration: duration || undefined,
    }).catch(console.error);
  });
}
