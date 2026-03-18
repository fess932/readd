<template>
  <main class="page">
    <div class="page-header">
      <h2>Моя библиотека</h2>
    </div>

    <p v-if="isLoading" class="hint">Загрузка...</p>
    <p v-else-if="error" class="error-msg">{{ (error as any).message }}</p>
    <p v-else-if="!books?.length" class="hint">
      Библиотека пуста. Добавьте книги из <router-link to="/explore">общей библиотеки</router-link>.
    </p>
    <div v-else class="books-list">
      <div
        v-for="book in books"
        :key="book.id"
        class="book-row"
        :class="{ active: isActive(book) }"
      >
        <router-link :to="`/book/${book.id}`" class="row-cover-link" tabindex="-1" @click="resume(book)">
          <img :src="book.coverPath ? `/uploads/${book.coverPath}` : '/placeholder.jpg'" :alt="book.title" class="row-cover" />
        </router-link>

        <div class="row-body">
          <div class="row-top">
            <router-link :to="`/book/${book.id}`" class="row-info" @click="resume(book)">
              <p class="row-title">{{ book.title }}</p>
              <p class="row-author">{{ book.author }}{{ book.narrator ? ` · ${book.narrator}` : '' }}</p>
            </router-link>
            <div class="row-actions">
              <button
                class="btn-remove"
                @click="confirmRemoveId = book.id"
                :disabled="removeMutation.isPending.value && removeMutation.variables.value === book.id"
                title="Убрать из библиотеки"
              ><X :size="11" /></button>
            </div>
          </div>

          <div class="progress-row">
            <div class="progress-track">
              <div class="progress-fill" :style="{ width: liveProgress(book) * 100 + '%' }"></div>
            </div>
            <div class="progress-meta">
              <span v-if="book.chapters.length > 1" class="chapters-info">
                <template v-if="book.progress">
                  Гл. {{ book.chapters.findIndex(c => c.filePath === book.progress!.chapterPath) + 1 }} / {{ book.chapters.length }}
                </template>
                <template v-else>{{ book.chapters.length }} глав</template>
              </span>
              <span v-if="remainingTime(book)" class="remaining">осталось {{ remainingTime(book) }}</span>
            </div>
          </div>
        </div>
      </div>
    </div>
  </main>

  <Confirm
    v-if="confirmRemoveId !== null"
    :message="books?.find(b => b.id === confirmRemoveId) ? `Убрать «${books!.find(b => b.id === confirmRemoveId)!.title}» из вашей библиотеки?` : ''"
    confirmLabel="Убрать"
    :onconfirm="() => doRemove(confirmRemoveId!)"
    :oncancel="() => confirmRemoveId = null"
  />
</template>

<script setup lang="ts">
import { ref, watch } from 'vue';
import { X } from 'lucide-vue-next';
import { useQuery, useMutation, useQueryClient } from '@tanstack/vue-query';
import { api, type LibraryBook } from '../api';
import { player, playBook } from '../stores/player';
import { toast } from '../stores/toasts';
import Confirm from '../components/Confirm.vue';

const queryClient = useQueryClient();
const confirmRemoveId = ref<number | null>(null);

const { data: books, isLoading, error } = useQuery({
  queryKey: ['library'],
  queryFn: api.library.list,
});

const { data: lastProgress } = useQuery({
  queryKey: ['progress', 'last'],
  queryFn: api.progress.last,
});

// Auto-restore last book on first load
const restored = ref(false);
watch([books, lastProgress], ([bookList, last]) => {
  if (restored.value || player.book || !bookList || !last) return;
  restored.value = true;
  const book = bookList.find(b => b.id === last.bookId);
  if (book) {
    const chapterIdx = book.chapters.findIndex(c => c.filePath === last.chapterPath);
    playBook(book, Math.max(0, chapterIdx), last.positionSec);
    player.playing = false;
  }
});

const removeMutation = useMutation({
  mutationFn: (bookId: number) => api.library.remove(bookId),
  onSuccess: (_, bookId) => {
    queryClient.setQueryData(['library'], (old: LibraryBook[] | undefined) =>
      old?.filter(b => b.id !== bookId) ?? []
    );
    if (player.book?.id === bookId) player.book = null;
    toast('success', 'Убрано из библиотеки');
    confirmRemoveId.value = null;
  },
  onError: (err: any) => {
    toast('error', err.message);
  },
});

function doRemove(bookId: number) {
  removeMutation.mutate(bookId);
}

function resume(book: LibraryBook) {
  const chapterIdx = book.progress
    ? book.chapters.findIndex(c => c.filePath === book.progress!.chapterPath)
    : 0;
  playBook(book, Math.max(0, chapterIdx), book.progress?.positionSec ?? 0);
}

function isActive(book: LibraryBook) {
  return player.book?.id === book.id;
}

function bookProgress(book: LibraryBook): number {
  if (!book.chapters.length || !book.progress) return 0;
  const prog = book.progress;
  const chIdx = book.chapters.findIndex(c => c.filePath === prog.chapterPath);
  if (chIdx < 0) return 0;
  const chDur = book.chapters[chIdx].durationSec;
  const chProgress = chDur && chDur > 0 ? prog.positionSec / chDur : 0;
  return (chIdx + chProgress) / book.chapters.length;
}

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


</script>

<style scoped>
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

.row-cover-link { flex-shrink: 0; display: block; }
.row-cover { width: 64px; height: 64px; border-radius: 6px; object-fit: cover; display: block; }
.row-cover.placeholder { background: linear-gradient(135deg, #2a2a2a, #1a1a1a); }

.row-body { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 0.5rem; }

.row-top { display: flex; align-items: flex-start; gap: 0.5rem; }
.row-info { flex: 1; min-width: 0; text-decoration: none; color: inherit; }
.row-info:hover .row-title { color: #fff; }
.row-title { font-weight: 500; font-size: 0.95rem; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; transition: color 0.1s; }
.row-author { color: #666; font-size: 0.8rem; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; margin-top: 2px; }

.row-actions { display: flex; gap: 0.35rem; flex-shrink: 0; }
.btn-remove { background: none; color: #444; border: 1px solid #2a2a2a; width: 30px; height: 30px; border-radius: 50%; cursor: pointer; font-size: 0.65rem; display: flex; align-items: center; justify-content: center; }
.btn-remove:hover:not(:disabled) { color: #f87171; border-color: #f87171; }
button:disabled { opacity: 0.4; cursor: not-allowed; }

.progress-row { display: flex; flex-direction: column; gap: 3px; }
.progress-track { height: 3px; background: #2a2a2a; border-radius: 2px; position: relative; overflow: hidden; width: 100%; }
.progress-fill { height: 100%; background: #555; border-radius: 2px; transition: width 0.3s; }
.book-row.active .progress-fill { background: #fff; }

.progress-meta { display: flex; justify-content: space-between; font-size: 0.72rem; }
.chapters-info { color: #444; }
.remaining { color: #444; }
</style>
