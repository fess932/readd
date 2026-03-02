# readd

Базовое веб-приложение: Elysia.js бекенд + SvelteKit фронтенд.

## Структура

```
readd/
├── src/
│   └── index.ts          # Elysia.js бекенд (порт 3000)
├── src/frontend/          # SvelteKit фронтенд (порт 5173)
├── package.json
└── tsconfig.json
```

## Запуск

**Бекенд:**
```bash
bun dev
```

**Фронтенд** (в отдельном терминале):
```bash
cd src/frontend
bun dev
```

Открыть: http://localhost:5173

## API

- `GET /api/health` — статус сервера
