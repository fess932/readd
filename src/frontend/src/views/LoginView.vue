<template>
  <main class="login-page">
    <div class="login-card">
      <h1>readd</h1>
      <p class="subtitle">Аудиокниги</p>

      <form @submit.prevent="submit">
        <input
          type="text"
          placeholder="Ваше имя"
          v-model="name"
          :disabled="isPending"
          autofocus
        />
        <p v-if="errorMsg" class="error">{{ errorMsg }}</p>
        <button type="submit" :disabled="isPending || !name.trim()">
          {{ isPending ? 'Входим...' : 'Войти' }}
        </button>
      </form>
    </div>
  </main>
</template>

<script setup lang="ts">
import { ref } from 'vue';
import { useMutation } from '@tanstack/vue-query';
import { useRouter } from 'vue-router';
import { api } from '../api';
import { setAuth } from '../stores/auth';

const router = useRouter();
const name = ref('');
const errorMsg = ref('');

const { mutate: login, isPending } = useMutation({
  mutationFn: (n: string) => api.auth.login(n),
  onSuccess: (res) => {
    setAuth(res.token, res.user);
    router.push('/explore');
  },
  onError: (err: any) => {
    errorMsg.value = err.message ?? 'Ошибка входа';
  },
});

function submit() {
  if (!name.value.trim()) return;
  errorMsg.value = '';
  login(name.value.trim());
}
</script>

<style scoped>
.login-page {
  min-height: 100vh;
  display: flex;
  align-items: center;
  justify-content: center;
  background: #0f0f0f;
}

.login-card {
  background: #1a1a1a;
  border: 1px solid #2a2a2a;
  border-radius: 12px;
  padding: 2rem 2.5rem;
  width: 100%;
  max-width: 360px;
  text-align: center;
}

h1 {
  font-size: 2.5rem;
  font-weight: 800;
  color: #fff;
  margin: 0 0 0.25rem;
  letter-spacing: -0.05em;
}

.subtitle {
  color: #666;
  margin: 0 0 2rem;
  font-size: 0.9rem;
}

input {
  width: 100%;
  padding: 0.75rem 1rem;
  background: #0f0f0f;
  border: 1px solid #333;
  border-radius: 8px;
  color: #fff;
  font-size: 1rem;
  outline: none;
  box-sizing: border-box;
}

input:focus { border-color: #555; }

button {
  width: 100%;
  margin-top: 0.75rem;
  padding: 0.75rem;
  background: #fff;
  color: #000;
  border: none;
  border-radius: 8px;
  font-size: 1rem;
  font-weight: 600;
  cursor: pointer;
}

button:disabled { opacity: 0.4; cursor: not-allowed; }

.error {
  color: #f87171;
  font-size: 0.85rem;
  margin: 0.5rem 0 0;
}
</style>
