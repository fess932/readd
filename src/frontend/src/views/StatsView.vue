<template>
  <main class="page">
    <div class="page-header">
      <h2>Статистика</h2>
    </div>

    <p v-if="isLoading" class="hint">Загрузка...</p>
    <p v-else-if="error" class="error-msg">{{ (error as any).message }}</p>
    <template v-else-if="stats">

      <section class="section">
        <h3 class="section-title">Моя активность</h3>
        <div class="stat-grid">
          <div class="stat-card">
            <div class="stat-value">{{ stats.personal.booksInLibrary }}</div>
            <div class="stat-label">{{ pluralBooks(stats.personal.booksInLibrary) }} в библиотеке</div>
          </div>
          <div class="stat-card">
            <div class="stat-value">{{ formatDuration(stats.personal.listenedSec) || '0 мин' }}</div>
            <div class="stat-label">прослушано</div>
          </div>
          <div class="stat-card">
            <div class="stat-value">{{ stats.personal.favoriteAuthor ?? '—' }}</div>
            <div class="stat-label">любимый автор</div>
          </div>
        </div>
      </section>

      <section class="section">
        <h3 class="section-title">Общая библиотека</h3>
        <div class="stat-grid">
          <div class="stat-card">
            <div class="stat-value">{{ stats.global.totalBooks }}</div>
            <div class="stat-label">{{ pluralBooks(stats.global.totalBooks) }}</div>
          </div>
          <div class="stat-card">
            <div class="stat-value">{{ formatDuration(stats.global.totalSec) || '—' }}</div>
            <div class="stat-label">аудио в фонде</div>
          </div>
          <div class="stat-card">
            <div class="stat-value">{{ stats.global.totalUsers }}</div>
            <div class="stat-label">{{ pluralUsers(stats.global.totalUsers) }}</div>
          </div>
        </div>
      </section>

      <div class="two-col">
        <section class="section">
          <h3 class="section-title">Популярные книги</h3>
          <div class="rank-list">
            <div v-for="(book, i) in stats.global.topBooks" :key="book.id" class="rank-row">
              <span class="rank-num">{{ i + 1 }}</span>
              <img
                :src="book.coverPath ? `/uploads/${book.coverPath}` : '/placeholder.jpg'"
                :alt="book.title"
                class="rank-cover"
              />
              <div class="rank-info">
                <p class="rank-title">{{ book.title }}</p>
                <p class="rank-author">{{ book.author }}</p>
              </div>
              <span class="rank-count">{{ book.libraryCount }} {{ pluralReaders(book.libraryCount) }}</span>
            </div>
            <p v-if="!stats.global.topBooks.length" class="hint-small">Пока никто не добавил книги</p>
          </div>
        </section>

        <section class="section" v-if="stats.global.uploaders.length">
          <h3 class="section-title">Кто загружает</h3>
          <div class="rank-list">
            <div v-for="(u, i) in stats.global.uploaders" :key="u.name" class="rank-row">
              <span class="rank-num">{{ i + 1 }}</span>
              <div class="rank-avatar">{{ u.name[0].toUpperCase() }}</div>
              <div class="rank-info">
                <p class="rank-title">{{ u.name }}</p>
              </div>
              <span class="rank-count">{{ u.booksCount }} {{ pluralBooks(u.booksCount) }}</span>
            </div>
          </div>
        </section>
      </div>

    </template>
  </main>
</template>

<script setup lang="ts">
import { useQuery } from '@tanstack/vue-query';
import { api } from '../api';

const { data: stats, isLoading, error } = useQuery({
  queryKey: ['stats'],
  queryFn: api.stats.get,
});

function formatDuration(sec: number | null): string {
  if (!sec || sec <= 0) return '';
  const h = Math.floor(sec / 3600);
  const m = Math.floor((sec % 3600) / 60);
  if (h > 0) return `${h} ч ${m} мин`;
  return `${m} мин`;
}

function plural(n: number, one: string, few: string, many: string) {
  const mod10 = n % 10, mod100 = n % 100;
  if (mod10 === 1 && mod100 !== 11) return one;
  if (mod10 >= 2 && mod10 <= 4 && (mod100 < 10 || mod100 >= 20)) return few;
  return many;
}

const pluralBooks = (n: number) => plural(n, 'книга', 'книги', 'книг');
const pluralUsers = (n: number) => plural(n, 'пользователь', 'пользователя', 'пользователей');
const pluralReaders = (n: number) => plural(n, 'читатель', 'читателя', 'читателей');
</script>

<style scoped>
.page { max-width: 860px; margin: 0 auto; padding: 1.5rem; }
.page-header { margin-bottom: 1.5rem; }
h2 { font-size: 1.4rem; font-weight: 700; }

.hint { color: #555; text-align: center; padding: 3rem 0; }
.hint-small { color: #555; font-size: 0.85rem; padding: 0.5rem 0; }
.error-msg { color: #f87171; }

.section { margin-bottom: 2rem; }
.section-title { font-size: 0.8rem; font-weight: 600; text-transform: uppercase; letter-spacing: 0.08em; color: #555; margin-bottom: 0.75rem; }

.stat-grid {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 0.75rem;
}
.stat-card {
  background: #1a1a1a;
  border: 1px solid #2a2a2a;
  border-radius: 10px;
  padding: 1rem 1.25rem;
}
.stat-value {
  font-size: 1.6rem;
  font-weight: 700;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.stat-label {
  font-size: 0.78rem;
  color: #555;
  margin-top: 0.25rem;
}

.two-col {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 1.5rem;
}

.rank-list { display: flex; flex-direction: column; gap: 0.5rem; }
.rank-row {
  display: flex;
  align-items: center;
  gap: 0.75rem;
  background: #1a1a1a;
  border: 1px solid #2a2a2a;
  border-radius: 8px;
  padding: 0.6rem 0.75rem;
}
.rank-num { width: 16px; text-align: right; font-size: 0.8rem; color: #444; flex-shrink: 0; }
.rank-cover { width: 36px; height: 36px; border-radius: 4px; object-fit: cover; flex-shrink: 0; }
.rank-avatar {
  width: 36px; height: 36px; border-radius: 50%;
  background: #2a2a2a; color: #888;
  display: flex; align-items: center; justify-content: center;
  font-size: 0.95rem; font-weight: 600; flex-shrink: 0;
}
.rank-info { flex: 1; min-width: 0; }
.rank-title { font-size: 0.85rem; font-weight: 500; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
.rank-author { font-size: 0.75rem; color: #666; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; margin-top: 1px; }
.rank-count { font-size: 0.75rem; color: #555; white-space: nowrap; flex-shrink: 0; }

@media (max-width: 600px) {
  .stat-grid { grid-template-columns: 1fr 1fr; }
  .two-col { grid-template-columns: 1fr; }
}
</style>
