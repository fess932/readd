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

export const player = $state({
  book: null as PlayerBook | null,
  chapterIdx: 0,
  positionSec: 0,
  playing: false,
  duration: 0,        // длительность текущей главы (от <audio>)
  currentTime: 0,     // текущее время (от <audio>)
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

// Позиции по каждой главе — реактивный объект чтобы UI обновлялся
const chapterPositions = $state<Record<string, number>>({});

export function setChapterPos(path: string, time: number) {
  chapterPositions[path] = time;
}

export function getChapterPos(path: string): number {
  return chapterPositions[path] ?? 0;
}

export function saveProgress() {
  const path = currentChapterPath();
  console.log("save progress", path, player.book);
  if (!player.book || !path) return;
  setChapterPos(path, player.currentTime);
  import('./api').then(({ api }) => {
    api.progress.save(player.book!.id, {
      chapterPath: path,
      positionSec: player.currentTime,
      chapterDuration: player.duration || undefined,
    }).catch((err) => {
      console.error("save progress error", err);
    }).finally(() => {
      console.log("save progress done");
    });
  });
}
