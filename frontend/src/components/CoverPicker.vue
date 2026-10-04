<!-- Book cover with an admin-only "replace" button that opens a file picker. -->
<template>
  <div class="cover-wrap">
    <img :src="coverUrl(coverPath)" :alt="alt" class="book-cover" />
    <template v-if="editable">
      <button class="cover-edit-btn" @click="inputEl?.click()" title="Заменить обложку"><ImagePlus :size="14" /></button>
      <input ref="inputEl" type="file" accept="image/*" hidden @change="onChange" />
    </template>
  </div>
</template>

<script setup lang="ts">
import { ref } from 'vue';
import { ImagePlus } from 'lucide-vue-next';
import { coverUrl } from '../utils/format';

defineProps<{
  coverPath?: string | null;
  alt: string;
  editable: boolean;
}>();

const emit = defineEmits<{ pick: [file: File] }>();
const inputEl = ref<HTMLInputElement | null>(null);

function onChange(e: Event) {
  const input = e.target as HTMLInputElement;
  const file = input.files?.[0];
  if (file) emit('pick', file);
  input.value = ''; // picking the same file again must fire `change`
}
</script>
