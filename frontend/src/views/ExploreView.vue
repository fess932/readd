<template>
  <main class="page">
    <div class="page-header">
      <h2>Общая библиотека</h2>
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
      <AudioBookCard
        :book="item"
        :is-admin="isAdmin"
        :show-author="!underAuthor"
        :in-library="libraryIds.has(item.id)"
        :adding="addMutation.isPending.value && addMutation.variables.value === item.id"
        :deleting="deleteMutation.isPending.value && deleteMutation.variables.value === item.id"
        :save="(edit) => editMutation.mutateAsync({ id: item.id, edit })"
        @add="addMutation.mutate(item.id)"
        @delete="confirmDeleteId = item.id"
        @cover="coverMutation.mutate({ id: item.id, file: $event })"
      />
    </AuthorGroups>
  </main>

  <UploadDialog
    ref="uploadDialog"
    title="Загрузить книгу"
    :pending="uploadMutation.isPending.value"
    :done="uploadDone"
    :progress="uploadProgress"
    :error="uploadError"
    :can-submit="audioCount > 0 && !!title && !!author"
    :status="uploadStatus"
    cancellable
    @submit="submitUpload"
    @cancel="uploadAbort?.abort()"
    @closed="resetUpload"
  >
    <label class="file-drop">
      <FolderOpen :size="28" />
      <span>Выберите папку с книгой</span>
      <input type="file" webkitdirectory @change="onFolderPicked" />
    </label>

    <div v-if="files.length" class="folder-summary">
      <img v-if="coverPreview" :src="coverPreview" alt="обложка" class="cover-preview" />
      <div v-else class="cover-preview placeholder"></div>
      <div class="folder-stats">
        <p>{{ files.length }} {{ plural(files.length, 'файл', 'файла', 'файлов') }}</p>
        <p class="hint-small">{{ audioCount }} аудио</p>
      </div>
    </div>

    <label>Название * <input type="text" v-model="title" required /></label>
    <label>Автор * <input type="text" v-model="author" required /></label>
    <label>Диктор <input type="text" v-model="narrator" /></label>
  </UploadDialog>

  <Confirm
    v-if="bookToDelete"
    title="Удалить книгу?"
    :message="`«${bookToDelete.title}» будет удалена из общей библиотеки. Это действие нельзя отменить.`"
    confirmLabel="Удалить"
    danger
    :pending="deleteMutation.isPending.value"
    :onconfirm="() => deleteMutation.mutate(bookToDelete!.id)"
    :oncancel="() => confirmDeleteId = null"
  />
</template>

<script setup lang="ts">
import { ref, computed } from 'vue';
import { Upload, FolderOpen, Users } from 'lucide-vue-next';
import { useQuery, useMutation, useQueryClient } from '@tanstack/vue-query';
import { api, type Book } from '../api';
import { auth } from '../stores/auth';
import { player, stopPlayer } from '../stores/player';
import { flushOutbox } from '../stores/progressSync';
import { toast } from '../stores/toasts';
import { uploadBook, type BookUpload } from '../upload';
import { plural } from '../utils/format';
import AudioBookCard, { type BookEdit } from '../components/AudioBookCard.vue';
import AuthorGroups from '../components/AuthorGroups.vue';
import Confirm from '../components/Confirm.vue';
import UploadDialog from '../components/UploadDialog.vue';

const queryClient = useQueryClient();
const isAdmin = computed(() => !!auth.user?.isAdmin);
const grouped = ref(false);
const confirmDeleteId = ref<number | null>(null);

const { data: books, isLoading, error } = useQuery({
  queryKey: ['books'],
  queryFn: api.books.list,
});

// Which of these books the user already has
const { data: library } = useQuery({
  queryKey: ['library'],
  queryFn: () => flushOutbox().then(api.library.list),
});
const libraryIds = computed(() => new Set(library.value?.map(b => b.id)));

const bookToDelete = computed(() => books.value?.find(b => b.id === confirmDeleteId.value));

/** Applies `change` to one book in the cached list. */
function patchCached(id: number, change: Partial<Book>) {
  queryClient.setQueryData(['books'], (old: Book[] | undefined) =>
    old?.map(b => (b.id === id ? { ...b, ...change } : b)) ?? []
  );
}

const onError = (err: Error) => toast('error', err.message);

// ── Per-book actions ─────────────────────────────────────────────────────────

const addMutation = useMutation({
  mutationFn: (bookId: number) => api.library.add(bookId),
  onSuccess: () => {
    queryClient.invalidateQueries({ queryKey: ['library'] });
    toast('success', 'Добавлено в вашу библиотеку');
  },
  onError,
});

const editMutation = useMutation({
  mutationFn: ({ id, edit }: { id: number; edit: BookEdit }) => api.books.patch(id, edit),
  onSuccess: (_, { id, edit }) => {
    patchCached(id, { ...edit, narrator: edit.narrator || null });
    queryClient.invalidateQueries({ queryKey: ['library'] });
    toast('success', 'Сохранено');
  },
  onError,
});

const coverMutation = useMutation({
  mutationFn: ({ id, file }: { id: number; file: File }) => api.books.uploadCover(id, file),
  onSuccess: ({ coverPath }, { id }) => {
    patchCached(id, { coverPath });
    queryClient.invalidateQueries({ queryKey: ['library'] });
    toast('success', 'Обложка обновлена');
  },
  onError,
});

const deleteMutation = useMutation({
  mutationFn: (bookId: number) => api.books.delete(bookId),
  onSuccess: (_, bookId) => {
    queryClient.setQueryData(['books'], (old: Book[] | undefined) => old?.filter(b => b.id !== bookId) ?? []);
    // the book is gone from every library, and its voicing job with it
    queryClient.invalidateQueries({ queryKey: ['library'] });
    queryClient.invalidateQueries({ queryKey: ['tts-jobs'] });
    if (player.book?.id === bookId) stopPlayer();
    toast('success', 'Книга удалена');
    confirmDeleteId.value = null;
  },
  onError,
});

// ── Upload ───────────────────────────────────────────────────────────────────

const AUDIO_EXT = /\.(mp3|m4a|m4b|ogg|flac|wav|aac|opus)$/i;
const IMAGE_EXT = /\.(jpg|jpeg|png|webp|avif)$/i;
const COVER_NAME = /^(cover|folder|front|artwork|thumb)/i;

const uploadDialog = ref<InstanceType<typeof UploadDialog> | null>(null);
const title = ref('');
const author = ref('');
const narrator = ref('');
const files = ref<File[]>([]);
const coverPreview = ref<string | null>(null);
const uploadProgress = ref(0);
const uploadError = ref('');
const uploadDone = ref(false);

const audioCount = computed(() => files.value.filter(f => AUDIO_EXT.test(f.name)).length);

/** "Автор - Название (2019)" → both fields; anything else is taken as the title. */
function parseFolderName(name: string): { title: string; author: string } {
  const clean = name.replace(/\s*[\(\[][^\)\]]*[\)\]]/g, '').trim();
  const dash = clean.match(/^(.+?)\s+[-–—]\s+(.+)$/);
  return dash ? { author: dash[1].trim(), title: dash[2].trim() } : { title: clean, author: '' };
}

function setCoverPreview(file: File | undefined) {
  if (coverPreview.value) URL.revokeObjectURL(coverPreview.value);
  coverPreview.value = file ? URL.createObjectURL(file) : null;
}

function onFolderPicked(e: Event) {
  const picked = Array.from((e.currentTarget as HTMLInputElement).files ?? []);
  if (!picked.length) return;
  files.value = picked;
  uploadDone.value = false;
  uploadError.value = '';

  const parsed = parseFolderName(picked[0].webkitRelativePath.split('/')[0]);
  title.value = parsed.title;
  author.value = parsed.author;

  const images = picked.filter(f => IMAGE_EXT.test(f.name));
  setCoverPreview(images.find(f => COVER_NAME.test(f.name)) ?? images[0]);
}

const uploadStatus = ref('');
let uploadAbort: AbortController | null = null;

const uploadMutation = useMutation({
  mutationFn: (upload: BookUpload) => {
    uploadAbort = new AbortController();
    return uploadBook(upload, (p) => {
      uploadProgress.value = p.percent;
      uploadStatus.value = `${p.filesDone} из ${p.filesTotal} файлов`;
    }, uploadAbort.signal);
  },
  onSuccess: (book) => {
    queryClient.setQueryData(['books'], (old: Book[] | undefined) => [book, ...(old ?? [])]);
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

async function submitUpload() {
  uploadProgress.value = 0;
  uploadStatus.value = '';
  uploadError.value = '';
  uploadDone.value = false;

  // Only what makes up the book: audio and a cover, not .DS_Store, playlists or notes
  const bookFiles = files.value.filter(f => AUDIO_EXT.test(f.name) || IMAGE_EXT.test(f.name));

  // Ask before sending gigabytes: the server may already have this book
  try {
    await api.books.check(bookFiles);
  } catch (e: any) {
    uploadError.value = e.message;
    toast('error', e.message);
    return;
  }

  uploadMutation.mutate({
    title: title.value,
    author: author.value,
    narrator: narrator.value || undefined,
    files: bookFiles,
  });
}

function resetUpload() {
  title.value = '';
  author.value = '';
  narrator.value = '';
  files.value = [];
  setCoverPreview(undefined);
  uploadProgress.value = 0;
  uploadStatus.value = '';
  uploadError.value = '';
  uploadDone.value = false;
}
</script>

<style scoped>
.folder-summary { display: flex; align-items: center; gap: 0.75rem; background: #0f0f0f; border-radius: 8px; padding: 0.5rem 0.75rem; margin-bottom: 0.75rem; }
.cover-preview { width: 48px; height: 48px; border-radius: 6px; object-fit: cover; flex-shrink: 0; }
.cover-preview.placeholder { background: linear-gradient(135deg, #2a2a2a, #1a1a1a); }
.folder-stats p { font-size: 0.9rem; color: #ccc; }
.folder-stats .hint-small { color: #555; font-size: 0.8rem; }
</style>
