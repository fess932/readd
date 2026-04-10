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
          <div v-for="book in group.books" :key="book.id" class="book-card">
            <div class="cover-wrap" @mouseenter="hoveredCoverId = book.id" @mouseleave="hoveredCoverId = null">
              <img :src="book.coverPath ? `/uploads/${book.coverPath}` : '/placeholder.jpg'" :alt="book.title" class="book-cover" />
              <button v-if="auth.user?.isAdmin" class="cover-edit-btn" :class="{ hidden: hoveredCoverId !== book.id }" @click="openCoverUpload(book.id)" title="Заменить обложку"><ImagePlus :size="14" /></button>
            </div>
            <div class="book-info">
              <template v-if="editingId === book.id">
                <input class="edit-input" v-model="editTitle" placeholder="Название" />
                <input class="edit-input" v-model="editAuthor" placeholder="Автор" />
              </template>
              <template v-else>
                <h3>{{ book.title }}</h3>
                <div v-if="book.fileSize" class="stats">
                  <span>{{ formatSize(book.fileSize) }}</span>
                </div>
              </template>
            </div>
            <div class="book-actions">
              <template v-if="editingId === book.id">
                <button class="btn-save" @click="submitEdit(book.id)" :disabled="editMutation.isPending.value"><Check :size="14" /></button>
                <button class="btn-cancel-edit" @click="cancelEdit"><XIcon :size="14" /></button>
              </template>
              <template v-else>
                <button v-if="auth.user?.isAdmin" class="btn-edit" @click="startEdit(book)"><Pencil :size="14" /></button>
                <button v-if="auth.user?.isAdmin" class="btn-delete" @click="confirmDeleteId = book.id" :disabled="deleteMutation.isPending.value && deleteMutation.variables.value === book.id">
                  <Trash2 :size="14" />
                </button>
              </template>
            </div>
          </div>
        </div>
      </div>
    </template>
    <div v-else class="books-grid">
      <div v-for="book in books" :key="book.id" class="book-card">
        <div class="cover-wrap" @mouseenter="hoveredCoverId = book.id" @mouseleave="hoveredCoverId = null">
          <img :src="book.coverPath ? `/uploads/${book.coverPath}` : '/placeholder.jpg'" :alt="book.title" class="book-cover" />
          <button v-if="auth.user?.isAdmin" class="cover-edit-btn" :class="{ hidden: hoveredCoverId !== book.id }" @click="openCoverUpload(book.id)" title="Заменить обложку"><ImagePlus :size="14" /></button>
        </div>
        <div class="book-info">
          <template v-if="editingId === book.id">
            <input class="edit-input" v-model="editTitle" placeholder="Название" />
            <input class="edit-input" v-model="editAuthor" placeholder="Автор" />
          </template>
          <template v-else>
            <h3>{{ book.title }}</h3>
            <p class="author">{{ book.author }}</p>
            <div v-if="book.fileSize" class="stats">
              <span>{{ formatSize(book.fileSize) }}</span>
            </div>
          </template>
        </div>
        <div class="book-actions">
          <template v-if="editingId === book.id">
            <button class="btn-save" @click="submitEdit(book.id)" :disabled="editMutation.isPending.value"><Check :size="14" /></button>
            <button class="btn-cancel-edit" @click="cancelEdit"><XIcon :size="14" /></button>
          </template>
          <template v-else>
            <button v-if="auth.user?.isAdmin" class="btn-edit" @click="startEdit(book)"><Pencil :size="14" /></button>
            <button v-if="auth.user?.isAdmin" class="btn-delete" @click="confirmDeleteId = book.id" :disabled="deleteMutation.isPending.value && deleteMutation.variables.value === book.id">
              <Trash2 :size="14" />
            </button>
          </template>
        </div>
      </div>
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
            <div
              class="progress-bar"
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

        <div v-if="uploadError" class="upload-error">
          <X :size="14" /> {{ uploadError }}
        </div>

        <div class="modal-actions">
          <button type="button" @click="closeModal" :disabled="uploadMutation.isPending.value">Отмена</button>
          <button
            type="submit"
            class="btn-primary"
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
import { ref, computed } from 'vue';
import { Upload, Users, Pencil, Check, X as XIcon, ImagePlus, Trash2, BookOpen, CheckCircle2, X } from 'lucide-vue-next';
import { useQuery, useMutation, useQueryClient } from '@tanstack/vue-query';
import { api, type TextBook } from '../api';
import { auth } from '../stores/auth';
import { toast } from '../stores/toasts';
import Confirm from '../components/Confirm.vue';

const queryClient = useQueryClient();
const confirmDeleteId = ref<number | null>(null);
const groupByAuthor = ref(false);
const hoveredCoverId = ref<number | null>(null);

// Cover upload
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

// Edit
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

// Delete
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

// Upload
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
  if (dash) {
    uploadAuthor.value = dash[1].trim();
    uploadTitle.value = dash[2].trim();
  } else {
    uploadTitle.value = stem;
  }
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
  uploadFile.value = null;
  uploadProgress.value = 0;
  uploadError.value = '';
  uploadDone.value = false;
}

function onDialogClick(e: MouseEvent) {
  if (e.target === dialogEl.value) closeModal();
}

function formatSize(bytes: number): string {
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(0)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}
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
.book-card { background: #1a1a1a; border: 1px solid #2a2a2a; border-radius: 10px; overflow: hidden; display: flex; flex-direction: column; }
.cover-wrap { position: relative; display: block; }
.book-cover { width: 100%; aspect-ratio: 1; object-fit: cover; display: block; }
.cover-edit-btn { position: absolute; bottom: 6px; right: 6px; background: rgba(0,0,0,0.75); color: #ddd; border: none; border-radius: 6px; padding: 0.3rem; cursor: pointer; display: flex; align-items: center; z-index: 2; transition: opacity 0.15s, background 0.15s; }
.cover-edit-btn.hidden { opacity: 0; pointer-events: none; }
.cover-edit-btn:hover { color: #fff; background: rgba(0,0,0,0.95); }
.book-info { padding: 0.75rem; flex: 1; }
.book-info h3 { font-size: 0.95rem; font-weight: 600; margin-bottom: 0.25rem; line-height: 1.3; }
.author { color: #888; font-size: 0.85rem; }
.stats { display: flex; flex-wrap: wrap; gap: 0.3rem 0.5rem; margin-top: 0.35rem; }
.stats span { font-size: 0.75rem; color: #555; background: #222; padding: 1px 6px; border-radius: 4px; }
.book-actions { padding: 0.5rem 0.75rem 0.75rem; display: flex; gap: 0.5rem; justify-content: flex-end; }
.btn-edit { display: flex; align-items: center; justify-content: center; background: none; color: #555; border: 1px solid #2a2a2a; padding: 0.4rem 0.6rem; border-radius: 6px; cursor: pointer; }
.btn-edit:hover { color: #fff; border-color: #444; }
.btn-save { display: flex; align-items: center; justify-content: center; background: #1a3a1a; color: #4ade80; border: none; padding: 0.4rem 0.6rem; border-radius: 6px; cursor: pointer; flex: 1; }
.btn-save:hover:not(:disabled) { background: #1f4a1f; }
.btn-cancel-edit { display: flex; align-items: center; justify-content: center; background: none; color: #555; border: 1px solid #2a2a2a; padding: 0.4rem 0.6rem; border-radius: 6px; cursor: pointer; }
.btn-cancel-edit:hover { color: #fff; border-color: #444; }
.btn-delete { display: flex; align-items: center; justify-content: center; background: #3a1a1a; color: #f87171; border: none; padding: 0.4rem 0.6rem; border-radius: 6px; cursor: pointer; }
.btn-delete:hover:not(:disabled) { background: #4a1a1a; }
button:disabled { opacity: 0.5; cursor: not-allowed; }
.edit-input { background: #0f0f0f; border: 1px solid #333; border-radius: 5px; padding: 0.3rem 0.5rem; color: #fff; font-size: 0.82rem; width: 100%; margin-bottom: 0.3rem; outline: none; }
.edit-input:focus { border-color: #555; }
.edit-input:last-child { margin-bottom: 0; }

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
