# readd — Feature Roadmap

## Overview

Набор улучшений аудиокнижного приложения readd.
**Phase 1 и Phase 2 реализуются сейчас.** Phase 3 — задокументирована, реализуется позже.

Стек: Rust/Axum бэкенд (`server/src/`), Vue 3 фронтенд (`src/frontend/src/`), SQLite.

---

## Phase 1 — Frontend only

### Task 1: Скорость воспроизведения ✅
- [x] Добавить `speed: number` в `stores/player.ts`, инициализация из `localStorage`
- [x] В `Player.vue` применять `audio.playbackRate = player.speed` в `onLoadedMetadata`
- [x] Кнопка скорости в правой части плеера, цикл: 0.75→1→1.25→1.5→2
- [x] Сохранять в `localStorage` при изменении

### Task 2: Поиск по книгам
Строка поиска на странице "Все книги" (ExploreView), фильтрует по названию и автору через computed.

- [ ] Добавить `searchQuery` ref в `ExploreView.vue`
- [ ] Computed `filteredBooks` — фильтрует `books` по `searchQuery` (case-insensitive, title + author)
- [ ] Заменить использование `books` на `filteredBooks` в шаблоне (в обоих режимах — flat и grouped)
- [ ] Добавить `<input>` поиска в шапку страницы рядом с кнопками
- [ ] Сбрасывать `searchQuery` при очистке инпута крестиком (кнопка `×` если есть текст)

### Task 3: Таймер сна
Останавливает воспроизведение через N минут. Показывает обратный отсчёт в плеере.

- [ ] Добавить `sleepTimer: number | null` (секунды до остановки) и `sleepInterval` в `stores/player.ts`
- [ ] Функция `setSleepTimer(minutes: number | null)` — запускает/останавливает countdown
- [ ] Каждую секунду декрементить таймер; при достижении 0 — `player.playing = false`, сохранить прогресс
- [ ] В `Player.vue` добавить кнопку таймера с выпадающим меню: 15 мин, 30 мин, 60 мин, Отмена
- [ ] Показывать обратный отсчёт (`MM:SS`) рядом с кнопкой когда таймер активен

### Task 4: Горячие клавиши ✅
- [x] В `Player.vue` глобальный `keydown` listener с cleanup
- [x] `Space` — play/pause, игнорируется на input/textarea
- [x] `←` / `→` — перемотка ±30 секунд
- [x] `[` / `]` — предыдущая/следующая глава
- [x] Игнорировать если `!player.book`

---

## Phase 2 — Small backend changes

### Task 5: Закладки
Новая таблица `bookmarks`. Кнопка в плеере, список в библиотеке.

**Backend (Rust):**
- [ ] Добавить таблицу в `server/src/db.rs`:
  ```sql
  CREATE TABLE IF NOT EXISTS bookmarks (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      user_id INTEGER NOT NULL REFERENCES users(id),
      book_id INTEGER NOT NULL REFERENCES books(id),
      chapter_path TEXT NOT NULL,
      position_sec REAL NOT NULL,
      label TEXT NOT NULL DEFAULT '',
      created_at TEXT NOT NULL DEFAULT (datetime('now'))
  )
  ```
- [ ] Создать `server/src/routes/bookmarks.rs` с хендлерами:
  - `GET /api/bookmarks/:bookId` — список закладок книги для текущего юзера
  - `POST /api/bookmarks/:bookId` — создать закладку `{ chapterPath, positionSec, label? }`
  - `DELETE /api/bookmarks/:id` — удалить закладку
- [ ] Зарегистрировать роуты в `server/src/routes/mod.rs`

**Frontend:**
- [ ] Добавить методы `api.bookmarks.*` в `src/frontend/src/api.ts` + интерфейс `Bookmark`
- [ ] В `Player.vue` добавить кнопку закладки (иконка bookmark) — открывает мини-попап с полем label и кнопкой сохранить
- [ ] В `LibraryView.vue` при раскрытии книги показывать список её закладок с кнопкой "перейти"

### Task 6: Замена обложки ✅
- [x] `PATCH /api/books/:id/cover` — multipart, сохраняет в директорию книги
- [x] `api.books.uploadCover` в `api.ts`
- [x] Кнопка на обложке при наведении (только админ), один shared file input
- [x] Обновление кэша TanStack Query после загрузки

### Task 7: Отметить как прочитанное ✅
- [x] `ALTER TABLE user_library ADD COLUMN finished_at TEXT` при старте (с игнором ошибки)
- [x] `POST /api/library/:bookId/finish` — toggle finished_at
- [x] `finishedAt` в ответе списка библиотеки
- [x] Кнопка ✓ на каждой книге в LibraryView, секция "Прочитано" внизу

---

## Phase 3 — Крупные фичи (план, реализация позже)

### 8. Стрики прослушивания
- Новая таблица `listening_days(user_id, date TEXT)` — уникальная запись на каждый день активности
- При сохранении прогресса (`POST /api/progress/:bookId`) — upsert в `listening_days`
- Страница статистики: показывать текущий стрик и максимальный

### 9. Лента "сейчас слушают"
- Эндпоинт `GET /api/feed` — последний прогресс каждого юзера (JOIN users + progress + books)
- Показывать на странице "Все книги" или отдельной вкладке: аватар юзера, книга, глава

### 10. Архив завершённых книг ✅
- [x] В `onEnded` Player.vue: если последняя глава — вызвать `api.library.finish()` и инвалидировать кэш библиотеки
- Секция "Прочитано" реализована в Task 7

---

## Технические детали

**Player store** (`stores/player.ts`): добавить `speed`, `sleepTimer`.
**DB schema** (`server/src/db.rs`): новые таблицы через `CREATE TABLE IF NOT EXISTS`, колонки через `ALTER TABLE ... ADD COLUMN IF NOT EXISTS` (SQLite не поддерживает `IF NOT EXISTS` в ALTER — обернуть в `execute` с игнором ошибки).
**Маршруты** (`server/src/routes/mod.rs`): регистрировать все новые роуты здесь.
**API client** (`src/frontend/src/api.ts`): все новые методы и типы добавлять сюда.
