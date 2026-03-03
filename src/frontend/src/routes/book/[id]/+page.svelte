<script lang="ts">
  import { onMount } from 'svelte';
  import { page } from '$app/stores';
  import { goto } from '$app/navigation';
  import { api, type LibraryBook, type Chapter, type ChapterProgress } from '$lib/api';
  import { player, playBook, saveProgress, getChapterPos, setChapterPos } from '$lib/playerState.svelte';
  import { toast } from '$lib/toast.svelte';

  let book = $state<LibraryBook | null>(null);
  let loading = $state(true);
  let error = $state('');

  const bookId = $derived(Number($page.params.id));

  function fmt(s: number | null | undefined): string {
    if (!s) return '';
    const h = Math.floor(s / 3600);
    const m = Math.floor((s % 3600) / 60);
    const sec = Math.floor(s % 60);
    return h > 0
      ? `${h}:${String(m).padStart(2, '0')}:${String(sec).padStart(2, '0')}`
      : `${m}:${String(sec).padStart(2, '0')}`;
  }

  function chapterName(ch: Chapter, idx: number): string {
    const raw = ch.filePath.split('/').pop() ?? '';
    const name = raw.replace(/\.[^.]+$/, '').replace(/^[\d\s._-]+/, '').trim();
    return name || `Глава ${idx + 1}`;
  }

  // Индекс главы где сохранён прогресс
  function progressChapterIdx(b: LibraryBook): number {
    if (!b.progress) return -1;
    return b.chapters.findIndex(c => c.filePath === b.progress!.chapterPath);
  }

  // Прогресс главы (0..1)
  function chapterProgress(b: LibraryBook, ch: Chapter, idx: number): number {
    const progIdx = progressChapterIdx(b);
    if (progIdx < 0) return 0;
    if (idx < progIdx) return 1; // уже прослушана
    if (idx > progIdx) return 0; // ещё не начата
    // Текущая глава
    const dur = ch.durationSec;
    if (!dur) return 0;
    return Math.min(1, (b.progress!.positionSec) / dur);
  }

  // Для текущей главы — живое время, для остальных — позиция из карты
  function liveChapterProgress(b: LibraryBook, ch: Chapter, idx: number): number {
    if (player.book?.id === b.id && player.chapterIdx === idx && player.duration > 0) {
      return Math.min(1, player.currentTime / player.duration);
    }
    const pos = getChapterPos(ch.filePath);
    if (pos > 0 && ch.durationSec) return Math.min(1, pos / ch.durationSec);
    return 0;
  }

  function playChapter(idx: number) {
    if (!book) return;
    saveProgress();
    const ch = book.chapters[idx];
    playBook(book, idx, getChapterPos(ch.filePath));
  }

  function isActive(idx: number): boolean {
    return player.book?.id === bookId && player.chapterIdx === idx;
  }

  onMount(async () => {
    try {
      const [bookData, positions] = await Promise.all([
        api.library.get(bookId),
        api.progress.get(bookId),
      ]);
      book = bookData;
      for (const p of positions) setChapterPos(p.chapterPath, p.positionSec);
      // Загружаем книгу в плеер (без автоплея) если ещё не там
      if (player.book?.id !== bookId) {
        const chapterIdx = book.progress
          ? book.chapters.findIndex(c => c.filePath === book.progress!.chapterPath)
          : 0;
        playBook(book, Math.max(0, chapterIdx), book.progress?.positionSec ?? 0);
        player.playing = false;
      }
    } catch (e: any) {
      if (e.message?.includes('Not found')) goto('/library');
      else error = e.message;
    } finally {
      loading = false;
    }
  });
</script>

<main class="page">
  {#if loading}
    <p class="hint">Загрузка...</p>
  {:else if error}
    <p class="err">{error}</p>
  {:else if book}
    <div class="book-header">
      {#if book.coverPath}
        <img src="/uploads/{book.coverPath}" alt={book.title} class="cover" />
      {:else}
        <div class="cover placeholder"></div>
      {/if}
      <div class="book-info">
        <h1>{book.title}</h1>
        <p class="author">{book.author}</p>
        {#if book.narrator}<p class="narrator">Читает: {book.narrator}</p>{/if}
        <p class="meta">{book.chapters.length} глав</p>
        <div class="book-actions">
          {#if player.book?.id === bookId && player.playing}
            <button class="btn-resume playing" onclick={() => { player.playing = false; }}>⏸ Пауза</button>
          {:else}
            <button class="btn-resume" onclick={() => playChapter(progressChapterIdx(book) >= 0 ? progressChapterIdx(book) : 0)}>▶ {book.progress ? 'Продолжить' : 'Слушать'}</button>
          {/if}
          <a href="/library" class="back">← Библиотека</a>
        </div>
      </div>
    </div>

    <div class="chapters">
      {#each book.chapters as ch, i (ch.id)}
        {@const pct = liveChapterProgress(book, ch, i)}
        {@const active = isActive(i)}
        {@const started = pct > 0}
        <button class="chapter" class:active onclick={() => playChapter(i)}>
          <div class="ch-left">
            <span class="ch-num" class:playing={active && player.playing}>
              {#if active && player.playing}▶{:else}{i + 1}{/if}
            </span>
            <div class="ch-info">
              <span class="ch-name">{chapterName(ch, i)}</span>
              {#if ch.durationSec}
                <span class="ch-dur">{fmt(ch.durationSec)}</span>
              {/if}
            </div>
          </div>

          <div class="ch-progress-wrap">
            <div class="ch-progress-track">
              <div class="ch-progress-fill" style="width: {pct * 100}%"
                class:complete={pct >= 0.99}></div>
            </div>
            {#if pct >= 0.99}
              <span class="ch-done">✓</span>
            {:else if ch.durationSec}
              <span class="ch-remaining">−{fmt(ch.durationSec * (1 - pct))}</span>
            {/if}
          </div>
        </button>
      {/each}
    </div>
  {/if}
</main>

<style>
  .page { max-width: 720px; margin: 0 auto; padding: 1.5rem; }

  .book-header {
    display: flex;
    gap: 1.5rem;
    margin-bottom: 2rem;
    align-items: flex-start;
  }

  .cover {
    width: 120px;
    height: 120px;
    border-radius: 10px;
    object-fit: cover;
    flex-shrink: 0;
  }
  .cover.placeholder {
    background: linear-gradient(135deg, #2a2a2a, #1a1a1a);
  }

  .book-info { flex: 1; min-width: 0; }
  h1 { font-size: 1.4rem; font-weight: 700; margin-bottom: 0.25rem; }
  .author { color: #888; font-size: 0.9rem; margin-bottom: 0.1rem; }
  .narrator { color: #555; font-size: 0.8rem; margin-bottom: 0.1rem; }
  .meta { color: #444; font-size: 0.8rem; margin: 0.5rem 0; }
  .book-actions { display: flex; align-items: center; gap: 1rem; margin-top: 0.75rem; }
  .btn-resume {
    background: #fff; color: #000;
    border: none; border-radius: 20px;
    padding: 0.4rem 1.1rem; font-size: 0.85rem; font-weight: 600;
    cursor: pointer; transition: opacity 0.1s;
  }
  .btn-resume:hover { opacity: 0.85; }
  .btn-resume.playing { background: #2a2a2a; color: #fff; }
  .back { color: #555; font-size: 0.85rem; text-decoration: none; }
  .back:hover { color: #fff; }

  .hint { color: #555; text-align: center; padding: 3rem 0; }
  .err { color: #f87171; }

  /* Chapters list */
  .chapters { display: flex; flex-direction: column; gap: 2px; }

  .chapter {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    padding: 0.65rem 0.75rem;
    background: none;
    border: none;
    border-radius: 8px;
    cursor: pointer;
    text-align: left;
    width: 100%;
    color: #888;
    transition: background 0.1s, color 0.1s;
  }
  .chapter:hover { background: #1a1a1a; color: #ccc; }
  .chapter.active { background: #1e1e1e; color: #fff; }

  .ch-left {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    flex: 1;
    min-width: 0;
  }

  .ch-num {
    width: 2rem;
    text-align: right;
    font-size: 0.8rem;
    color: #444;
    flex-shrink: 0;
    font-variant-numeric: tabular-nums;
  }
  .ch-num.playing { color: #fff; font-size: 0.7rem; }
  .chapter.active .ch-num { color: #aaa; }

  .ch-info { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 1px; }
  .ch-name { font-size: 0.9rem; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .ch-dur { font-size: 0.72rem; color: #555; font-variant-numeric: tabular-nums; }
  .chapter.active .ch-dur { color: #666; }

  .ch-progress-wrap {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    width: 120px;
    flex-shrink: 0;
  }

  .ch-progress-track {
    flex: 1;
    height: 3px;
    background: #2a2a2a;
    border-radius: 2px;
    overflow: hidden;
  }

  .ch-progress-fill {
    height: 100%;
    background: #555;
    border-radius: 2px;
    transition: width 0.3s;
  }
  .ch-progress-fill.complete { background: #4ade80; }
  .chapter.active .ch-progress-fill:not(.complete) { background: #fff; }

  .ch-remaining { font-size: 0.68rem; color: #444; font-variant-numeric: tabular-nums; flex-shrink: 0; }
  .ch-done { font-size: 0.7rem; color: #4ade80; flex-shrink: 0; }
</style>
