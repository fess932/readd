<!-- An audiobook in the shared catalogue; admins can edit it in place. -->
<template>
  <div class="book-card">
    <CoverPicker :cover-path="book.coverPath" :alt="book.title" :editable="isAdmin" @pick="emit('cover', $event)" />

    <div class="book-info">
      <template v-if="editing">
        <input class="edit-input" v-model="title" placeholder="Название" />
        <input class="edit-input" v-model="author" placeholder="Автор" />
        <input class="edit-input" v-model="narrator" placeholder="Диктор" />
      </template>
      <template v-else>
        <h3>{{ book.title }}</h3>
        <p v-if="showAuthor" class="author">{{ book.author }}</p>
        <p v-if="book.narrator" class="meta">Читает: {{ book.narrator }}</p>
        <div class="stats">
          <span v-if="book.chaptersCount > 0">{{ book.chaptersCount }} {{ pluralChapters(book.chaptersCount) }}</span>
          <span v-if="formatDuration(book.totalSec)">{{ formatDuration(book.totalSec) }}</span>
        </div>
      </template>
    </div>

    <div class="book-actions">
      <template v-if="editing">
        <button class="btn-icon save" @click="submit" :disabled="saving || !title.trim() || !author.trim()" title="Сохранить"><Check :size="14" /></button>
        <button class="btn-icon" @click="editing = false" title="Отмена"><X :size="14" /></button>
      </template>
      <template v-else>
        <router-link v-if="inLibrary" :to="`/book/${book.id}`" class="btn-add in-library" title="Открыть книгу">
          <Check :size="14" /> В моей
        </router-link>
        <button v-else class="btn-add" @click="emit('add')" :disabled="adding"><Plus :size="14" /> В моё</button>
        <template v-if="isAdmin">
          <button class="btn-icon" @click="startEdit" title="Редактировать"><Pencil :size="14" /></button>
          <button class="btn-icon danger" @click="emit('delete')" :disabled="deleting" title="Удалить"><Trash2 :size="14" /></button>
        </template>
      </template>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref } from 'vue';
import { Plus, Trash2, Pencil, Check, X } from 'lucide-vue-next';
import type { Book } from '../api';
import { formatDuration, pluralChapters } from '../utils/format';
import CoverPicker from './CoverPicker.vue';

export interface BookEdit {
  title: string;
  author: string;
  narrator: string;
}

const props = defineProps<{
  book: Book;
  isAdmin: boolean;
  showAuthor: boolean;
  /** Already in the current user's library. */
  inLibrary: boolean;
  adding: boolean;
  deleting: boolean;
  /** Persists the edit; the form closes once it resolves. */
  save: (edit: BookEdit) => Promise<unknown>;
}>();

const emit = defineEmits<{
  add: [];
  delete: [];
  cover: [file: File];
}>();

const editing = ref(false);
const saving = ref(false);
const title = ref('');
const author = ref('');
const narrator = ref('');

function startEdit() {
  title.value = props.book.title;
  author.value = props.book.author;
  narrator.value = props.book.narrator ?? '';
  editing.value = true;
}

async function submit() {
  saving.value = true;
  try {
    await props.save({ title: title.value.trim(), author: author.value.trim(), narrator: narrator.value.trim() });
    editing.value = false;
  } catch {
    // the caller reports the error; keep the form open
  } finally {
    saving.value = false;
  }
}
</script>

<style scoped>
.btn-add { flex: 1; display: flex; align-items: center; justify-content: center; gap: 0.3rem; background: #2a2a2a; color: #fff; border: none; padding: 0.4rem 0.5rem; border-radius: 6px; cursor: pointer; font-size: 0.8rem; }
.btn-add:hover:not(:disabled) { background: #333; }
.btn-add.in-library { background: none; border: 1px solid #2a2a2a; color: #4ade80; }
.btn-add.in-library:hover { border-color: #4ade80; }
</style>
