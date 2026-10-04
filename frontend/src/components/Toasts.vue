<template>
  <div class="toasts" :class="{ 'above-player': !!player.book }">
    <div v-for="t in toasts" :key="t.id" :class="['toast', `toast-${t.type}`]">{{ t.text }}</div>
  </div>
</template>

<script setup lang="ts">
import { toasts } from '../stores/toasts';
import { player } from '../stores/player';
</script>

<style scoped>
.toasts {
  position: fixed;
  bottom: 1.5rem;
  right: 1.5rem;
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
  z-index: 300;
  pointer-events: none;
}
.toasts.above-player { bottom: calc(80px + 1rem); }
.toast {
  padding: 0.65rem 1rem;
  border-radius: 8px;
  font-size: 0.9rem;
  font-weight: 500;
  animation: slide-in 0.2s ease;
  max-width: 320px;
}
.toast-success { background: #166534; color: #bbf7d0; }
.toast-error   { background: #7f1d1d; color: #fecaca; }
.toast-info    { background: #1e3a5f; color: #bfdbfe; }

@keyframes slide-in {
  from { opacity: 0; transform: translateY(8px); }
  to   { opacity: 1; transform: translateY(0); }
}
</style>
