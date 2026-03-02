<script lang="ts">
  import { onMount } from 'svelte';
  import { api, type LibraryBook } from '$lib/api';
  import { player, playBook } from '$lib/playerState.svelte';
  import { toast } from '$lib/toast.svelte';
  import Confirm from '$lib/Confirm.svelte';

  let books = $state<LibraryBook[]>([]);
  let loading = $state(true);
  let error = $state('');
  let removingId = $state<number | null>(null);
  let confirmRemoveId = $state<number | null>(null);

  async function loadLibrary() {
    loading = true;
    error = '';
    try {
      books = await api.library.list();
    } catch (e: any) {
      error = e.message;
    } finally {
      loading = false;
    }
  }

  async function removeBook(bookId: number) {
    removingId = bookId;
    confirmRemoveId = null;
    try {
      await api.library.remove(bookId);
      books = books.filter(b => b.id !== bookId);
      if (player.book?.id === bookId) player.book = null;
      toast('success', 'Убрано из библиотеки');
    } catch (e: any) {
      toast('error', e.message);
    } finally {
      removingId = null;
    }
  }

  function resume(book: LibraryBook) {
    const chapterIdx = book.progress
      ? book.chapters.findIndex(c => c.filePath === book.progress!.chapterPath)
      : 0;
    playBook(book, Math.max(0, chapterIdx), book.progress?.positionSec ?? 0);
  }

  function isPlaying(book: LibraryBook) {
    return player.book?.id === book.id && player.playing;
  }

  function isActive(book: LibraryBook) {
    return player.book?.id === book.id;
  }

  // Прогресс книги: (завершённые главы + позиция в текущей) / всего глав
  function bookProgress(book: LibraryBook): number {
    if (!book.chapters.length || !book.progress) return 0;
    const prog = book.progress;
    const chIdx = book.chapters.findIndex(c => c.filePath === prog.chapterPath);
    if (chIdx < 0) return 0;

    const chDur = book.chapters[chIdx].durationSec;
    const chProgress = chDur && chDur > 0 ? prog.positionSec / chDur : 0;
    return (chIdx + chProgress) / book.chapters.length;
  }

  // Если книга в плеере — берём актуальное время из player
  function liveProgress(book: LibraryBook): number {
    if (player.book?.id !== book.id) return bookProgress(book);
    const chDur = player.duration;
    const chProgress = chDur > 0 ? player.currentTime / chDur : 0;
    return (player.chapterIdx + chProgress) / (player.book.chapters.length || 1);
  }

  function remainingTime(book: LibraryBook): string | null {
    const knownDurations = book.chapters.filter(c => c.durationSec).map(c => c.durationSec!);
    if (knownDurations.length === 0) return null;

    const avgDur = knownDurations.reduce((a, b) => a + b, 0) / knownDurations.length;
    const totalSec = book.chapters.length * avgDur;
    const listenedSec = liveProgress(book) * totalSec;
    const leftSec = Math.max(0, totalSec - listenedSec);

    const h = Math.floor(leftSec / 3600);
    const m = Math.floor((leftSec % 3600) / 60);
    return h > 0 ? `~${h} ч ${m} мин` : `~${m} мин`;
  }

  // Авто-восстановление последней книги при входе
  onMount(async () => {
    await loadLibrary();

    // Восстанавливаем последнюю книгу только если плеер пустой
    if (!player.book) {
      const last = await api.progress.last().catch(() => null);
      if (last) {
        const book = books.find(b => b.id === last.bookId);
        if (book) {
          const chapterIdx = book.chapters.findIndex(c => c.filePath === last.chapterPath);
          playBook(book, Math.max(0, chapterIdx), last.positionSec);
          player.playing = false; // загружаем но не автоплей
        }
      }
    }
  });
</script>

<main class="page">
  <div class="page-header">
    <h2>Моя библиотека</h2>
  </div>

  {#if loading}
    <p class="hint">Загрузка...</p>
  {:else if error}
    <p class="error-msg">{error}</p>
  {:else if books.length === 0}
    <p class="hint">Библиотека пуста. Добавьте книги из <a href="/explore">общей библиотеки</a>.</p>
  {:else}
    <div class="books-list">
      {#each books as book (book.id)}
        {@const pct = liveProgress(book)}
        {@const remaining = remainingTime(book)}
        <div class="book-row" class:active={isActive(book)}>

          {#if book.coverPath}
            <img src="/uploads/{book.coverPath}" alt={book.title} class="row-cover" />
          {:else}
            <div class="row-cover placeholder"></div>
          {/if}

          <div class="row-body">
            <div class="row-top">
              <div class="row-info" onclick={() => resume(book)} role="button" tabindex="0" onkeydown={(e) => e.key === 'Enter' && resume(book)}>
                <p class="row-title">{book.title}</p>
                <p class="row-author">{book.author}{book.narrator ? ` · ${book.narrator}` : ''}</p>
              </div>
              <div class="row-actions">
                <button class="btn-play" onclick={() => resume(book)} title={isPlaying(book) ? 'Пауза' : 'Играть'}>
                  {isPlaying(book) ? '⏸' : '▶'}
                </button>
                <button class="btn-remove" onclick={() => confirmRemoveId = book.id} disabled={removingId === book.id} title="Убрать из библиотеки">✕</button>
              </div>
            </div>

            <!-- Прогресс строка -->
            <div class="progress-row">
              <button class="progress-track" type="button"
                aria-label="Прогресс книги, нажмите для перемотки"
                onclick={(e) => {
                  const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
                  const p = (e.clientX - rect.left) / rect.width;
                  const chIdx = Math.floor(p * book.chapters.length);
                  playBook(book, Math.min(chIdx, book.chapters.length - 1), 0);
                }}
                onkeydown={(e) => {
                  if (e.key === 'ArrowRight') playBook(book, Math.min(player.chapterIdx + 1, book.chapters.length - 1), 0);
                  if (e.key === 'ArrowLeft')  playBook(book, Math.max(player.chapterIdx - 1, 0), 0);
                }}>
                <div class="progress-fill" style="width: {pct * 100}%"></div>
              </button>
              <div class="progress-meta">
                {#if book.chapters.length > 1}
                  <span class="chapters-info">
                    {#if book.progress}
                      Гл. {(book.chapters.findIndex(c => c.filePath === book.progress!.chapterPath) + 1)} / {book.chapters.length}
                    {:else}
                      {book.chapters.length} глав
                    {/if}
                  </span>
                {/if}
                {#if remaining}
                  <span class="remaining">осталось {remaining}</span>
                {/if}
              </div>
            </div>
          </div>
        </div>
      {/each}
    </div>
  {/if}
</main>

{#if confirmRemoveId !== null}
  {@const book = books.find(b => b.id === confirmRemoveId)}
  <Confirm
    message={book ? `Убрать «${book.title}» из вашей библиотеки?` : ''}
    confirmLabel="Убрать"
    onconfirm={() => removeBook(confirmRemoveId!)}
    oncancel={() => confirmRemoveId = null}
  />
{/if}

<style>
  .page { max-width: 800px; margin: 0 auto; padding: 1.5rem; }
  .page-header { margin-bottom: 1.5rem; }
  h2 { font-size: 1.4rem; font-weight: 700; }

  .hint { color: #555; text-align: center; padding: 3rem 0; }
  .hint a { color: #888; text-decoration: underline; }
  .error-msg { color: #f87171; }

  .books-list { display: flex; flex-direction: column; gap: 0.75rem; }

  .book-row {
    background: #1a1a1a;
    border: 1px solid #2a2a2a;
    border-radius: 10px;
    padding: 0.75rem;
    display: flex;
    gap: 0.75rem;
    align-items: flex-start;
    transition: border-color 0.15s;
  }
  .book-row.active { border-color: #444; }

  .row-cover { width: 64px; height: 64px; border-radius: 6px; object-fit: cover; flex-shrink: 0; }
  .row-cover.placeholder { background: linear-gradient(135deg, #2a2a2a, #1a1a1a); }

  .row-body { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 0.5rem; }

  .row-top { display: flex; align-items: flex-start; gap: 0.5rem; }
  .row-info { flex: 1; min-width: 0; cursor: pointer; }
  .row-title { font-weight: 500; font-size: 0.95rem; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .row-author { color: #666; font-size: 0.8rem; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; margin-top: 2px; }

  .row-actions { display: flex; gap: 0.35rem; flex-shrink: 0; }
  .btn-play { background: #2a2a2a; color: #fff; border: none; width: 30px; height: 30px; border-radius: 50%; cursor: pointer; font-size: 0.7rem; display: flex; align-items: center; justify-content: center; }
  .btn-play:hover { background: #333; }
  .btn-remove { background: none; color: #444; border: 1px solid #2a2a2a; width: 30px; height: 30px; border-radius: 50%; cursor: pointer; font-size: 0.65rem; display: flex; align-items: center; justify-content: center; }
  .btn-remove:hover:not(:disabled) { color: #f87171; border-color: #f87171; }
  button:disabled { opacity: 0.4; cursor: not-allowed; }

  /* Прогресс */
  .progress-row { display: flex; flex-direction: column; gap: 3px; }
  .progress-track {
    height: 3px; background: #2a2a2a; border-radius: 2px;
    cursor: pointer; position: relative; overflow: hidden;
    border: none; padding: 0; width: 100%; display: block;
    transition: height 0.1s;
  }
  .progress-track:hover { height: 5px; }
  .progress-track:focus-visible { outline: 1px solid #555; outline-offset: 2px; }
  .progress-fill { height: 100%; background: #555; border-radius: 2px; transition: width 0.3s; }
  .book-row.active .progress-fill { background: #fff; }

  .progress-meta { display: flex; justify-content: space-between; font-size: 0.72rem; }
  .chapters-info { color: #444; }
  .remaining { color: #444; }
</style>
