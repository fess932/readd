<!-- An epub in the catalogue, with its voicing status; admins can edit it in place. -->
<template>
  <div class="book-card">
    <CoverPicker :cover-path="book.coverPath" :alt="book.title" :editable="isAdmin" @pick="emit('cover', $event)" />

    <div class="book-info">
      <template v-if="editing">
        <input class="edit-input" v-model="title" placeholder="Название" />
        <input class="edit-input" v-model="author" placeholder="Автор" />
      </template>
      <template v-else>
        <h3>{{ book.title }}</h3>
        <p v-if="showAuthor" class="author">{{ book.author }}</p>
        <div v-if="book.fileSize" class="stats"><span>{{ formatSize(book.fileSize) }}</span></div>
      </template>
    </div>

    <slot name="tts" />

    <div v-if="isAdmin" class="book-actions">
      <template v-if="editing">
        <button class="btn-icon save" @click="submit" :disabled="saving || !title.trim() || !author.trim()" title="Сохранить"><Check :size="14" /></button>
        <button class="btn-icon" @click="editing = false" title="Отмена"><X :size="14" /></button>
      </template>
      <template v-else>
        <button class="btn-icon" @click="startEdit" title="Редактировать"><Pencil :size="14" /></button>
        <button class="btn-icon danger" @click="emit('delete')" :disabled="deleting" title="Удалить"><Trash2 :size="14" /></button>
      </template>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref } from 'vue';
import { Trash2, Pencil, Check, X } from 'lucide-vue-next';
import type { TextBook } from '../api';
import { formatSize } from '../utils/format';
import CoverPicker from './CoverPicker.vue';

export interface TextBookEdit {
  title: string;
  author: string;
}

const props = defineProps<{
  book: TextBook;
  isAdmin: boolean;
  showAuthor: boolean;
  deleting: boolean;
  /** Persists the edit; the form closes once it resolves. */
  save: (edit: TextBookEdit) => Promise<unknown>;
}>();

const emit = defineEmits<{
  delete: [];
  cover: [file: File];
}>();

const editing = ref(false);
const saving = ref(false);
const title = ref('');
const author = ref('');

function startEdit() {
  title.value = props.book.title;
  author.value = props.book.author;
  editing.value = true;
}

async function submit() {
  saving.value = true;
  try {
    await props.save({ title: title.value.trim(), author: author.value.trim() });
    editing.value = false;
  } catch {
    // the caller reports the error; keep the form open
  } finally {
    saving.value = false;
  }
}
</script>
