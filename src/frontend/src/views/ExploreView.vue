<template>
  <main class="page">
    <div class="page-header">
      <h2>Общая библиотека</h2>
      <button class="btn-primary" @click="openModal">+ Загрузить книгу</button>
    </div>

    <p v-if="isLoading" class="hint">Загрузка...</p>
    <p v-else-if="error" class="error">{{ (error as any).message }}</p>
    <p v-else-if="!books?.length" class="hint">Книг пока нет. Загрузите первую!</p>
    <div v-else class="books-grid">
      <div v-for="book in books" :key="book.id" class="book-card">
        <img v-if="book.coverPath" :src="`/uploads/${book.coverPath}`" :alt="book.title" class="book-cover" />
        <div v-else class="book-cover placeholder"></div>
        <div class="book-info">
          <h3>{{ book.title }}</h3>
          <p class="author">{{ book.author }}</p>
          <p v-if="book.narrator" class="meta">Читает: {{ book.narrator }}</p>
          <div class="stats">
            <span v-if="book.chaptersCount > 0">{{ book.chaptersCount }} {{ pluralChapters(book.chaptersCount) }}</span>
            <span v-if="formatDuration(book.totalSec)">{{ formatDuration(book.totalSec) }}</span>
          </div>
        </div>
        <div class="book-actions">
          <button class="btn-add" @click="addMutation.mutate(book.id)" :disabled="addMutation.isPending.value && addMutation.variables.value === book.id">
            {{ addMutation.isPending.value && addMutation.variables.value === book.id ? '...' : '+ В моё' }}
          </button>
          <button v-if="auth.user?.isAdmin" class="btn-delete" @click="confirmDeleteId = book.id" :disabled="deleteMutation.isPending.value && deleteMutation.variables.value === book.id">
            {{ deleteMutation.isPending.value && deleteMutation.variables.value === book.id ? '...' : 'Удалить' }}
          </button>
        </div>
      </div>
    </div>
  </main>

  <!-- Upload dialog -->
  <dialog ref="dialogEl" class="modal" @close="closeModal" @click="onDialogClick">
    <template v-if="showUploadModal">
      <h3>Загрузить книгу</h3>
      <form @submit.prevent="submitUpload">
        <label class="folder-drop">
          <span>Выберите папку с книгой</span>
          <input type="file" webkitdirectory @change="handleFolderSelect" />
        </label>

        <div v-if="uploadFiles.length > 0" class="folder-summary">
          <img v-if="coverPreview" :src="coverPreview" alt="обложка" class="cover-preview" />
          <div v-else class="cover-preview placeholder"></div>
          <div class="folder-stats">
            <p>{{ uploadFiles.length }} файлов</p>
            <p class="hint-small">{{ audioCount() }} аудио</p>
          </div>
        </div>

        <label>Название * <input type="text" v-model="uploadTitle" required /></label>
        <label>Автор * <input type="text" v-model="uploadAuthor" required /></label>
        <label>Диктор <input type="text" v-model="uploadNarrator" /></label>

        <template v-if="uploadMutation.isPending.value || uploadDone">
          <div class="progress-wrap" :class="{ done: uploadDone }">
            <div
              class="progress-bar"
              :class="{ indeterminate: uploadMutation.isPending.value && uploadProgress === 100 && !uploadDone }"
              :style="{ width: (uploadDone ? 100 : uploadProgress) + '%' }"
            ></div>
          </div>
          <p class="progress-text">
            <template v-if="uploadDone">✓ Загружено</template>
            <template v-else-if="uploadProgress === 100">Сохранение на сервере…</template>
            <template v-else>Отправка {{ uploadProgress }}%…</template>
          </p>
        </template>

        <div v-if="uploadError" class="upload-error">
          <span>✕</span> {{ uploadError }}
        </div>

        <div class="modal-actions">
          <button type="button" @click="closeModal" :disabled="uploadMutation.isPending.value">Отмена</button>
          <button
            type="submit"
            class="btn-primary"
            :disabled="uploadMutation.isPending.value || uploadDone || !uploadFiles.length || !uploadTitle || !uploadAuthor"
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
    :message="books?.find(b => b.id === confirmDeleteId) ? `«${books!.find(b => b.id === confirmDeleteId)!.title}» будет удалена из общей библиотеки. Это действие нельзя отменить.` : ''"
    confirmLabel="Удалить"
    :danger="true"
    :onconfirm="() => deleteMutation.mutate(confirmDeleteId!)"
    :oncancel="() => confirmDeleteId = null"
  />
</template>

<script setup lang="ts">
import { ref } from 'vue';
import { useQuery, useMutation, useQueryClient } from '@tanstack/vue-query';
import { api, type Book } from '../api';
import { auth } from '../stores/auth';
import { toast } from '../stores/toasts';
import Confirm from '../components/Confirm.vue';

const queryClient = useQueryClient();
const confirmDeleteId = ref<number | null>(null);

const { data: books, isLoading, error } = useQuery({
  queryKey: ['books'],
  queryFn: api.books.list,
});

// — Add to library —
const addMutation = useMutation({
  mutationFn: (bookId: number) => api.library.add(bookId),
  onSuccess: () => toast('success', 'Добавлено в вашу библиотеку'),
  onError: (err: any) => toast('error', err.message),
});

// — Delete book —
const deleteMutation = useMutation({
  mutationFn: (bookId: number) => api.books.delete(bookId),
  onSuccess: (_, bookId) => {
    queryClient.setQueryData(['books'], (old: Book[] | undefined) => old?.filter(b => b.id !== bookId) ?? []);
    toast('success', 'Книга удалена');
    confirmDeleteId.value = null;
  },
  onError: (err: any) => toast('error', err.message),
});

// — Upload book —
const dialogEl = ref<HTMLDialogElement | null>(null);
const showUploadModal = ref(false);
const uploadTitle = ref('');
const uploadAuthor = ref('');
const uploadNarrator = ref('');
const uploadFiles = ref<File[]>([]);
const coverPreview = ref<string | null>(null);
const uploadProgress = ref(0);
const uploadError = ref('');
const uploadDone = ref(false);

const AUDIO_EXT = /\.(mp3|m4a|m4b|ogg|flac|wav|aac|opus)$/i;
const IMAGE_EXT = /\.(jpg|jpeg|png|webp|avif)$/i;
const COVER_NAME = /^(cover|folder|front|artwork|thumb)/i;

const uploadMutation = useMutation({
  mutationFn: async (fd: FormData) => {
    return api.books.upload(fd, (pct) => { uploadProgress.value = pct; });
  },
  onSuccess: (book) => {
    queryClient.setQueryData(['books'], (old: Book[] | undefined) => [book, ...(old ?? [])]);
    uploadDone.value = true;
    setTimeout(() => { toast('success', `«${book.title}» загружена`); closeModal(); }, 800);
  },
  onError: (err: any) => {
    uploadError.value = err.message;
    toast('error', err.message);
  },
});

function parseFolderName(name: string): { title: string; author: string } {
  const clean = name.replace(/\s*[\(\[][^\)\]]*[\)\]]/g, '').trim();
  const dash = clean.match(/^(.+?)\s+[-–—]\s+(.+)$/);
  if (dash) return { author: dash[1].trim(), title: dash[2].trim() };
  return { title: clean, author: '' };
}

function handleFolderSelect(e: Event) {
  const input = e.currentTarget as HTMLInputElement;
  const all = Array.from(input.files ?? []);
  if (all.length === 0) return;
  uploadFiles.value = all;
  uploadDone.value = false;
  uploadError.value = '';

  const folderName = all[0].webkitRelativePath.split('/')[0];
  const parsed = parseFolderName(folderName);
  uploadTitle.value = parsed.title;
  uploadAuthor.value = parsed.author;

  const coverFile =
    all.find(f => IMAGE_EXT.test(f.name) && COVER_NAME.test(f.name)) ??
    all.find(f => IMAGE_EXT.test(f.name));
  coverPreview.value = coverFile ? URL.createObjectURL(coverFile) : null;
}

function audioCount() {
  return uploadFiles.value.filter(f => AUDIO_EXT.test(f.name)).length;
}

async function submitUpload() {
  if (!uploadFiles.value.length || !uploadTitle.value || !uploadAuthor.value) return;
  uploadProgress.value = 0;
  uploadError.value = '';
  uploadDone.value = false;

  try {
    await api.books.check(uploadFiles.value);
  } catch (e: any) {
    uploadError.value = e.message;
    toast('error', e.message);
    return;
  }

  const fd = new FormData();
  fd.append('title', uploadTitle.value);
  fd.append('author', uploadAuthor.value);
  if (uploadNarrator.value) fd.append('narrator', uploadNarrator.value);
  for (const f of uploadFiles.value) fd.append('files', f);

  uploadMutation.mutate(fd);
}

function openModal() {
  showUploadModal.value = true;
  dialogEl.value?.showModal();
}

function closeModal() {
  if (uploadMutation.isPending.value) return;
  dialogEl.value?.close();
  showUploadModal.value = false;
  uploadTitle.value = '';
  uploadAuthor.value = '';
  uploadNarrator.value = '';
  uploadFiles.value = [];
  coverPreview.value = null;
  uploadProgress.value = 0;
  uploadError.value = '';
  uploadDone.value = false;
}

function onDialogClick(e: MouseEvent) {
  if (e.target === dialogEl.value) closeModal();
}

function formatDuration(sec: number | null | undefined) {
  if (!sec) return null;
  const h = Math.floor(sec / 3600);
  const m = Math.floor((sec % 3600) / 60);
  return h > 0 ? `${h} ч ${m} мин` : `${m} мин`;
}

function pluralChapters(n: number) {
  return n === 1 ? 'глава' : n < 5 ? 'главы' : 'глав';
}
</script>

<style scoped>
.page { max-width: 960px; margin: 0 auto; padding: 1.5rem; }
.page-header { display: flex; align-items: center; justify-content: space-between; margin-bottom: 1.5rem; }
h2 { font-size: 1.4rem; font-weight: 700; }

.btn-primary {
  background: #fff; color: #000; border: none;
  padding: 0.5rem 1rem; border-radius: 8px;
  font-weight: 600; cursor: pointer; font-size: 0.9rem;
}
.btn-primary:hover { background: #e5e5e5; }
.btn-primary:disabled { opacity: 0.4; cursor: not-allowed; }

.hint { color: #555; text-align: center; padding: 3rem 0; }
.error { color: #f87171; font-size: 0.85rem; }

.books-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(200px, 1fr)); gap: 1rem; }
.book-card { background: #1a1a1a; border: 1px solid #2a2a2a; border-radius: 10px; overflow: hidden; display: flex; flex-direction: column; }
.book-cover { width: 100%; aspect-ratio: 1; object-fit: cover; }
.book-cover.placeholder { background: linear-gradient(135deg, #2a2a2a, #1a1a1a); aspect-ratio: 1; }
.book-info { padding: 0.75rem; flex: 1; }
.book-info h3 { font-size: 0.95rem; font-weight: 600; margin-bottom: 0.25rem; line-height: 1.3; }
.author { color: #888; font-size: 0.85rem; }
.meta { color: #555; font-size: 0.8rem; margin-top: 0.1rem; }
.stats { display: flex; flex-wrap: wrap; gap: 0.3rem 0.5rem; margin-top: 0.35rem; }
.stats span { font-size: 0.75rem; color: #555; background: #222; padding: 1px 6px; border-radius: 4px; }
.book-actions { padding: 0.5rem 0.75rem 0.75rem; display: flex; gap: 0.5rem; }
.btn-add { flex: 1; background: #2a2a2a; color: #fff; border: none; padding: 0.4rem 0.5rem; border-radius: 6px; cursor: pointer; font-size: 0.8rem; }
.btn-add:hover:not(:disabled) { background: #333; }
.btn-delete { background: #3a1a1a; color: #f87171; border: none; padding: 0.4rem 0.5rem; border-radius: 6px; cursor: pointer; font-size: 0.8rem; }
.btn-delete:hover:not(:disabled) { background: #4a1a1a; }
button:disabled { opacity: 0.5; cursor: not-allowed; }

.modal {
  background: #1a1a1a;
  border: 1px solid #2a2a2a;
  border-radius: 12px;
  padding: 1.5rem;
  width: min(420px, calc(100vw - 2rem));
  color: #fff;
  position: fixed;
  top: 50%;
  left: 50%;
  transform: translate(-50%, -50%);
  margin: 0;
}
.modal::backdrop { background: rgba(0,0,0,0.7); }
.modal h3 { margin-bottom: 1rem; font-size: 1.1rem; }

.folder-drop {
  display: flex; flex-direction: column; align-items: center; justify-content: center;
  gap: 0.5rem; border: 2px dashed #333; border-radius: 8px;
  padding: 1.5rem; cursor: pointer; margin-bottom: 1rem;
  color: #888; font-size: 0.9rem; transition: border-color 0.15s;
}
.folder-drop:hover { border-color: #555; color: #aaa; }
.folder-drop input { display: none; }

.folder-summary { display: flex; align-items: center; gap: 0.75rem; background: #0f0f0f; border-radius: 8px; padding: 0.5rem 0.75rem; margin-bottom: 0.75rem; }
.cover-preview { width: 48px; height: 48px; border-radius: 6px; object-fit: cover; flex-shrink: 0; }
.cover-preview.placeholder { background: linear-gradient(135deg, #2a2a2a, #1a1a1a); }
.folder-stats p { font-size: 0.9rem; color: #ccc; margin: 0; }
.hint-small { color: #555 !important; font-size: 0.8rem !important; }

.modal label { display: flex; flex-direction: column; gap: 0.3rem; margin-bottom: 0.75rem; font-size: 0.85rem; color: #888; }
.modal input[type="text"] { background: #0f0f0f; border: 1px solid #333; border-radius: 6px; padding: 0.5rem 0.75rem; color: #fff; font-size: 0.95rem; outline: none; }
.modal input[type="text"]:focus { border-color: #555; }

.progress-wrap { height: 4px; background: #2a2a2a; border-radius: 2px; margin-bottom: 0.4rem; overflow: hidden; }
.progress-bar { height: 100%; background: #fff; transition: width 0.15s ease; }
.progress-wrap.done .progress-bar { background: #4ade80; }

.progress-bar.indeterminate {
  width: 40% !important;
  background: #888;
  animation: indeterminate 1.2s ease-in-out infinite;
}
@keyframes indeterminate {
  0%   { margin-left: -40%; }
  100% { margin-left: 100%; }
}

.progress-text { font-size: 0.8rem; color: #888; margin-bottom: 0.75rem; }
.progress-wrap.done + .progress-text { color: #4ade80; }

.upload-error {
  display: flex; align-items: center; gap: 0.4rem;
  background: #3a1a1a; color: #f87171;
  border-radius: 6px; padding: 0.5rem 0.75rem;
  font-size: 0.85rem; margin-bottom: 0.75rem;
}

.modal-actions { display: flex; gap: 0.75rem; margin-top: 1rem; justify-content: flex-end; }
.modal-actions button { background: #2a2a2a; color: #fff; border: none; padding: 0.5rem 1rem; border-radius: 6px; cursor: pointer; font-size: 0.9rem; }
.modal-actions .btn-primary { background: #fff; color: #000; font-weight: 600; }
</style>
