<template>
  <main class="page">
    <p v-if="isLoading" class="hint">Загрузка...</p>
    <p v-else-if="error" class="err">{{ (error as any).message }}</p>
    <template v-else-if="book">
      <div class="book-header">
        <img :src="book.coverPath ? `/uploads/${book.coverPath}` : '/placeholder.jpg'" :alt="book.title" class="cover" />
        <div class="book-info">
          <h1>{{ book.title }}</h1>
          <p class="author">{{ book.author }}</p>
          <p v-if="book.narrator" class="narrator">Читает: {{ book.narrator }}</p>
          <p class="meta">{{ book.chapters.length }} глав</p>
          <div class="book-actions">
            <button v-if="player.book?.id === bookId && player.playing" class="btn-resume playing" @click="player.playing = false">
              <Pause :size="14" /> Пауза
            </button>
            <button v-else class="btn-resume" @click="playChapter(progressChapterIdx >= 0 ? progressChapterIdx : 0)">
              <Play :size="14" /> {{ book.progress ? 'Продолжить' : 'Слушать' }}
            </button>
            <router-link to="/library" class="back"><ArrowLeft :size="14" /> Библиотека</router-link>
          </div>
        </div>
      </div>

      <div class="chapters">
        <button
          v-for="(ch, i) in book.chapters"
          :key="ch.id"
          class="chapter"
          :class="{ active: isActive(i) }"
          @click="playChapter(i)"
        >
          <div class="ch-left">
            <span class="ch-num" :class="{ playing: isActive(i) && player.playing }">
              {{ isActive(i) && player.playing ? '▶' : i + 1 }}
            </span>
            <div class="ch-info">
              <span class="ch-name">{{ chapterName(ch, i) }}</span>
              <span v-if="ch.durationSec" class="ch-dur">{{ fmt(ch.durationSec) }}</span>
            </div>
          </div>

          <div class="ch-progress-wrap">
            <div class="ch-progress-track">
              <div
                class="ch-progress-fill"
                :class="{ complete: liveChapterProgress(ch, i) >= 0.99 }"
                :style="{ width: liveChapterProgress(ch, i) * 100 + '%' }"
              ></div>
            </div>
            <span v-if="liveChapterProgress(ch, i) >= 0.99" class="ch-done">✓</span>
            <span v-else-if="ch.durationSec" class="ch-remaining">−{{ fmt(ch.durationSec * (1 - liveChapterProgress(ch, i))) }}</span>
          </div>
        </button>
      </div>
    </template>
  </main>
</template>

<script setup lang="ts">
import { computed, watch } from 'vue';
import { Play, Pause, ArrowLeft } from 'lucide-vue-next';
import { useRoute, useRouter } from 'vue-router';
import { useQuery } from '@tanstack/vue-query';
import { api, type Chapter } from '../api';
import { player, playBook, saveProgress, getChapterPos, setChapterPos } from '../stores/player';

const route = useRoute();
const router = useRouter();

const bookId = computed(() => Number(route.params.id));

const { data: book, isLoading, error } = useQuery({
  queryKey: computed(() => ['library', bookId.value]),
  queryFn: () => api.library.get(bookId.value),
});

const { data: positions } = useQuery({
  queryKey: computed(() => ['progress', bookId.value]),
  queryFn: () => api.progress.get(bookId.value),
});

// Sync server positions to in-memory store for playChapter resume
watch(positions, (ps) => {
  if (ps) for (const p of ps) setChapterPos(p.chapterPath, p.positionSec);
}, { immediate: true });

function serverPos(chapterPath: string): number {
  return positions.value?.find(p => p.chapterPath === chapterPath)?.positionSec ?? 0;
}

// Load book in player (no autoplay) when first opened
watch(book, (b) => {
  if (!b) return;
  if ((error.value as any)?.message?.includes('Not found')) {
    router.push('/library');
    return;
  }
  if (player.book?.id !== bookId.value) {
    const chapterIdx = b.progress
      ? b.chapters.findIndex(c => c.filePath === b.progress!.chapterPath)
      : 0;
    playBook(b, Math.max(0, chapterIdx), b.progress?.positionSec ?? 0);
    player.playing = false;
  }
}, { immediate: true });

watch(error, (err: any) => {
  if (err?.message?.includes('Not found')) router.push('/library');
});

const progressChapterIdx = computed(() => {
  if (!book.value?.progress) return -1;
  return book.value.chapters.findIndex(c => c.filePath === book.value!.progress!.chapterPath);
});

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

function liveChapterProgress(ch: Chapter, idx: number): number {
  if (player.book?.id === bookId.value && player.chapterIdx === idx && player.duration > 0) {
    return Math.min(1, player.currentTime / player.duration);
  }
  // positions.value читается напрямую — Vue отследит зависимость и перерисует когда query вернёт данные
  // getChapterPos даёт live-обновления от saveProgress во время воспроизведения
  const pos = Math.max(serverPos(ch.filePath), getChapterPos(ch.filePath));
  if (pos > 0 && ch.durationSec) return Math.min(1, pos / ch.durationSec);
  return 0;
}

function playChapter(idx: number) {
  if (!book.value) return;
  saveProgress(true);
  const ch = book.value.chapters[idx];
  const pos = isActive(idx)
    ? player.currentTime
    : Math.max(serverPos(ch.filePath), getChapterPos(ch.filePath));
  playBook(book.value, idx, pos);
}

function isActive(idx: number): boolean {
  return player.book?.id === bookId.value && player.chapterIdx === idx;
}
</script>

<style scoped>
.page { max-width: 720px; margin: 0 auto; padding: 1.5rem; }

.book-header {
  display: flex;
  gap: 1.5rem;
  margin-bottom: 2rem;
  align-items: flex-start;
}

.cover { width: 120px; height: 120px; border-radius: 10px; object-fit: cover; flex-shrink: 0; }
.cover.placeholder { background: linear-gradient(135deg, #2a2a2a, #1a1a1a); }

.book-info { flex: 1; min-width: 0; }
h1 { font-size: 1.4rem; font-weight: 700; margin-bottom: 0.25rem; }
.author { color: #888; font-size: 0.9rem; margin-bottom: 0.1rem; }
.narrator { color: #555; font-size: 0.8rem; margin-bottom: 0.1rem; }
.meta { color: #444; font-size: 0.8rem; margin: 0.5rem 0; }
.book-actions { display: flex; align-items: center; gap: 1rem; margin-top: 0.75rem; }
.btn-resume {
  display: flex; align-items: center; gap: 0.35rem;
  background: #fff; color: #000;
  border: none; border-radius: 20px;
  padding: 0.4rem 1.1rem; font-size: 0.85rem; font-weight: 600;
  cursor: pointer; transition: opacity 0.1s;
}
.btn-resume:hover { opacity: 0.85; }
.btn-resume.playing { background: #2a2a2a; color: #fff; }
.back { display: flex; align-items: center; gap: 0.3rem; color: #555; font-size: 0.85rem; text-decoration: none; }
.back:hover { color: #fff; }

.hint { color: #555; text-align: center; padding: 3rem 0; }
.err { color: #f87171; }

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

.ch-left { display: flex; align-items: center; gap: 0.75rem; flex: 1; min-width: 0; }

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

.ch-progress-wrap { display: flex; align-items: center; gap: 0.5rem; width: 120px; flex-shrink: 0; }

.ch-progress-track { flex: 1; height: 3px; background: #2a2a2a; border-radius: 2px; overflow: hidden; }

.ch-progress-fill { height: 100%; background: #555; border-radius: 2px; transition: width 0.3s; }
.ch-progress-fill.complete { background: #4ade80; }
.chapter.active .ch-progress-fill:not(.complete) { background: #fff; }

.ch-remaining { font-size: 0.68rem; color: #444; font-variant-numeric: tabular-nums; flex-shrink: 0; }
.ch-done { font-size: 0.7rem; color: #4ade80; flex-shrink: 0; }
</style>
