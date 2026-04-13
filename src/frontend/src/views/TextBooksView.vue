<template>
  <main class="page">
    <div class="page-header">
      <h2>Книги</h2>
      <div class="header-actions">
        <button class="btn-toggle" :class="{ active: groupByAuthor }" @click="groupByAuthor = !groupByAuthor" title="По авторам">
          <Users :size="15" />
        </button>
        <button class="btn-primary" @click="openModal"><Upload :size="15" /> Загрузить книгу</button>
      </div>
    </div>

    <p v-if="isLoading" class="hint">Загрузка...</p>
    <p v-else-if="error" class="error">{{ (error as any).message }}</p>
    <p v-else-if="!books?.length" class="hint">Книг пока нет. Загрузите первую!</p>
    <template v-else-if="groupByAuthor">
      <div v-for="group in grouped" :key="group.author" class="author-group">
        <h3 class="author-heading">{{ group.author }} <span class="author-count">{{ group.books.length }}</span></h3>
        <div class="books-grid">
          <BookCard
            v-for="book in group.books" :key="book.id"
            :book="book"
            :job="ttsJobMap.get(book.id)"
            :editing-id="editingId"
            :edit-title="editTitle"
            :edit-author="editAuthor"
            :hovered-cover-id="hoveredCoverId"
            :is-admin="!!auth.user?.isAdmin"
            :delete-pending="deleteMutation.isPending.value && deleteMutation.variables.value === book.id"
            :edit-pending="editMutation.isPending.value"
            @hover="hoveredCoverId = book.id"
            @unhover="hoveredCoverId = null"
            @cover-click="openCoverUpload(book.id)"
            @start-edit="startEdit(book)"
            @cancel-edit="cancelEdit"
            @submit-edit="submitEdit(book.id)"
            @confirm-delete="confirmDeleteId = book.id"
            @tts-start="ttsCreate(book.id)"
            @tts-pause="ttsPause(ttsJobMap.get(book.id)!.id)"
            @tts-resume="ttsResume(ttsJobMap.get(book.id)!.id)"
            @tts-cancel="ttsCancel(ttsJobMap.get(book.id)!.id)"
            @update:edit-title="editTitle = $event"
            @update:edit-author="editAuthor = $event"
          />
        </div>
      </div>
    </template>
    <div v-else class="books-grid">
      <BookCard
        v-for="book in books" :key="book.id"
        :book="book"
        :job="ttsJobMap.get(book.id)"
        :editing-id="editingId"
        :edit-title="editTitle"
        :edit-author="editAuthor"
        :hovered-cover-id="hoveredCoverId"
        :is-admin="!!auth.user?.isAdmin"
        :delete-pending="deleteMutation.isPending.value && deleteMutation.variables.value === book.id"
        :edit-pending="editMutation.isPending.value"
        show-author
        @hover="hoveredCoverId = book.id"
        @unhover="hoveredCoverId = null"
        @cover-click="openCoverUpload(book.id)"
        @start-edit="startEdit(book)"
        @cancel-edit="cancelEdit"
        @submit-edit="submitEdit(book.id)"
        @confirm-delete="confirmDeleteId = book.id"
        @tts-start="ttsCreate(book.id)"
        @tts-pause="ttsPause(ttsJobMap.get(book.id)!.id)"
        @tts-resume="ttsResume(ttsJobMap.get(book.id)!.id)"
        @tts-cancel="ttsCancel(ttsJobMap.get(book.id)!.id)"
        @update:edit-title="editTitle = $event"
        @update:edit-author="editAuthor = $event"
      />
    </div>
  </main>

  <input ref="coverInputEl" type="file" accept="image/*" style="display:none" @change="onCoverFileChange" />

  <dialog ref="dialogEl" class="modal" @close="closeModal" @click="onDialogClick">
    <template v-if="showUploadModal">
      <h3>Загрузить книгу (epub)</h3>
      <form @submit.prevent="submitUpload">
        <label class="file-drop">
          <BookOpen :size="28" />
          <span>{{ uploadFile ? uploadFile.name : 'Выберите epub файл' }}</span>
          <input type="file" accept=".epub" @change="handleFileSelect" />
        </label>

        <label>Название * <input type="text" v-model="uploadTitle" required /></label>
        <label>Автор * <input type="text" v-model="uploadAuthor" required /></label>

        <template v-if="uploadMutation.isPending.value || uploadDone">
          <div class="progress-wrap" :class="{ done: uploadDone }">
            <div class="progress-bar"
              :class="{ indeterminate: uploadMutation.isPending.value && uploadProgress === 100 && !uploadDone }"
              :style="{ width: (uploadDone ? 100 : uploadProgress) + '%' }"
            ></div>
          </div>
          <p class="progress-text">
            <template v-if="uploadDone"><CheckCircle2 :size="13" style="vertical-align: -2px" /> Загружено</template>
            <template v-else-if="uploadProgress === 100">Сохранение на сервере…</template>
            <template v-else>Отправка {{ uploadProgress }}%…</template>
          </p>
        </template>

        <div v-if="uploadError" class="upload-error"><X :size="14" /> {{ uploadError }}</div>

        <div class="modal-actions">
          <button type="button" @click="closeModal" :disabled="uploadMutation.isPending.value">Отмена</button>
          <button type="submit" class="btn-primary"
            :disabled="uploadMutation.isPending.value || uploadDone || !uploadFile || !uploadTitle || !uploadAuthor"
          >
            {{ uploadMutation.isPending.value ? `${uploadProgress}%` : uploadDone ? 'Готово' : 'Загрузить' }}
          </button>
        </div>
      </form>
    </template>
  </dialog>

  <Confirm
    v-if="confirmDeleteId !== null"
    title="Удалить книгу?"
    :message="books?.find(b => b.id === confirmDeleteId) ? `«${books!.find(b => b.id === confirmDeleteId)!.title}» будет удалена. Это действие нельзя отменить.` : ''"
    confirmLabel="Удалить"
    :danger="true"
    :onconfirm="() => deleteMutation.mutate(confirmDeleteId!)"
    :oncancel="() => confirmDeleteId = null"
  />
</template>

<script setup lang="ts">
import { ref, computed, watch, defineComponent, h } from 'vue';
import {
  Upload, Users, Pencil, Check, X as XIcon, ImagePlus, Trash2,
  BookOpen, CheckCircle2, X, Mic, Pause, Play, AlertCircle, CheckCheck, XCircle,
} from 'lucide-vue-next';
import { useQuery, useMutation, useQueryClient } from '@tanstack/vue-query';
import { api, type TextBook, type TtsJob } from '../api';
import { auth } from '../stores/auth';
import { toast } from '../stores/toasts';
import Confirm from '../components/Confirm.vue';

// ── Inline BookCard component ─────────────────────────────────────────────────
const BookCard = defineComponent({
  name: 'BookCard',
  props: {
    book: { type: Object as () => TextBook, required: true },
    job: { type: Object as () => TtsJob | undefined, default: undefined },
    editingId: { type: Number as () => number | null, default: null },
    editTitle: String,
    editAuthor: String,
    hoveredCoverId: { type: Number as () => number | null, default: null },
    isAdmin: Boolean,
    deletePending: Boolean,
    editPending: Boolean,
    showAuthor: Boolean,
  },
  emits: [
    'hover', 'unhover', 'cover-click',
    'start-edit', 'cancel-edit', 'submit-edit', 'confirm-delete',
    'tts-start', 'tts-pause', 'tts-resume', 'tts-cancel',
    'update:editTitle', 'update:editAuthor',
  ],
  setup(props, { emit }) {
    return () => {
      const book = props.book;
      const job = props.job;
      const isEditing = props.editingId === book.id;

      // Cover section
      const coverSection = h('div', {
        class: 'cover-wrap',
        onMouseenter: () => emit('hover'),
        onMouseleave: () => emit('unhover'),
      }, [
        h('img', {
          src: book.coverPath ? `/uploads/${book.coverPath}` : '/placeholder.jpg',
          alt: book.title, class: 'book-cover',
        }),
        props.isAdmin && h('button', {
          class: ['cover-edit-btn', { hidden: props.hoveredCoverId !== book.id }],
          onClick: () => emit('cover-click'), title: 'Заменить обложку',
        }, [h(ImagePlus, { size: 14 })]),
      ]);

      // Info section
      const infoSection = h('div', { class: 'book-info' }, isEditing ? [
        h('input', { class: 'edit-input', value: props.editTitle, placeholder: 'Название', onInput: (e: Event) => emit('update:editTitle', (e.target as HTMLInputElement).value) }),
        h('input', { class: 'edit-input', value: props.editAuthor, placeholder: 'Автор', onInput: (e: Event) => emit('update:editAuthor', (e.target as HTMLInputElement).value) }),
      ] : [
        h('h3', book.title),
        props.showAuthor && h('p', { class: 'author' }, book.author),
        book.fileSize && h('div', { class: 'stats' }, [h('span', formatSize(book.fileSize))]),
      ]);

      // TTS section
      const ttsSection = h('div', { class: 'tts-section' }, buildTtsSection(job, props.isAdmin, emit));

      // Actions section
      const actionsSection = h('div', { class: 'book-actions' }, isEditing ? [
        h('button', { class: 'btn-save', onClick: () => emit('submit-edit'), disabled: props.editPending }, [h(Check, { size: 14 })]),
        h('button', { class: 'btn-cancel-edit', onClick: () => emit('cancel-edit') }, [h(XIcon, { size: 14 })]),
      ] : props.isAdmin ? [
        h('button', { class: 'btn-edit', onClick: () => emit('start-edit') }, [h(Pencil, { size: 14 })]),
        h('button', { class: 'btn-delete', onClick: () => emit('confirm-delete'), disabled: props.deletePending }, [h(Trash2, { size: 14 })]),
      ] : []);

      return h('div', { class: 'book-card' }, [coverSection, infoSection, ttsSection, actionsSection]);
    };
  },
});

function buildTtsSection(
  job: TtsJob | undefined,
  isAdmin: boolean,
  emit: (event: string) => void,
) {
  if (!job) {
    if (!isAdmin) return [];
    return [
      h('button', { class: 'btn-tts-start', onClick: () => emit('tts-start') }, [
        h(Mic, { size: 13 }), ' Озвучить',
      ]),
    ];
  }

  const pct = job.totalChunks > 0
    ? Math.round((job.doneChunks / job.totalChunks) * 100)
    : 0;
  const label = `${job.doneChunks} / ${job.totalChunks}`;

  if (job.status === 'running') {
    return [
      h('div', { class: 'tts-progress-row' }, [
        h('span', { class: 'tts-label running' }, [h(Mic, { size: 11 }), ` ${label}`]),
        isAdmin && h('button', { class: 'btn-tts-ctrl', onClick: () => emit('tts-pause'), title: 'Пауза' }, [h(Pause, { size: 13 })]),
      ]),
      h('div', { class: 'tts-bar-wrap' }, [
        h('div', { class: 'tts-bar', style: { width: pct + '%' } }),
      ]),
    ];
  }

  if (job.status === 'paused') {
    return [
      h('div', { class: 'tts-progress-row' }, [
        h('span', { class: 'tts-label paused' }, [h(Pause, { size: 11 }), ` ${label}`]),
        isAdmin && h('button', { class: 'btn-tts-ctrl', onClick: () => emit('tts-resume'), title: 'Продолжить' }, [h(Play, { size: 13 })]),
        isAdmin && h('button', { class: 'btn-tts-ctrl danger', onClick: () => emit('tts-cancel'), title: 'Отменить' }, [h(XCircle, { size: 13 })]),
      ]),
      h('div', { class: 'tts-bar-wrap' }, [
        h('div', { class: 'tts-bar paused', style: { width: pct + '%' } }),
      ]),
    ];
  }

  if (job.status === 'done') {
    return [
      h('div', { class: 'tts-label done' }, [h(CheckCheck, { size: 12 }), ' Аудиокнига готова']),
    ];
  }

  if (job.status === 'failed') {
    return [
      h('div', { class: 'tts-label failed' }, [h(AlertCircle, { size: 12 }), ' Ошибка озвучки']),
      isAdmin && h('button', { class: 'btn-tts-start', onClick: () => emit('tts-start') }, [
        h(Mic, { size: 13 }), ' Повторить',
      ]),
    ];
  }

  return [];
}

function formatSize(bytes: number): string {
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(0)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

// ── State ─────────────────────────────────────────────────────────────────────

const queryClient = useQueryClient();
const confirmDeleteId = ref<number | null>(null);
const groupByAuthor = ref(false);
const hoveredCoverId = ref<number | null>(null);

// ── TTS jobs ─────────────────────────────────────────────────────────────────

const hasRunningJobs = ref(false);

const { data: ttsJobsList } = useQuery({
  queryKey: ['tts-jobs'],
  queryFn: api.ttsJobs.list,
  refetchInterval: () => hasRunningJobs.value ? 2000 : false,
});

watch(ttsJobsList, (jobs) => {
  hasRunningJobs.value = jobs?.some(j => j.status === 'running') ?? false;
});

const ttsJobMap = computed(() => {
  const map = new Map<number, TtsJob>();
  for (const job of ttsJobsList.value ?? []) {
    // Keep only the latest job per book
    if (!map.has(job.textBookId) || job.id > map.get(job.textBookId)!.id) {
      map.set(job.textBookId, job);
    }
  }
  return map;
});

const ttsCreateMutation = useMutation({
  mutationFn: (textBookId: number) => api.ttsJobs.create(textBookId),
  onSuccess: (job) => {
    queryClient.setQueryData(['tts-jobs'], (old: TtsJob[] | undefined) => [job, ...(old ?? [])]);
    hasRunningJobs.value = true;
    toast('success', 'Озвучивание запущено');
  },
  onError: (err: any) => toast('error', err.message),
});

const ttsPauseMutation = useMutation({
  mutationFn: (jobId: number) => api.ttsJobs.pause(jobId),
  onSuccess: (_, jobId) => {
    queryClient.setQueryData(['tts-jobs'], (old: TtsJob[] | undefined) =>
      old?.map(j => j.id === jobId ? { ...j, status: 'paused' as const } : j)
    );
  },
  onError: (err: any) => toast('error', err.message),
});

const ttsResumeMutation = useMutation({
  mutationFn: (jobId: number) => api.ttsJobs.resume(jobId),
  onSuccess: (_, jobId) => {
    queryClient.setQueryData(['tts-jobs'], (old: TtsJob[] | undefined) =>
      old?.map(j => j.id === jobId ? { ...j, status: 'running' as const } : j)
    );
    hasRunningJobs.value = true;
  },
  onError: (err: any) => toast('error', err.message),
});

const ttsCancelMutation = useMutation({
  mutationFn: (jobId: number) => api.ttsJobs.cancel(jobId),
  onSuccess: (_, jobId) => {
    queryClient.setQueryData(['tts-jobs'], (old: TtsJob[] | undefined) =>
      old?.filter(j => j.id !== jobId)
    );
    toast('success', 'Задача отменена');
  },
  onError: (err: any) => toast('error', err.message),
});

function ttsCreate(textBookId: number) { ttsCreateMutation.mutate(textBookId); }
function ttsPause(jobId: number) { ttsPauseMutation.mutate(jobId); }
function ttsResume(jobId: number) { ttsResumeMutation.mutate(jobId); }
function ttsCancel(jobId: number) { ttsCancelMutation.mutate(jobId); }

// ── Cover upload ──────────────────────────────────────────────────────────────

const coverInputEl = ref<HTMLInputElement | null>(null);
const coverTargetId = ref<number | null>(null);

function openCoverUpload(bookId: number) {
  coverTargetId.value = bookId;
  coverInputEl.value?.click();
}

const coverMutation = useMutation({
  mutationFn: ({ id, file }: { id: number; file: File }) => api.textBooks.uploadCover(id, file),
  onSuccess: ({ coverPath }, { id }) => {
    queryClient.setQueryData(['text-books'], (old: TextBook[] | undefined) =>
      old?.map(b => b.id === id ? { ...b, coverPath } : b) ?? []
    );
    toast('success', 'Обложка обновлена');
  },
  onError: (err: any) => toast('error', err.message),
});

function onCoverFileChange(e: Event) {
  const file = (e.target as HTMLInputElement).files?.[0];
  if (!file || coverTargetId.value === null) return;
  coverMutation.mutate({ id: coverTargetId.value, file });
  (e.target as HTMLInputElement).value = '';
}

// ── Edit ──────────────────────────────────────────────────────────────────────

const editingId = ref<number | null>(null);
const editAuthor = ref('');
const editTitle = ref('');

function startEdit(book: TextBook) {
  editingId.value = book.id;
  editAuthor.value = book.author;
  editTitle.value = book.title;
}
function cancelEdit() { editingId.value = null; }

const editMutation = useMutation({
  mutationFn: ({ id, body }: { id: number; body: { title?: string; author?: string } }) =>
    api.textBooks.patch(id, body),
  onSuccess: (_, { id, body }) => {
    queryClient.setQueryData(['text-books'], (old: TextBook[] | undefined) =>
      old?.map(b => b.id === id ? { ...b, ...body } : b) ?? []
    );
    editingId.value = null;
    toast('success', 'Сохранено');
  },
  onError: (err: any) => toast('error', err.message),
});

function submitEdit(id: number) {
  editMutation.mutate({ id, body: { author: editAuthor.value, title: editTitle.value } });
}

const grouped = computed(() => {
  const map = new Map<string, TextBook[]>();
  for (const book of books.value ?? []) {
    if (!map.has(book.author)) map.set(book.author, []);
    map.get(book.author)!.push(book);
  }
  return [...map.entries()]
    .map(([author, books]) => ({ author, books }))
    .sort((a, b) => a.author.localeCompare(b.author, 'ru'));
});

const { data: books, isLoading, error } = useQuery({
  queryKey: ['text-books'],
  queryFn: api.textBooks.list,
});

// ── Delete ────────────────────────────────────────────────────────────────────

const deleteMutation = useMutation({
  mutationFn: (id: number) => api.textBooks.delete(id),
  onSuccess: (_, id) => {
    queryClient.setQueryData(['text-books'], (old: TextBook[] | undefined) => old?.filter(b => b.id !== id) ?? []);
    if (editingId.value === id) editingId.value = null;
    toast('success', 'Книга удалена');
    confirmDeleteId.value = null;
  },
  onError: (err: any) => toast('error', err.message),
});

// ── Upload ────────────────────────────────────────────────────────────────────

const dialogEl = ref<HTMLDialogElement | null>(null);
const showUploadModal = ref(false);
const uploadTitle = ref('');
const uploadAuthor = ref('');
const uploadFile = ref<File | null>(null);
const uploadProgress = ref(0);
const uploadError = ref('');
const uploadDone = ref(false);

const uploadMutation = useMutation({
  mutationFn: async (fd: FormData) => api.textBooks.upload(fd, (pct) => { uploadProgress.value = pct; }),
  onSuccess: (book) => {
    queryClient.setQueryData(['text-books'], (old: TextBook[] | undefined) => [book, ...(old ?? [])]);
    uploadDone.value = true;
    setTimeout(() => { toast('success', `«${book.title}» загружена`); closeModal(); }, 800);
  },
  onError: (err: any) => {
    uploadError.value = err.message;
    toast('error', err.message);
  },
});

function handleFileSelect(e: Event) {
  const file = (e.target as HTMLInputElement).files?.[0];
  if (!file) return;
  uploadFile.value = file;
  uploadDone.value = false;
  uploadError.value = '';
  const stem = file.name.replace(/\.epub$/i, '');
  const dash = stem.match(/^(.+?)\s+[-–—]\s+(.+)$/);
  if (dash) { uploadAuthor.value = dash[1].trim(); uploadTitle.value = dash[2].trim(); }
  else { uploadTitle.value = stem; }
}

async function submitUpload() {
  if (!uploadFile.value || !uploadTitle.value || !uploadAuthor.value) return;
  uploadProgress.value = 0;
  uploadError.value = '';
  uploadDone.value = false;
  const fd = new FormData();
  fd.append('title', uploadTitle.value);
  fd.append('author', uploadAuthor.value);
  fd.append('file', uploadFile.value);
  uploadMutation.mutate(fd);
}

function openModal() { showUploadModal.value = true; dialogEl.value?.showModal(); }

function closeModal() {
  if (uploadMutation.isPending.value) return;
  dialogEl.value?.close();
  showUploadModal.value = false;
  uploadTitle.value = ''; uploadAuthor.value = ''; uploadFile.value = null;
  uploadProgress.value = 0; uploadError.value = ''; uploadDone.value = false;
}

function onDialogClick(e: MouseEvent) { if (e.target === dialogEl.value) closeModal(); }
</script>

<style scoped>
.page { max-width: 960px; margin: 0 auto; padding: 1.5rem; }
.page-header { display: flex; align-items: center; justify-content: space-between; margin-bottom: 1.5rem; }
h2 { font-size: 1.4rem; font-weight: 700; }
.header-actions { display: flex; align-items: center; gap: 0.5rem; }
.btn-toggle { display: flex; align-items: center; justify-content: center; background: none; border: 1px solid #2a2a2a; color: #555; padding: 0.45rem 0.6rem; border-radius: 8px; cursor: pointer; }
.btn-toggle:hover { color: #fff; border-color: #444; }
.btn-toggle.active { background: #2a2a2a; color: #fff; border-color: #444; }
.author-group { margin-bottom: 2rem; }
.author-heading { font-size: 1rem; font-weight: 600; color: #888; margin-bottom: 0.75rem; display: flex; align-items: center; gap: 0.5rem; }
.author-count { font-size: 0.75rem; font-weight: 400; color: #444; background: #1a1a1a; border: 1px solid #2a2a2a; padding: 1px 7px; border-radius: 10px; }

.btn-primary { display: flex; align-items: center; gap: 0.4rem; background: #fff; color: #000; border: none; padding: 0.5rem 1rem; border-radius: 8px; font-weight: 600; cursor: pointer; font-size: 0.9rem; }
.btn-primary:hover { background: #e5e5e5; }
.btn-primary:disabled { opacity: 0.4; cursor: not-allowed; }

.hint { color: #555; text-align: center; padding: 3rem 0; }
.error { color: #f87171; font-size: 0.85rem; }

.books-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(200px, 1fr)); gap: 1rem; }

/* BookCard styles (applied to elements rendered by the inline component) */
:deep(.book-card) { background: #1a1a1a; border: 1px solid #2a2a2a; border-radius: 10px; overflow: hidden; display: flex; flex-direction: column; }
:deep(.cover-wrap) { position: relative; display: block; }
:deep(.book-cover) { width: 100%; aspect-ratio: 1; object-fit: cover; display: block; }
:deep(.cover-edit-btn) { position: absolute; bottom: 6px; right: 6px; background: rgba(0,0,0,0.75); color: #ddd; border: none; border-radius: 6px; padding: 0.3rem; cursor: pointer; display: flex; align-items: center; z-index: 2; transition: opacity 0.15s, background 0.15s; }
:deep(.cover-edit-btn.hidden) { opacity: 0; pointer-events: none; }
:deep(.cover-edit-btn:hover) { color: #fff; background: rgba(0,0,0,0.95); }
:deep(.book-info) { padding: 0.75rem; flex: 1; }
:deep(.book-info h3) { font-size: 0.95rem; font-weight: 600; margin-bottom: 0.25rem; line-height: 1.3; }
:deep(.author) { color: #888; font-size: 0.85rem; }
:deep(.stats) { display: flex; flex-wrap: wrap; gap: 0.3rem 0.5rem; margin-top: 0.35rem; }
:deep(.stats span) { font-size: 0.75rem; color: #555; background: #222; padding: 1px 6px; border-radius: 4px; }
:deep(.book-actions) { padding: 0.4rem 0.75rem 0.6rem; display: flex; gap: 0.5rem; justify-content: flex-end; min-height: 32px; }
:deep(.btn-edit) { display: flex; align-items: center; justify-content: center; background: none; color: #555; border: 1px solid #2a2a2a; padding: 0.35rem 0.6rem; border-radius: 6px; cursor: pointer; }
:deep(.btn-edit:hover) { color: #fff; border-color: #444; }
:deep(.btn-save) { display: flex; align-items: center; justify-content: center; background: #1a3a1a; color: #4ade80; border: none; padding: 0.35rem 0.6rem; border-radius: 6px; cursor: pointer; flex: 1; }
:deep(.btn-save:hover:not(:disabled)) { background: #1f4a1f; }
:deep(.btn-cancel-edit) { display: flex; align-items: center; justify-content: center; background: none; color: #555; border: 1px solid #2a2a2a; padding: 0.35rem 0.6rem; border-radius: 6px; cursor: pointer; }
:deep(.btn-cancel-edit:hover) { color: #fff; border-color: #444; }
:deep(.btn-delete) { display: flex; align-items: center; justify-content: center; background: #3a1a1a; color: #f87171; border: none; padding: 0.35rem 0.6rem; border-radius: 6px; cursor: pointer; }
:deep(.btn-delete:hover:not(:disabled)) { background: #4a1a1a; }
:deep(button:disabled) { opacity: 0.5; cursor: not-allowed; }
:deep(.edit-input) { background: #0f0f0f; border: 1px solid #333; border-radius: 5px; padding: 0.3rem 0.5rem; color: #fff; font-size: 0.82rem; width: 100%; margin-bottom: 0.3rem; outline: none; }
:deep(.edit-input:focus) { border-color: #555; }

/* TTS section */
:deep(.tts-section) { padding: 0.4rem 0.75rem; border-top: 1px solid #1e1e1e; }
:deep(.tts-progress-row) { display: flex; align-items: center; gap: 0.4rem; margin-bottom: 0.3rem; }
:deep(.tts-label) { display: flex; align-items: center; gap: 0.3rem; font-size: 0.75rem; flex: 1; }
:deep(.tts-label.running) { color: #60a5fa; }
:deep(.tts-label.paused) { color: #fbbf24; }
:deep(.tts-label.done) { color: #4ade80; padding: 0.2rem 0; }
:deep(.tts-label.failed) { color: #f87171; padding: 0.2rem 0; }
:deep(.tts-bar-wrap) { height: 3px; background: #2a2a2a; border-radius: 2px; overflow: hidden; }
:deep(.tts-bar) { height: 100%; background: #60a5fa; transition: width 0.4s ease; border-radius: 2px; }
:deep(.tts-bar.paused) { background: #fbbf24; }
:deep(.btn-tts-start) { display: flex; align-items: center; gap: 0.3rem; width: 100%; justify-content: center; background: #1e2a3a; color: #60a5fa; border: 1px solid #2a3a4a; padding: 0.35rem 0.5rem; border-radius: 6px; cursor: pointer; font-size: 0.8rem; margin-top: 0.1rem; }
:deep(.btn-tts-start:hover) { background: #243040; border-color: #3a4a5a; }
:deep(.btn-tts-ctrl) { display: flex; align-items: center; justify-content: center; background: none; color: #555; border: 1px solid #2a2a2a; padding: 0.2rem 0.4rem; border-radius: 5px; cursor: pointer; }
:deep(.btn-tts-ctrl:hover) { color: #fff; border-color: #444; }
:deep(.btn-tts-ctrl.danger) { color: #f87171; border-color: #3a1a1a; }
:deep(.btn-tts-ctrl.danger:hover) { background: #3a1a1a; }

/* Modal */
.modal { background: #1a1a1a; border: 1px solid #2a2a2a; border-radius: 12px; padding: 1.5rem; width: min(420px, calc(100vw - 2rem)); color: #fff; position: fixed; top: 50%; left: 50%; transform: translate(-50%, -50%); margin: 0; }
.modal::backdrop { background: rgba(0,0,0,0.7); }
.modal h3 { margin-bottom: 1rem; font-size: 1.1rem; }
.file-drop { display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 0.5rem; border: 2px dashed #333; border-radius: 8px; padding: 1.5rem; cursor: pointer; margin-bottom: 1rem; color: #888; font-size: 0.9rem; transition: border-color 0.15s; }
.file-drop:hover { border-color: #555; color: #aaa; }
.file-drop input { display: none; }
.modal label { display: flex; flex-direction: column; gap: 0.3rem; margin-bottom: 0.75rem; font-size: 0.85rem; color: #888; }
.modal input[type="text"] { background: #0f0f0f; border: 1px solid #333; border-radius: 6px; padding: 0.5rem 0.75rem; color: #fff; font-size: 0.95rem; outline: none; }
.modal input[type="text"]:focus { border-color: #555; }
.progress-wrap { height: 4px; background: #2a2a2a; border-radius: 2px; margin-bottom: 0.4rem; overflow: hidden; }
.progress-bar { height: 100%; background: #fff; transition: width 0.15s ease; }
.progress-wrap.done .progress-bar { background: #4ade80; }
.progress-bar.indeterminate { width: 40% !important; background: #888; animation: indeterminate 1.2s ease-in-out infinite; }
@keyframes indeterminate { 0% { margin-left: -40%; } 100% { margin-left: 100%; } }
.progress-text { font-size: 0.8rem; color: #888; margin-bottom: 0.75rem; }
.progress-wrap.done + .progress-text { color: #4ade80; }
.upload-error { display: flex; align-items: center; gap: 0.4rem; background: #3a1a1a; color: #f87171; border-radius: 6px; padding: 0.5rem 0.75rem; font-size: 0.85rem; margin-bottom: 0.75rem; }
.modal-actions { display: flex; gap: 0.75rem; margin-top: 1rem; justify-content: flex-end; }
.modal-actions button { background: #2a2a2a; color: #fff; border: none; padding: 0.5rem 1rem; border-radius: 6px; cursor: pointer; font-size: 0.9rem; }
.modal-actions .btn-primary { background: #fff; color: #000; font-weight: 600; }
</style>
