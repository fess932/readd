<template>
  <nav v-if="auth.user">
    <router-link to="/library" :class="{ active: route.path === '/library' }">Моя библиотека</router-link>
    <router-link to="/explore" :class="{ active: route.path === '/explore' }">Все книги</router-link>
    <router-link to="/stats" :class="{ active: route.path === '/stats' }">Статистика</router-link>
    <span class="spacer"></span>
    <span class="user-name">{{ auth.user.name }}</span>
    <button @click="logout" class="logout" title="Выйти"><LogOut :size="15" /></button>
  </nav>

  <div class="content" :class="{ 'with-player': !!player.book }">
    <router-view />
  </div>

  <Player />
  <Toasts />
</template>

<script setup lang="ts">
import { useRoute, useRouter } from 'vue-router';
import { LogOut } from 'lucide-vue-next';
import { auth, clearAuth } from './stores/auth';
import { player } from './stores/player';
import Player from './components/Player.vue';
import Toasts from './components/Toasts.vue';

const route = useRoute();
const router = useRouter();

function logout() {
  clearAuth();
  router.push('/login');
}
</script>

<style>
*, *::before, *::after { box-sizing: border-box; margin: 0; padding: 0; }
body { background: #0f0f0f; color: #fff; font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif; }
a { color: inherit; text-decoration: none; }
</style>

<style scoped>
nav {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  padding: 0.75rem 1.5rem;
  background: #1a1a1a;
  border-bottom: 1px solid #2a2a2a;
}
nav a { color: #666; font-weight: 500; padding: 0.25rem 0.6rem; border-radius: 6px; font-size: 0.9rem; transition: color 0.15s; }
nav a.active, nav a:hover { color: #fff; }
.spacer { flex: 1; }
.user-name { color: #444; font-size: 0.85rem; }
.logout { display: flex; align-items: center; background: none; border: 1px solid #2a2a2a; color: #555; padding: 0.3rem; border-radius: 6px; cursor: pointer; }
.logout:hover { color: #fff; border-color: #444; }

.content { min-height: calc(100vh - 48px); }
.content.with-player { padding-bottom: 80px; }
</style>
