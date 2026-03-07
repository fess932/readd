<template>
  <audio
    ref="audioEl"
    @loadedmetadata="onLoadedMetadata"
    @timeupdate="onTimeUpdate"
    @pause="onPause"
    @play="onPlay"
    @ended="onEnded"
  ></audio>

  <div v-if="player.book" class="player">
    <div class="player-left">
      <img v-if="player.book.coverPath" :src="`/uploads/${player.book.coverPath}`" alt="" class="player-thumb" />
      <div v-else class="player-thumb placeholder"></div>
      <div class="player-meta">
        <p class="player-title">{{ player.book.title }}</p>
        <p class="player-sub">{{ player.book.author }}</p>
      </div>
    </div>

    <div class="player-center">
      <div class="player-controls">
        <button class="ctrl-btn" @click="prevChapter" :disabled="player.chapterIdx === 0" title="Предыдущая">⏮</button>
        <button class="ctrl-btn play-btn" @click="togglePlay">{{ player.playing ? '⏸' : '▶' }}</button>
        <button class="ctrl-btn" @click="nextChapter" :disabled="player.chapterIdx >= chapterCount - 1" title="Следующая">⏭</button>
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
        <span>{{ fmt(player.currentTime) }}</span>
        <span class="remaining">−{{ fmt(remaining) }}</span>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, watch, onUnmounted } from 'vue';
import { player, currentChapterPath, saveProgress } from '../stores/player';

const audioEl = ref<HTMLAudioElement | null>(null);
const seekBarEl = ref<HTMLElement | null>(null);
const dragging = ref(false);
const switching = ref(false);

function fmt(s: number): string {
  if (!isFinite(s) || s < 0) return '0:00';
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const sec = Math.floor(s % 60);
  return h > 0
    ? `${h}:${String(m).padStart(2, '0')}:${String(sec).padStart(2, '0')}`
    : `${m}:${String(sec).padStart(2, '0')}`;
}

// Watch chapter path changes to swap audio src
watch(currentChapterPath, (path) => {
  if (!audioEl.value || !path) return;
  const src = `/uploads/${path}`;
  if (audioEl.value.src !== location.origin + src) {
    switching.value = true;
    audioEl.value.src = src;
    audioEl.value.load();
  }
}, { immediate: true });

// Watch playing state
watch(() => player.playing, (playing) => {
  if (!audioEl.value || switching.value) return;
  if (playing) {
    audioEl.value.play().catch(() => { player.playing = false; });
  } else {
    audioEl.value.pause();
  }
});

function onLoadedMetadata() {
  if (!audioEl.value) return;
  player.duration = audioEl.value.duration;
  if (player.positionSec > 0 && player.positionSec < audioEl.value.duration - 1) {
    audioEl.value.currentTime = player.positionSec;
    player.currentTime = player.positionSec;
    player.positionSec = 0;
  }
  if (player.playing) audioEl.value.play().catch(() => {});
  switching.value = false;
}

function onTimeUpdate() {
  if (!audioEl.value) return;
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
  saveProgress(true, player.duration > 5 ? player.duration - 5 : 0);
  if (player.book && player.chapterIdx < player.book.chapters.length - 1) {
    player.chapterIdx++;
    player.positionSec = 0;
    player.playing = true;
  } else {
    player.playing = false;
  }
}

function seekFromX(clientX: number) {
  if (!audioEl.value || !player.duration || !seekBarEl.value) return;
  const rect = seekBarEl.value.getBoundingClientRect();
  const pct = Math.max(0, Math.min(1, (clientX - rect.left) / rect.width));
  audioEl.value.currentTime = pct * player.duration;
  player.currentTime = audioEl.value.currentTime;
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
  if (e.key === 'ArrowLeft')  { e.preventDefault(); audioEl.value && (audioEl.value.currentTime -= 10); }
  if (e.key === 'ArrowRight') { e.preventDefault(); audioEl.value && (audioEl.value.currentTime += 10); }
}

watch(dragging, (isDragging) => {
  if (!isDragging) return;
  const onMove = (e: MouseEvent | TouchEvent) => {
    const x = e instanceof MouseEvent ? e.clientX : e.touches[0].clientX;
    seekFromX(x);
  };
  const onUp = () => { dragging.value = false; };
  window.addEventListener('mousemove', onMove);
  window.addEventListener('touchmove', onMove);
  window.addEventListener('mouseup', onUp);
  window.addEventListener('touchend', onUp);

  const stop = watch(dragging, (val) => {
    if (!val) {
      window.removeEventListener('mousemove', onMove);
      window.removeEventListener('touchmove', onMove);
      window.removeEventListener('mouseup', onUp);
      window.removeEventListener('touchend', onUp);
      stop();
    }
  });
});

function prevChapter() {
  if (!player.book || player.chapterIdx === 0) return;
  saveProgress(true);
  player.chapterIdx--;
  player.positionSec = 0;
  player.playing = true;
}

function nextChapter() {
  if (!player.book || player.chapterIdx >= player.book.chapters.length - 1) return;
  saveProgress(true);
  player.chapterIdx++;
  player.positionSec = 0;
  player.playing = true;
}

function togglePlay() {
  if (!player.book) return;
  player.playing = !player.playing;
}

const progress = computed(() => player.duration > 0 ? player.currentTime / player.duration : 0);
const remaining = computed(() => player.duration > 0 ? player.duration - player.currentTime : 0);
const chapterCount = computed(() => player.book?.chapters.length ?? 0);
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
</style>
