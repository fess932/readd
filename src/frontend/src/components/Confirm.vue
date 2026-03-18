<template>
  <dialog ref="dialogEl" class="confirm-dialog" @close="oncancel">
    <h3 v-if="title">{{ title }}</h3>
    <p>{{ message }}</p>
    <div class="actions">
      <button class="btn-cancel" @click="oncancel">{{ cancelLabel }}</button>
      <button class="btn-confirm" :class="{ danger }" @click="onconfirm">{{ confirmLabel }}</button>
    </div>
  </dialog>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue';

const props = withDefaults(defineProps<{
  title?: string;
  message: string;
  confirmLabel?: string;
  cancelLabel?: string;
  danger?: boolean;
  onconfirm: () => void;
  oncancel: () => void;
}>(), {
  confirmLabel: 'Подтвердить',
  cancelLabel: 'Отмена',
  danger: false,
});

const dialogEl = ref<HTMLDialogElement | null>(null);

onMounted(() => {
  dialogEl.value?.showModal();
});
</script>

<style scoped>
.confirm-dialog {
  background: #1a1a1a;
  border: 1px solid #2a2a2a;
  border-radius: 12px;
  padding: 1.5rem;
  width: min(380px, calc(100vw - 2rem));
  color: #fff;
  position: fixed;
  top: 50%;
  left: 50%;
  transform: translate(-50%, -50%);
  margin: 0;
}
.confirm-dialog::backdrop { background: rgba(0,0,0,0.7); }

h3 { font-size: 1rem; font-weight: 600; margin-bottom: 0.5rem; }
p  { color: #aaa; font-size: 0.9rem; line-height: 1.5; }

.actions {
  display: flex;
  justify-content: flex-end;
  gap: 0.5rem;
  margin-top: 1.25rem;
}

.btn-cancel {
  background: #2a2a2a;
  color: #aaa;
  border: none;
  padding: 0.5rem 1rem;
  border-radius: 7px;
  cursor: pointer;
  font-size: 0.9rem;
}
.btn-cancel:hover { background: #333; color: #fff; }

.btn-confirm {
  background: #fff;
  color: #000;
  border: none;
  padding: 0.5rem 1rem;
  border-radius: 7px;
  cursor: pointer;
  font-size: 0.9rem;
  font-weight: 600;
}
.btn-confirm.danger { background: #dc2626; color: #fff; }
.btn-confirm.danger:hover { background: #b91c1c; }
.btn-confirm:not(.danger):hover { background: #e5e5e5; }
</style>
