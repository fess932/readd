<!-- A book in the user's library: cover, progress and the finish / remove actions. -->
<template>
  <div class="book-row" :class="{ active: isActive, finished }">
    <component :is="finished ? 'div' : RouterLink" :to="link" class="row-cover-link" tabindex="-1" @click="emit('open')">
      <img :src="coverUrl(book.coverPath)" :alt="book.title" class="row-cover" />
    </component>

    <div class="row-body">
      <div class="row-top">
        <component :is="finished ? 'div' : RouterLink" :to="link" class="row-info" @click="emit('open')">
          <p class="row-title">{{ book.title }}</p>
          <p v-if="subtitle" class="row-author">{{ subtitle }}</p>
        </component>
        <div class="row-actions">
          <button
            class="btn-round finish"
            :class="{ active: finished }"
            @click="emit('finish', !finished)"
            :title="finished ? 'Снять отметку' : 'Отметить прочитанным'"
          ><CheckCheck :size="13" /></button>
          <button class="btn-round remove" @click="emit('remove')" :disabled="removing" title="Убрать из библиотеки"><X :size="11" /></button>
        </div>
      </div>

      <div v-if="finished" class="progress-track"><div class="progress-fill done"></div></div>
      <div v-else class="progress-row">
        <div class="progress-track"><div class="progress-fill" :style="{ width: progress * 100 + '%' }"></div></div>
        <div class="progress-meta">
          <span v-if="book.chapters.length > 1" class="chapters-info">{{ chaptersInfo }}</span>
          <span v-if="remaining" class="remaining">осталось {{ remaining }}</span>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue';
import { RouterLink } from 'vue-router';
import { X, CheckCheck } from 'lucide-vue-next';
import type { LibraryBook } from '../api';
import { player } from '../stores/player';
import { coverUrl, pluralChapters } from '../utils/format';

const props = defineProps<{
  book: LibraryBook;
  /** Off when the row sits under an author heading. */
  showAuthor: boolean;
  removing: boolean;
}>();

const emit = defineEmits<{
  open: [];
  finish: [finished: boolean];
  remove: [];
}>();

const finished = computed(() => !!props.book.finishedAt);
const isActive = computed(() => player.book?.id === props.book.id);
const link = computed(() => `/book/${props.book.id}`);

const subtitle = computed(() =>
  [props.showAuthor ? props.book.author : null, props.book.narrator].filter(Boolean).join(' · ')
);

/** Index of the chapter the saved progress points at, -1 if there is none. */
const savedChapterIdx = computed(() => {
  const saved = props.book.progress;
  return saved ? props.book.chapters.findIndex(c => c.filePath === saved.chapterPath) : -1;
});

/** Share of the book listened to, 0–1; live while this book is in the player. */
const progress = computed(() => {
  const chapters = props.book.chapters;
  if (!chapters.length) return 0;

  if (isActive.value) {
    const inChapter = player.duration > 0 ? player.currentTime / player.duration : 0;
    return (player.chapterIdx + inChapter) / chapters.length;
  }

  const idx = savedChapterIdx.value;
  if (idx < 0) return 0;
  const duration = chapters[idx].durationSec;
  const inChapter = duration ? Math.min(1, props.book.progress!.positionSec / duration) : 0;
  return (idx + inChapter) / chapters.length;
});

const chaptersInfo = computed(() => {
  const total = props.book.chapters.length;
  const current = isActive.value ? player.chapterIdx : savedChapterIdx.value;
  return current >= 0 ? `Гл. ${current + 1} / ${total}` : `${total} ${pluralChapters(total)}`;
});

/** Rough time left, extrapolating chapters of unknown length from the known ones. */
const remaining = computed(() => {
  const known = props.book.chapters.map(c => c.durationSec).filter((d): d is number => !!d);
  if (!known.length) return null;
  const average = known.reduce((a, b) => a + b, 0) / known.length;
  const leftSec = Math.max(0, props.book.chapters.length * average * (1 - progress.value));
  const h = Math.floor(leftSec / 3600);
  const m = Math.floor((leftSec % 3600) / 60);
  return h > 0 ? `~${h} ч ${m} мин` : `~${m} мин`;
});
</script>

<style scoped>
.book-row { background: #1a1a1a; border: 1px solid #2a2a2a; border-radius: 10px; padding: 0.75rem; display: flex; gap: 0.75rem; align-items: flex-start; transition: border-color 0.15s, opacity 0.15s; }
.book-row.active { border-color: #444; }
.book-row.finished { opacity: 0.5; }
.book-row.finished:hover { opacity: 0.75; }

.row-cover-link { flex-shrink: 0; display: block; }
.row-cover { width: 64px; height: 64px; border-radius: 6px; object-fit: cover; display: block; }

.row-body { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 0.5rem; }
.row-top { display: flex; align-items: flex-start; gap: 0.5rem; }
.row-info { flex: 1; min-width: 0; color: inherit; }
a.row-info:hover .row-title { color: #fff; }
.row-title { font-weight: 500; font-size: 0.95rem; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; transition: color 0.1s; }
.row-author { color: #666; font-size: 0.8rem; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; margin-top: 2px; }

.row-actions { display: flex; gap: 0.35rem; flex-shrink: 0; }
.btn-round { background: none; color: #444; border: 1px solid #2a2a2a; width: 30px; height: 30px; border-radius: 50%; cursor: pointer; display: flex; align-items: center; justify-content: center; }
.btn-round.finish:hover, .btn-round.finish.active { color: #4ade80; border-color: #4ade80; }
.btn-round.remove:hover:not(:disabled) { color: #f87171; border-color: #f87171; }

.progress-row { display: flex; flex-direction: column; gap: 3px; }
.progress-track { height: 3px; background: #2a2a2a; border-radius: 2px; overflow: hidden; width: 100%; }
.progress-fill { height: 100%; background: #555; border-radius: 2px; transition: width 0.3s; }
.progress-fill.done { width: 100%; background: #2a2a2a; }
.book-row.active .progress-fill { background: #fff; }
.progress-meta { display: flex; justify-content: space-between; font-size: 0.72rem; color: #444; }
</style>
