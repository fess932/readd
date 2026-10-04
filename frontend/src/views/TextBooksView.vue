<template>
  <main class="page">
    <div class="page-header">
      <h2>Книги</h2>
      <div class="header-actions">
        <button class="btn-toggle" :class="{ active: grouped }" @click="grouped = !grouped" title="По авторам">
          <Users :size="15" />
        </button>
        <button class="btn-primary" @click="uploadDialog?.show()"><Upload :size="15" /> Загрузить книгу</button>
      </div>
    </div>

    <p v-if="isLoading" class="hint">Загрузка...</p>
    <p v-else-if="error" class="error-msg">{{ error.message }}</p>
    <p v-else-if="!books?.length" class="hint">Книг пока нет. Загрузите первую!</p>
    <AuthorGroups v-else :items="books" :grouped="grouped" list-class="books-grid" v-slot="{ item, grouped: underAuthor }">
      <TextBookCard
        :book="item"
        :is-admin="isAdmin"
        :show-author="!underAuthor"
        :deleting="deleteMutation.isPending.value && deleteMutation.variables.value === item.id"
        :save="(edit) => editMutation.mutateAsync({ id: item.id, edit })"
        @delete="confirmDeleteId = item.id"
        @cover="coverMutation.mutate({ id: item.id, file: $event })"
      >
        <template #tts>
          <TtsStatus
            :job="jobByBook.get(item.id)"
            :is-admin="isAdmin"
            @start="ttsCreate.mutate(item.id)"
            @pause="ttsPause.mutate(jobByBook.get(item.id)!.id)"
            @resume="ttsResume.mutate(jobByBook.get(item.id)!.id)"
            @cancel="ttsCancel.mutate(jobByBook.get(item.id)!.id)"
          />
        </template>
      </TextBookCard>
    </AuthorGroups>
  </main>

  <UploadDialog
    ref="uploadDialog"
    title="Загрузить книгу (epub)"
    :pending="uploadMutation.isPending.value"
    :done="uploadDone"
    :progress="uploadProgress"
    :error="uploadError"
    :can-submit="!!file && !!title && !!author"
    @submit="submitUpload"
    @closed="resetUpload"
  >
    <label class="file-drop">
      <BookOpen :size="28" />
      <span>{{ file ? file.name : 'Выберите epub файл' }}</span>
      <input type="file" accept=".epub" @change="onFilePicked" />
    </label>

    <label>Название * <input type="text" v-model="title" required /></label>
    <label>Автор * <input type="text" v-model="author" required /></label>
  </UploadDialog>

  <Confirm
    v-if="bookToDelete"
    title="Удалить книгу?"
    :message="`«${bookToDelete.title}» будет удалена. Это действие нельзя отменить.`"
    confirmLabel="Удалить"
    danger
    :pending="deleteMutation.isPending.value"
    :onconfirm="() => deleteMutation.mutate(bookToDelete!.id)"
    :oncancel="() => confirmDeleteId = null"
  />
</template>

<script setup lang="ts">
import { ref, computed, watch } from 'vue';
import { Upload, Users, BookOpen } from 'lucide-vue-next';
import { useQuery, useMutation, useQueryClient } from '@tanstack/vue-query';
import { api, type TextBook, type TtsJob } from '../api';
import { auth } from '../stores/auth';
import { toast } from '../stores/toasts';
import AuthorGroups from '../components/AuthorGroups.vue';
import Confirm from '../components/Confirm.vue';
import TextBookCard, { type TextBookEdit } from '../components/TextBookCard.vue';
import TtsStatus from '../components/TtsStatus.vue';
import UploadDialog from '../components/UploadDialog.vue';

const queryClient = useQueryClient();
const isAdmin = computed(() => !!auth.user?.isAdmin);
const grouped = ref(false);
const confirmDeleteId = ref<number | null>(null);

const { data: books, isLoading, error } = useQuery({
  queryKey: ['text-books'],
  queryFn: api.textBooks.list,
});

const bookToDelete = computed(() => books.value?.find(b => b.id === confirmDeleteId.value));

function patchCached(id: number, change: Partial<TextBook>) {
  queryClient.setQueryData(['text-books'], (old: TextBook[] | undefined) =>
    old?.map(b => (b.id === id ? { ...b, ...change } : b)) ?? []
  );
}

const onError = (err: Error) => toast('error', err.message);

// ── Voicing jobs ─────────────────────────────────────────────────────────────

const POLL_MS = 2000;

const { data: jobs } = useQuery({
  queryKey: ['tts-jobs'],
  queryFn: api.ttsJobs.list,
  // Decided from the data itself, so polling starts and stops with every update of the list
  refetchInterval: (query) => (query.state.data?.some(j => j.status === 'running') ? POLL_MS : false),
});

/** The latest job of each book. */
const jobByBook = computed(() => {
  const map = new Map<number, TtsJob>();
  for (const job of jobs.value ?? []) {
    const known = map.get(job.textBookId);
    if (!known || job.id > known.id) map.set(job.textBookId, job);
  }
  return map;
});

// A finished job means a new audiobook in the catalogue
const doneCount = computed(() => jobs.value?.filter(j => j.status === 'done').length ?? 0);
watch(doneCount, (now, before) => {
  if (now > before) queryClient.invalidateQueries({ queryKey: ['books'] });
});

function setJobStatus(jobId: number, status: TtsJob['status']) {
  queryClient.setQueryData(['tts-jobs'], (old: TtsJob[] | undefined) =>
    old?.map(j => (j.id === jobId ? { ...j, status, errorMsg: null } : j))
  );
}

const ttsCreate = useMutation({
  mutationFn: (textBookId: number) => api.ttsJobs.create(textBookId),
  onSuccess: (job) => {
    queryClient.setQueryData(['tts-jobs'], (old: TtsJob[] | undefined) => [job, ...(old ?? [])]);
    toast('success', 'Озвучивание запущено');
  },
  onError,
});

const ttsPause = useMutation({
  mutationFn: (jobId: number) => api.ttsJobs.pause(jobId),
  onSuccess: (_, jobId) => setJobStatus(jobId, 'paused'),
  onError,
});

const ttsResume = useMutation({
  mutationFn: (jobId: number) => api.ttsJobs.resume(jobId),
  onSuccess: (_, jobId) => setJobStatus(jobId, 'running'),
  onError,
});

const ttsCancel = useMutation({
  mutationFn: (jobId: number) => api.ttsJobs.cancel(jobId),
  onSuccess: (_, jobId) => {
    queryClient.setQueryData(['tts-jobs'], (old: TtsJob[] | undefined) => old?.filter(j => j.id !== jobId));
    toast('success', 'Задача отменена');
  },
  onError,
});

// ── Per-book actions ─────────────────────────────────────────────────────────

const editMutation = useMutation({
  mutationFn: ({ id, edit }: { id: number; edit: TextBookEdit }) => api.textBooks.patch(id, edit),
  onSuccess: (_, { id, edit }) => {
    patchCached(id, edit);
    toast('success', 'Сохранено');
  },
  onError,
});

const coverMutation = useMutation({
  mutationFn: ({ id, file }: { id: number; file: File }) => api.textBooks.uploadCover(id, file),
  onSuccess: ({ coverPath }, { id }) => {
    patchCached(id, { coverPath });
    toast('success', 'Обложка обновлена');
  },
  onError,
});

const deleteMutation = useMutation({
  mutationFn: (id: number) => api.textBooks.delete(id),
  onSuccess: (_, id) => {
    queryClient.setQueryData(['text-books'], (old: TextBook[] | undefined) => old?.filter(b => b.id !== id) ?? []);
    queryClient.invalidateQueries({ queryKey: ['tts-jobs'] }); // its jobs went with it
    toast('success', 'Книга удалена');
    confirmDeleteId.value = null;
  },
  onError,
});

// ── Upload ───────────────────────────────────────────────────────────────────

const uploadDialog = ref<InstanceType<typeof UploadDialog> | null>(null);
const title = ref('');
const author = ref('');
const file = ref<File | null>(null);
const uploadProgress = ref(0);
const uploadError = ref('');
const uploadDone = ref(false);

function onFilePicked(e: Event) {
  const picked = (e.target as HTMLInputElement).files?.[0];
  if (!picked) return;
  file.value = picked;
  uploadDone.value = false;
  uploadError.value = '';

  // "Автор - Название.epub" fills both fields
  const stem = picked.name.replace(/\.epub$/i, '');
  const dash = stem.match(/^(.+?)\s+[-–—]\s+(.+)$/);
  if (dash) {
    author.value = dash[1].trim();
    title.value = dash[2].trim();
  } else {
    title.value = stem;
  }
}

const uploadMutation = useMutation({
  mutationFn: (fd: FormData) => api.textBooks.upload(fd, (pct) => { uploadProgress.value = pct; }),
  onSuccess: (book) => {
    queryClient.setQueryData(['text-books'], (old: TextBook[] | undefined) => [book, ...(old ?? [])]);
    uploadDone.value = true;
    setTimeout(() => {
      toast('success', `«${book.title}» загружена`);
      uploadDialog.value?.close();
    }, 800);
  },
  onError: (err: Error) => {
    uploadError.value = err.message;
    toast('error', err.message);
  },
});

function submitUpload() {
  if (!file.value) return;
  uploadProgress.value = 0;
  uploadError.value = '';
  uploadDone.value = false;

  const fd = new FormData();
  fd.append('title', title.value);
  fd.append('author', author.value);
  fd.append('file', file.value);
  uploadMutation.mutate(fd);
}

function resetUpload() {
  title.value = '';
  author.value = '';
  file.value = null;
  uploadProgress.value = 0;
  uploadError.value = '';
  uploadDone.value = false;
}
</script>
