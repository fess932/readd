<!-- State of a text book's voicing job, with the controls admins get in each state. -->
<template>
  <div v-if="job || isAdmin" class="tts-section">
    <button v-if="!job" class="btn-tts-start" @click="emit('start')"><Mic :size="13" /> Озвучить</button>

    <template v-else-if="job.status === 'running' || job.status === 'paused'">
      <div class="tts-progress-row">
        <span class="tts-label" :class="job.status">
          <Mic v-if="job.status === 'running'" :size="11" /><Pause v-else :size="11" />
          {{ job.doneChunks }} / {{ job.totalChunks }}
        </span>
        <template v-if="isAdmin">
          <button v-if="job.status === 'running'" class="btn-tts-ctrl" @click="emit('pause')" title="Пауза"><Pause :size="13" /></button>
          <template v-else>
            <button class="btn-tts-ctrl" @click="emit('resume')" title="Продолжить"><Play :size="13" /></button>
            <button class="btn-tts-ctrl danger" @click="emit('cancel')" title="Отменить"><XCircle :size="13" /></button>
          </template>
        </template>
      </div>
      <div class="tts-bar-wrap"><div class="tts-bar" :class="job.status" :style="{ width: percent + '%' }"></div></div>
      <!-- a running job reports an error while it waits for the TTS server -->
      <p v-if="job.errorMsg" class="tts-error">{{ job.errorMsg }}</p>
    </template>

    <div v-else-if="job.status === 'done'" class="tts-label done"><CheckCheck :size="12" /> Аудиокнига готова</div>

    <template v-else-if="job.status === 'failed'">
      <div class="tts-progress-row">
        <span class="tts-label failed"><AlertCircle :size="12" /> Ошибка озвучки</span>
        <button v-if="isAdmin" class="btn-tts-ctrl danger" @click="emit('cancel')" title="Удалить задачу"><XCircle :size="13" /></button>
      </div>
      <p v-if="job.errorMsg" class="tts-error">{{ job.errorMsg }}</p>
      <button v-if="isAdmin" class="btn-tts-start" @click="emit('resume')"><Mic :size="13" /> Повторить</button>
    </template>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue';
import { Mic, Pause, Play, AlertCircle, CheckCheck, XCircle } from 'lucide-vue-next';
import type { TtsJob } from '../api';

const props = defineProps<{
  job?: TtsJob;
  isAdmin: boolean;
}>();

const emit = defineEmits<{
  start: [];
  pause: [];
  /** Continue a paused job or retry a failed one. */
  resume: [];
  cancel: [];
}>();

const percent = computed(() => {
  const job = props.job;
  return job && job.totalChunks > 0 ? Math.round((job.doneChunks / job.totalChunks) * 100) : 0;
});
</script>

<style scoped>
.tts-section { padding: 0.4rem 0.75rem; border-top: 1px solid #1e1e1e; }
.tts-progress-row { display: flex; align-items: center; gap: 0.4rem; margin-bottom: 0.3rem; }
.tts-label { display: flex; align-items: center; gap: 0.3rem; font-size: 0.75rem; flex: 1; }
.tts-label.running { color: #60a5fa; }
.tts-label.paused { color: #fbbf24; }
.tts-label.done { color: #4ade80; padding: 0.2rem 0; }
.tts-label.failed { color: #f87171; }
.tts-error { font-size: 0.7rem; color: #a16262; margin-top: 0.3rem; overflow-wrap: anywhere; display: -webkit-box; -webkit-line-clamp: 3; -webkit-box-orient: vertical; overflow: hidden; }
.tts-bar-wrap { height: 3px; background: #2a2a2a; border-radius: 2px; overflow: hidden; }
.tts-bar { height: 100%; background: #60a5fa; transition: width 0.4s ease; border-radius: 2px; }
.tts-bar.paused { background: #fbbf24; }
.btn-tts-start { display: flex; align-items: center; gap: 0.3rem; width: 100%; justify-content: center; background: #1e2a3a; color: #60a5fa; border: 1px solid #2a3a4a; padding: 0.35rem 0.5rem; border-radius: 6px; cursor: pointer; font-size: 0.8rem; margin-top: 0.3rem; }
.btn-tts-start:hover { background: #243040; border-color: #3a4a5a; }
.btn-tts-ctrl { display: flex; align-items: center; justify-content: center; background: none; color: #555; border: 1px solid #2a2a2a; padding: 0.2rem 0.4rem; border-radius: 5px; cursor: pointer; }
.btn-tts-ctrl:hover { color: #fff; border-color: #444; }
.btn-tts-ctrl.danger { color: #f87171; border-color: #3a1a1a; }
.btn-tts-ctrl.danger:hover { background: #3a1a1a; }
</style>
