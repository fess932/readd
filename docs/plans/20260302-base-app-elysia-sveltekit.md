# Base App: Elysia.js Backend + SvelteKit Frontend

## Overview
- Базовое веб-приложение с Elysia.js бекендом и SvelteKit фронтендом
- Backend: Elysia.js (Bun runtime) в `src/`
- Frontend: SvelteKit в `src/frontend/`
- Структура готова для дальнейшего развития

## Context
- Проект: `/Users/fess932/git/readd`
- Runtime: Bun (для Elysia.js)
- Язык: TypeScript

## Development Approach
- Regular (code first)
- Минимальная базовая структура, без лишнего

## Implementation Steps

### Task 1: Инициализировать Bun-проект (root)
- [x] создать `package.json` с Elysia.js зависимостью (bun init)
- [x] создать `src/index.ts` — точка входа Elysia сервера
- [x] добавить базовый health-check маршрут `GET /api/health`
- [x] добавить `tsconfig.json` для TypeScript
- [x] запустить `bun install` и убедиться что сервер стартует

### Task 2: Инициализировать SvelteKit фронтенд
- [x] создать SvelteKit проект в `src/frontend/` (`bunx sv create`)
- [x] убедиться что фронтенд запускается (`bun dev`)
- [x] добавить базовую страницу `src/frontend/src/routes/+page.svelte`

### Task 3: Связать фронтенд с бекендом
- [x] добавить fetch вызов к `GET /api/health` на главной странице фронтенда
- [x] отобразить результат на странице
- [x] проверить что всё работает вместе

### Task 4: [Final] Документация и запуск
- [x] обновить README.md с инструкциями запуска
- [x] убедиться что оба процесса запускаются корректно

## Technical Details

```
readd/
├── src/
│   └── index.ts          # Elysia.js сервер (порт 3000)
├── src/frontend/          # SvelteKit приложение (порт 5173)
│   └── src/routes/
│       └── +page.svelte
├── package.json           # Bun workspace root
└── tsconfig.json
```

- Backend порт: `3000`
- Frontend dev порт: `5173`
- API prefix: `/api`

## Post-Completion
- Настроить прокси в SvelteKit для продакшен сборки (vite.config proxy)
- Рассмотреть Bun workspaces для monorepo управления
