<template>
  <audio
    ref="audioEl"
    @loadedmetadata="onLoadedMetadata"
    @timeupdate="onTimeUpdate"
    @pause="onPause"
    @play="onPlay"
    @ended="onEnded"
    @error="onError"
  ></audio>

  <div v-if="player.book" class="player">
    <div class="player-left">
      <img :src="coverUrl(player.book.coverPath)" alt="" class="player-thumb" />
      <div class="player-meta">
        <p class="player-title">{{ player.book.title }}</p>
        <p class="player-sub">{{ player.book.author }}</p>
      </div>
    </div>

    <div class="player-center">
      <div class="player-controls">
        <button class="ctrl-btn" @click="prevChapter" :disabled="player.chapterIdx === 0" title="Предыдущая глава ["><SkipBack :size="16" /></button>
        <button class="ctrl-btn" @click="skip(-SKIP_SEC)" title="-30 сек ←"><RotateCcw :size="15" /></button>
        <button class="ctrl-btn play-btn" @click="togglePlay" title="Пауза/Воспроизведение Пробел"><Pause v-if="player.playing" :size="20" /><Play v-else :size="20" /></button>
        <button class="ctrl-btn" @click="skip(SKIP_SEC)" title="+30 сек →"><RotateCw :size="15" /></button>
        <button class="ctrl-btn" @click="nextChapter" :disabled="player.chapterIdx >= chapterCount - 1" title="Следующая глава ]"><SkipForward :size="16" /></button>
      </div>

      <div
        class="seek-wrap"
        :class="{ dragging }"
        ref="seekBarEl"
        @mousedown="onSeekMouseDown"
        @touchstart="onSeekTouchStart"
        role="slider"
        aria-label="Перемотка"
        :aria-valuenow="Math.round(progress * 100)"
        aria-valuemin="0"
        aria-valuemax="100"
        tabindex="0"
        @keydown="onSeekKeyDown"
      >
        <div class="seek-fill" :style="{ width: progress * 100 + '%' }"></div>
        <div class="seek-thumb" :style="{ left: progress * 100 + '%' }"></div>
      </div>

      <div class="player-times">
        <span>{{ formatClock(player.currentTime) }}</span>
        <span class="remaining">−{{ formatClock(remaining) }}</span>
      </div>
    </div>

    <div class="player-right">
      <button class="speed-btn" @click="cycleSpeed" :title="`Скорость: ${player.speed}×`">{{ player.speed }}×</button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, watch, onMounted, onUnmounted } from 'vue';
import { SkipBack, SkipForward, Play, Pause, RotateCcw, RotateCw } from 'lucide-vue-next';
import { useQueryClient } from '@tanstack/vue-query';
import { api } from '../api';
import { player, currentChapterPath, openChapter, saveProgress } from '../stores/player';
import { toast } from '../stores/toasts';
import { coverUrl, formatClock } from '../utils/format';

const audioEl = ref<HTMLAudioElement | null>(null);
const seekBarEl = ref<HTMLElement | null>(null);
const dragging = ref(false);
/** A new chapter is loading: events of the old one must not touch the state. */
const switching = ref(false);
const queryClient = useQueryClient();

const SKIP_SEC = 30;
const SEEK_KEY_SEC = 10;
const SPEEDS = [0.75, 1, 1.25, 1.5, 2];

const progress = computed(() => (player.duration > 0 ? player.currentTime / player.duration : 0));
const remaining = computed(() => (player.duration > 0 ? player.duration - player.currentTime : 0));
const chapterCount = computed(() => player.book?.chapters.length ?? 0);

function cycleSpeed() {
  const idx = SPEEDS.indexOf(player.speed);
  player.speed = SPEEDS[(idx + 1) % SPEEDS.length];
  localStorage.setItem('readd_speed', String(player.speed));
  if (audioEl.value) audioEl.value.playbackRate = player.speed;
}

// ── Keeping the <audio> element in sync with the store ──────────────────────

watch(currentChapterPath, (path) => {
  const el = audioEl.value;
  if (!el) return;

  if (!path) {
    // The book was unloaded (removed from the library, logout)
    el.pause();
    el.removeAttribute('src');
    el.load();
    switching.value = false;
    return;
  }

  const src = `/uploads/${path}`;
  if (el.src !== location.origin + src) {
    switching.value = true;
    el.src = src;
    el.load();
  }
}, { immediate: true });

watch(() => player.playing, (playing) => {
  if (!audioEl.value || switching.value) return;
  if (playing) {
    audioEl.value.play().catch(() => { player.playing = false; });
  } else {
    audioEl.value.pause();
  }
});

// The store asks to jump within the loaded chapter (another device listened further)
watch(() => player.seekRequest, (sec) => {
  if (sec === null) return;
  player.seekRequest = null;
  if (switching.value || !player.duration) {
    player.positionSec = sec; // applied once the audio has loaded
  } else {
    seekTo(sec);
  }
});

function onLoadedMetadata() {
  const el = audioEl.value;
  if (!el) return;
  player.duration = el.duration;
  el.playbackRate = player.speed;

  const resumeAt = player.positionSec < el.duration - 1 ? player.positionSec : 0;
  if (resumeAt > 0) el.currentTime = resumeAt;
  player.currentTime = resumeAt;
  player.positionSec = 0;

  switching.value = false;
  if (player.playing) el.play().catch(() => { player.playing = false; });
}

function onTimeUpdate() {
  if (!audioEl.value || switching.value) return;
  player.currentTime = audioEl.value.currentTime;
  saveProgress();
}

function onPause() {
  if (switching.value) return;
  player.playing = false;
  saveProgress(true);
}

function onPlay() {
  player.playing = true;
  switching.value = false;
}

function onEnded() {
  if (!player.book) return;
  saveProgress(true, player.duration);

  if (player.chapterIdx < player.book.chapters.length - 1) {
    openChapter(player.chapterIdx + 1);
    return;
  }
  // The last chapter is over: the book is finished
  player.playing = false;
  api.library.finish(player.book.id, true)
    .then(() => queryClient.invalidateQueries({ queryKey: ['library'] }))
    .catch(() => {});
}

function onError() {
  // Unloading the book also fires `error`; only a real source matters
  if (!player.book || !audioEl.value?.getAttribute('src')) return;
  switching.value = false;
  player.playing = false;
  toast('error', 'Не удалось загрузить аудио');
}

// ── Controls ────────────────────────────────────────────────────────────────

function togglePlay() {
  if (!player.book) return;
  player.playing = !player.playing;
}

function seekTo(sec: number) {
  if (!audioEl.value || !player.duration) return;
  audioEl.value.currentTime = Math.max(0, Math.min(player.duration, sec));
  player.currentTime = audioEl.value.currentTime;
}

function skip(sec: number) {
  if (audioEl.value) seekTo(audioEl.value.currentTime + sec);
}

function prevChapter() {
  if (!player.book || player.chapterIdx === 0) return;
  saveProgress(true);
  openChapter(player.chapterIdx - 1);
}

function nextChapter() {
  if (!player.book || player.chapterIdx >= player.book.chapters.length - 1) return;
  saveProgress(true);
  openChapter(player.chapterIdx + 1);
}

// ── Hotkeys ─────────────────────────────────────────────────────────────────

function onKeyDown(e: KeyboardEvent) {
  if (!player.book) return;
  // Leave browser shortcuts (Cmd+←, Alt+→ …) and open dialogs alone
  if (e.metaKey || e.ctrlKey || e.altKey || document.querySelector('dialog[open]')) return;

  const target = e.target as HTMLElement;
  const tag = target.tagName;
  if (tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT' || target.isContentEditable) return;

  // `code` is the physical key, so the brackets work in any keyboard layout
  switch (e.code) {
    case 'Space':
      if (tag === 'BUTTON' || tag === 'A') return; // Space activates the focused control
      e.preventDefault();
      togglePlay();
      break;
    case 'ArrowLeft': e.preventDefault(); skip(-SKIP_SEC); break;
    case 'ArrowRight': e.preventDefault(); skip(SKIP_SEC); break;
    case 'BracketLeft': prevChapter(); break;
    case 'BracketRight': nextChapter(); break;
  }
}

// Closing the tab mid-chapter: `pause` never fires, so save here
function onPageHide() {
  if (player.playing) saveProgress(true);
}

onMounted(() => {
  window.addEventListener('keydown', onKeyDown);
  window.addEventListener('pagehide', onPageHide);
});
onUnmounted(() => {
  window.removeEventListener('keydown', onKeyDown);
  window.removeEventListener('pagehide', onPageHide);
});

// ── Seek bar ────────────────────────────────────────────────────────────────

function seekFromX(clientX: number) {
  if (!seekBarEl.value) return;
  const rect = seekBarEl.value.getBoundingClientRect();
  seekTo(((clientX - rect.left) / rect.width) * player.duration);
}

function onSeekMouseDown(e: MouseEvent) {
  dragging.value = true;
  seekFromX(e.clientX);
}

function onSeekTouchStart(e: TouchEvent) {
  dragging.value = true;
  seekFromX(e.touches[0].clientX);
}

function onSeekKeyDown(e: KeyboardEvent) {
  if (e.key !== 'ArrowLeft' && e.key !== 'ArrowRight') return;
  e.preventDefault();
  e.stopPropagation(); // otherwise the global hotkey would skip as well
  skip(e.key === 'ArrowLeft' ? -SEEK_KEY_SEC : SEEK_KEY_SEC);
}

watch(dragging, (isDragging, _, onCleanup) => {
  if (!isDragging) return;
  const onMove = (e: MouseEvent | TouchEvent) => {
    seekFromX(e instanceof MouseEvent ? e.clientX : e.touches[0].clientX);
  };
  const onUp = () => { dragging.value = false; };
  window.addEventListener('mousemove', onMove);
  window.addEventListener('touchmove', onMove);
  window.addEventListener('mouseup', onUp);
  window.addEventListener('touchend', onUp);
  onCleanup(() => {
    window.removeEventListener('mousemove', onMove);
    window.removeEventListener('touchmove', onMove);
    window.removeEventListener('mouseup', onUp);
    window.removeEventListener('touchend', onUp);
  });
});
</script>

<style scoped>
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

.player-right {
  display: flex;
  align-items: center;
  flex-shrink: 0;
  min-width: 48px;
  justify-content: flex-end;
}

.speed-btn {
  background: none;
  border: 1px solid #2a2a2a;
  color: #666;
  font-size: 0.75rem;
  font-weight: 600;
  padding: 0.25rem 0.5rem;
  border-radius: 5px;
  cursor: pointer;
  font-variant-numeric: tabular-nums;
  min-width: 40px;
  text-align: center;
}
.speed-btn:hover { color: #fff; border-color: #444; }
</style>
