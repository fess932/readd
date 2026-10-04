/** Russian plural form: plural(21, 'глава', 'главы', 'глав') → 'глава'. */
export function plural(n: number, one: string, few: string, many: string): string {
  const mod10 = n % 10;
  const mod100 = n % 100;
  if (mod10 === 1 && mod100 !== 11) return one;
  if (mod10 >= 2 && mod10 <= 4 && (mod100 < 12 || mod100 > 14)) return few;
  return many;
}

export const pluralChapters = (n: number) => plural(n, 'глава', 'главы', 'глав');

/** Position on a timeline: 1:02:03 or 2:03. */
export function formatClock(sec: number | null | undefined): string {
  if (!sec || !isFinite(sec) || sec < 0) return '0:00';
  const h = Math.floor(sec / 3600);
  const m = Math.floor((sec % 3600) / 60);
  const s = Math.floor(sec % 60);
  const ss = String(s).padStart(2, '0');
  return h > 0 ? `${h}:${String(m).padStart(2, '0')}:${ss}` : `${m}:${ss}`;
}

/** Length in words: "3 ч 5 мин". Null when unknown. */
export function formatDuration(sec: number | null | undefined): string | null {
  if (!sec) return null;
  const h = Math.floor(sec / 3600);
  const m = Math.floor((sec % 3600) / 60);
  return h > 0 ? `${h} ч ${m} мин` : `${m} мин`;
}

export function formatSize(bytes: number): string {
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(0)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

export function coverUrl(coverPath: string | null | undefined): string {
  return coverPath ? `/uploads/${coverPath}` : '/placeholder.jpg';
}

/** Groups items by author, authors sorted alphabetically. */
export function groupByAuthor<T extends { author: string }>(items: T[]): Array<{ author: string; items: T[] }> {
  const map = new Map<string, T[]>();
  for (const item of items) {
    if (!map.has(item.author)) map.set(item.author, []);
    map.get(item.author)!.push(item);
  }
  return [...map.entries()]
    .map(([author, items]) => ({ author, items }))
    .sort((a, b) => a.author.localeCompare(b.author, 'ru'));
}
