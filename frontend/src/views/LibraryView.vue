<template>
  <main class="page narrow">
    <div class="page-header">
      <h2>Моя библиотека</h2>
      <button v-if="activeBooks.length" class="btn-toggle" :class="{ active: grouped }" @click="grouped = !grouped" title="По авторам">
        <Users :size="15" />
      </button>
    </div>

    <p v-if="isLoading" class="hint">Загрузка...</p>
    <p v-else-if="error" class="error-msg">{{ error.message }}</p>
    <p v-else-if="!books?.length" class="hint">
      Библиотека пуста. Добавьте книги из <router-link to="/explore">общей библиотеки</router-link>.
    </p>
    <template v-else>
      <AuthorGroups :items="activeBooks" :grouped="grouped" list-class="books-list" v-slot="{ item, grouped: underAuthor }">
        <LibraryRow
          :book="item"
          :show-author="!underAuthor"
          :removing="isRemoving(item.id)"
          @open="resumeBook(item)"
          @finish="finishMutation.mutate({ bookId: item.id, finished: $event })"
          @remove="confirmRemoveId = item.id"
        />
      </AuthorGroups>

      <section v-if="finishedBooks.length" class="finished-section">
        <h3 class="section-heading">Прочитано</h3>
        <div class="books-list">
          <LibraryRow
            v-for="book in finishedBooks"
            :key="book.id"
            :book="book"
            show-author
            :removing="isRemoving(book.id)"
            @finish="finishMutation.mutate({ bookId: book.id, finished: $event })"
            @remove="confirmRemoveId = book.id"
          />
        </div>
      </section>
    </template>
  </main>

  <Confirm
    v-if="bookToRemove"
    :message="`Убрать «${bookToRemove.title}» из вашей библиотеки?`"
    confirmLabel="Убрать"
    :pending="removeMutation.isPending.value"
    :onconfirm="() => removeMutation.mutate(bookToRemove!.id)"
    :oncancel="() => confirmRemoveId = null"
  />
</template>

<script setup lang="ts">
import { ref, watch, computed } from 'vue';
import { Users } from 'lucide-vue-next';
import { useQuery, useMutation, useQueryClient } from '@tanstack/vue-query';
import { api, type LibraryBook } from '../api';
import { player, playBook, resumeBook, stopPlayer } from '../stores/player';
import { flushOutbox } from '../stores/progressSync';
import { toast } from '../stores/toasts';
import AuthorGroups from '../components/AuthorGroups.vue';
import Confirm from '../components/Confirm.vue';
import LibraryRow from '../components/LibraryRow.vue';

const queryClient = useQueryClient();
const grouped = ref(false);
const confirmRemoveId = ref<number | null>(null);

// Positions this device could not deliver earlier go first, so the answers include them
const { data: books, isLoading, error } = useQuery({
  queryKey: ['library'],
  queryFn: () => flushOutbox().then(api.library.list),
});

const { data: lastProgress } = useQuery({
  queryKey: ['progress', 'last'],
  queryFn: () => flushOutbox().then(api.progress.last),
});

const activeBooks = computed(() => books.value?.filter(b => !b.finishedAt) ?? []);
const finishedBooks = computed(() => books.value?.filter(b => b.finishedAt) ?? []);
const bookToRemove = computed(() => books.value?.find(b => b.id === confirmRemoveId.value));

// Once per page load: put the most recently played book into the player, paused
const restored = ref(false);
watch([books, lastProgress], ([bookList, last]) => {
  if (restored.value || player.book || !bookList || !last) return;
  restored.value = true;
  const book = bookList.find(b => b.id === last.bookId);
  if (!book) return;
  const chapterIdx = book.chapters.findIndex(c => c.filePath === last.chapterPath);
  playBook(book, Math.max(0, chapterIdx), last.positionSec);
  player.playing = false;
}, { immediate: true });

const finishMutation = useMutation({
  mutationFn: ({ bookId, finished }: { bookId: number; finished: boolean }) => api.library.finish(bookId, finished),
  onSuccess: () => queryClient.invalidateQueries({ queryKey: ['library'] }),
  onError: (err: Error) => toast('error', err.message),
});

const removeMutation = useMutation({
  mutationFn: (bookId: number) => api.library.remove(bookId),
  onSuccess: (_, bookId) => {
    queryClient.setQueryData(['library'], (old: LibraryBook[] | undefined) => old?.filter(b => b.id !== bookId) ?? []);
    queryClient.invalidateQueries({ queryKey: ['progress', 'last'] });
    if (player.book?.id === bookId) stopPlayer();
    toast('success', 'Убрано из библиотеки');
    confirmRemoveId.value = null;
  },
  onError: (err: Error) => toast('error', err.message),
});

function isRemoving(bookId: number) {
  return removeMutation.isPending.value && removeMutation.variables.value === bookId;
}
</script>

<style scoped>
:deep(.books-list) { display: flex; flex-direction: column; gap: 0.75rem; }
:deep(.author-group) { margin-bottom: 1.5rem; }
:deep(.author-heading) { font-size: 0.9rem; color: #666; margin-bottom: 0.5rem; }

.finished-section { margin-top: 2rem; }
.section-heading { font-size: 0.8rem; font-weight: 600; text-transform: uppercase; letter-spacing: 0.08em; color: #444; margin-bottom: 0.75rem; }
</style>
