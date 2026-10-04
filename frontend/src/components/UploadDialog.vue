<!-- Modal with an upload form: the fields come from the slot, progress and buttons live here. -->
<template>
  <dialog ref="dialogEl" class="modal" @cancel="onCancel" @close="onClosed" @click="onBackdropClick">
    <!-- v-if: a fresh form (and fresh file inputs) every time the dialog opens -->
    <template v-if="isOpen">
      <h3>{{ title }}</h3>
      <form @submit.prevent="emit('submit')">
        <slot />

        <template v-if="pending || done">
          <div class="progress-wrap" :class="{ done }">
            <div
              class="progress-bar"
              :class="{ indeterminate: pending && progress === 100 && !done }"
              :style="{ width: (done ? 100 : progress) + '%' }"
            ></div>
          </div>
          <p class="progress-text">
            <template v-if="done"><CheckCircle2 :size="13" style="vertical-align: -2px" /> Загружено</template>
            <template v-else-if="progress === 100">Сохранение на сервере…</template>
            <template v-else>Отправка {{ progress }}%…</template>
          </p>
        </template>

        <div v-if="error" class="upload-error"><X :size="14" /> {{ error }}</div>

        <div class="modal-actions">
          <button type="button" @click="close" :disabled="pending">Отмена</button>
          <button type="submit" class="btn-primary" :disabled="pending || done || !canSubmit">
            {{ pending ? `${progress}%` : done ? 'Готово' : 'Загрузить' }}
          </button>
        </div>
      </form>
    </template>
  </dialog>
</template>

<script setup lang="ts">
import { ref } from 'vue';
import { CheckCircle2, X } from 'lucide-vue-next';

const props = defineProps<{
  title: string;
  pending: boolean;
  done: boolean;
  /** Percent of the request body sent, 0–100. */
  progress: number;
  error: string;
  canSubmit: boolean;
}>();

const emit = defineEmits<{
  submit: [];
  /** The dialog is gone; the parent resets its form state. */
  closed: [];
}>();

const dialogEl = ref<HTMLDialogElement | null>(null);
const isOpen = ref(false);

function show() {
  isOpen.value = true;
  dialogEl.value?.showModal();
}

/** An upload in flight cannot be abandoned by closing the dialog. */
function close() {
  if (props.pending) return;
  dialogEl.value?.close();
}

function onCancel(e: Event) {
  if (props.pending) e.preventDefault(); // Esc
}

function onClosed() {
  isOpen.value = false;
  emit('closed');
}

function onBackdropClick(e: MouseEvent) {
  if (e.target === dialogEl.value) close();
}

defineExpose({ show, close });
</script>

<style scoped>
.progress-wrap { height: 4px; background: #2a2a2a; border-radius: 2px; margin-bottom: 0.4rem; overflow: hidden; }
.progress-bar { height: 100%; background: #fff; transition: width 0.15s ease; }
.progress-wrap.done .progress-bar { background: #4ade80; }
.progress-bar.indeterminate { width: 40% !important; background: #888; animation: indeterminate 1.2s ease-in-out infinite; }
@keyframes indeterminate {
  0% { margin-left: -40%; }
  100% { margin-left: 100%; }
}

.progress-text { font-size: 0.8rem; color: #888; margin-bottom: 0.75rem; }
.progress-wrap.done + .progress-text { color: #4ade80; }

.upload-error { display: flex; align-items: center; gap: 0.4rem; background: #3a1a1a; color: #f87171; border-radius: 6px; padding: 0.5rem 0.75rem; font-size: 0.85rem; margin-bottom: 0.75rem; }
</style>
