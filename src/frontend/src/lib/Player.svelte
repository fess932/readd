<script lang="ts">
  import { player, currentChapterPath, saveProgress } from './playerState.svelte';
  import { api } from './api';

  let audioEl = $state<HTMLAudioElement | null>(null);
  let saveTimer: ReturnType<typeof setTimeout> | null = null;
  let seekBarEl = $state<HTMLElement | null>(null);
  let dragging = $state(false);
  let switching = $state(false); // true пока грузится новая глава — игнорируем pause

  function fmt(s: number): string {
    if (!isFinite(s) || s < 0) return '0:00';
    const h = Math.floor(s / 3600);
    const m = Math.floor((s % 3600) / 60);
    const sec = Math.floor(s % 60);
    return h > 0
      ? `${h}:${String(m).padStart(2, '0')}:${String(sec).padStart(2, '0')}`
      : `${m}:${String(sec).padStart(2, '0')}`;
  }

  function scheduleSave() {
    if (saveTimer) clearTimeout(saveTimer);
    saveTimer = setTimeout(() => { saveProgress(); }, 1000);
  }

  // Смена главы — загружаем новый файл
  $effect(() => {
    const path = currentChapterPath();
    if (!audioEl || !path) return;
    const src = `/uploads/${path}`;
    if (audioEl.src !== location.origin + src) {
      switching = true;
      audioEl.src = src;
      audioEl.load();
    }
  });

  // Управляем воспроизведением через player.playing
  $effect(() => {
    if (!audioEl || switching) return;
    if (player.playing) {
      audioEl.play().catch(() => { player.playing = false; });
    } else {
      audioEl.pause();
    }
  });

  function onLoadedMetadata() {
    if (!audioEl) return;
    player.duration = audioEl.duration;
    if (player.positionSec > 0 && player.positionSec < audioEl.duration - 1) {
      audioEl.currentTime = player.positionSec;
      player.positionSec = 0;
    }
    // play() сразу здесь — seek уже применён, эффект придёт позже
    if (player.playing) audioEl.play().catch(() => {});
    switching = false;
  }

  function onTimeUpdate() {
    if (!audioEl) return;
    player.currentTime = audioEl.currentTime;
    scheduleSave();
  }

  function onPause() {
    if (switching) return; // load() генерирует pause — игнорируем
    player.playing = false;
    saveProgress();
  }
  function onPlay() { player.playing = true; switching = false; }

  function onEnded() {
    saveProgress();
    if (player.book && player.chapterIdx < player.book.chapters.length - 1) {
      player.chapterIdx++;
      player.positionSec = 0;
      player.playing = true;
    } else {
      player.playing = false;
    }
  }

  // --- Seekbar: drag-to-seek ---
  function seekFromX(clientX: number) {
    if (!audioEl || !player.duration || !seekBarEl) return;
    const rect = seekBarEl.getBoundingClientRect();
    const pct = Math.max(0, Math.min(1, (clientX - rect.left) / rect.width));
    audioEl.currentTime = pct * player.duration;
    player.currentTime = audioEl.currentTime;
  }

  function onSeekMouseDown(e: MouseEvent) {
    dragging = true;
    seekFromX(e.clientX);
  }

  function onSeekTouchStart(e: TouchEvent) {
    dragging = true;
    seekFromX(e.touches[0].clientX);
  }

  $effect(() => {
    if (!dragging) return;
    const onMove = (e: MouseEvent | TouchEvent) => {
      const x = e instanceof MouseEvent ? e.clientX : e.touches[0].clientX;
      seekFromX(x);
    };
    const onUp = () => { dragging = false; };
    window.addEventListener('mousemove', onMove);
    window.addEventListener('touchmove', onMove);
    window.addEventListener('mouseup', onUp);
    window.addEventListener('touchend', onUp);
    return () => {
      window.removeEventListener('mousemove', onMove);
      window.removeEventListener('touchmove', onMove);
      window.removeEventListener('mouseup', onUp);
      window.removeEventListener('touchend', onUp);
    };
  });

  function prevChapter() {
    if (!player.book || player.chapterIdx === 0) return;
    saveProgress();
    player.chapterIdx--;
    player.positionSec = 0;
    player.playing = true;
  }

  function nextChapter() {
    if (!player.book || player.chapterIdx >= player.book.chapters.length - 1) return;
    saveProgress();
    player.chapterIdx++;
    player.positionSec = 0;
    player.playing = true;
  }

  function togglePlay() {
    if (!player.book) return;
    player.playing = !player.playing;
  }

  $effect(() => { return () => { if (saveTimer) clearTimeout(saveTimer); }; });

  const progress     = $derived(player.duration > 0 ? player.currentTime / player.duration : 0);
  const remaining    = $derived(player.duration > 0 ? player.duration - player.currentTime : 0);
  const chapterCount = $derived(player.book?.chapters.length ?? 0);
</script>

<audio
  bind:this={audioEl}
  onloadedmetadata={onLoadedMetadata}
  ontimeupdate={onTimeUpdate}
  onpause={onPause}
  onplay={onPlay}
  onended={onEnded}
></audio>

{#if player.book}
  <div class="player">
    <!-- Обложка + инфо -->
    <div class="player-left">
      {#if player.book.coverPath}
        <img src="/uploads/{player.book.coverPath}" alt="" class="player-thumb" />
      {:else}
        <div class="player-thumb placeholder"></div>
      {/if}
      <div class="player-meta">
        <p class="player-title">{player.book.title}</p>
        <p class="player-sub">{player.book.author}</p>
      </div>
    </div>

    <!-- Контролы -->
    <div class="player-center">
      <div class="player-controls">
        <button class="ctrl-btn" onclick={prevChapter} disabled={player.chapterIdx === 0} title="Предыдущая">⏮</button>
        <button class="ctrl-btn play-btn" onclick={togglePlay}>{player.playing ? '⏸' : '▶'}</button>
        <button class="ctrl-btn" onclick={nextChapter} disabled={player.chapterIdx >= chapterCount - 1} title="Следующая">⏭</button>
      </div>

      <!-- Seekbar с drag-to-seek -->
      <div
        class="seek-wrap"
        class:dragging
        bind:this={seekBarEl}
        onmousedown={onSeekMouseDown}
        ontouchstart={onSeekTouchStart}
        role="slider"
        aria-label="Перемотка"
        aria-valuenow={Math.round(progress * 100)}
        aria-valuemin={0}
        aria-valuemax={100}
        tabindex="0"
        onkeydown={(e) => {
          if (e.key === 'ArrowLeft')  { e.preventDefault(); audioEl && (audioEl.currentTime -= 10); }
          if (e.key === 'ArrowRight') { e.preventDefault(); audioEl && (audioEl.currentTime += 10); }
        }}
      >
        <div class="seek-fill" style="width: {progress * 100}%"></div>
        <div class="seek-thumb" style="left: {progress * 100}%"></div>
      </div>

      <div class="player-times">
        <span>{fmt(player.currentTime)}</span>
        <span class="remaining">−{fmt(remaining)}</span>
      </div>
    </div>
  </div>
{/if}

<style>
  /* Плеер */
  .player {
    position: fixed;
    bottom: 0;
    left: 0;
    right: 0;
    height: 80px;
    background: #111;
    border-top: 1px solid #222;
    display: flex;
    align-items: center;
    gap: 1rem;
    padding: 0 1.5rem;
    z-index: 50;
    user-select: none;
  }

  .player-left {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    min-width: 180px;
    flex-shrink: 0;
  }

  .player-thumb { width: 48px; height: 48px; border-radius: 6px; object-fit: cover; flex-shrink: 0; }
  .player-thumb.placeholder { background: #2a2a2a; }

  .player-meta { min-width: 0; }
  .player-title { font-size: 0.85rem; font-weight: 600; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; max-width: 150px; }
  .player-sub   { font-size: 0.75rem; color: #666; }

  .player-center {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 4px;
    max-width: 600px;
    margin: 0 auto;
  }

  .player-controls {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 0.4rem;
  }

  .ctrl-btn {
    background: none;
    border: none;
    color: #aaa;
    font-size: 1rem;
    cursor: pointer;
    padding: 0.2rem 0.4rem;
    border-radius: 4px;
    transition: color 0.1s;
  }
  .ctrl-btn:hover:not(:disabled) { color: #fff; }
  .ctrl-btn:disabled { opacity: 0.25; cursor: default; }
  .play-btn { font-size: 1.3rem; color: #fff; }

  /* Seekbar */
  .seek-wrap {
    position: relative;
    height: 4px;
    background: #2a2a2a;
    border-radius: 2px;
    cursor: pointer;
    outline: none;
    transition: height 0.1s;
  }
  .seek-wrap:hover,
  .seek-wrap.dragging { height: 6px; }

  .seek-fill {
    position: absolute;
    left: 0; top: 0; bottom: 0;
    background: #fff;
    border-radius: 2px;
    pointer-events: none;
  }
  .seek-thumb {
    position: absolute;
    top: 50%;
    transform: translate(-50%, -50%);
    width: 12px; height: 12px;
    background: #fff;
    border-radius: 50%;
    pointer-events: none;
    opacity: 0;
    transition: opacity 0.1s;
  }
  .seek-wrap:hover .seek-thumb,
  .seek-wrap.dragging .seek-thumb { opacity: 1; }

  .player-times {
    display: flex;
    justify-content: space-between;
    font-size: 0.7rem;
    color: #555;
    font-variant-numeric: tabular-nums;
  }
  .remaining { color: #444; }
</style>
