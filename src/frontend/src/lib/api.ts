import { auth } from './auth.svelte';

// В dev-режиме загрузка идёт напрямую на бэкенд, минуя Vite proxy
// (http-proxy зависает на повторных больших загрузках).
// В prod фронт и бэкенд на одном сервере — relative URL работает.
const API_BASE = import.meta.env.DEV ? 'http://localhost:3000' : '';

async function request<T>(path: string, options: RequestInit = {}): Promise<T> {
  const headers: Record<string, string> = { ...(options.headers as Record<string, string>) };
  if (auth.token) headers['Authorization'] = `Bearer ${auth.token}`;
  if (!(options.body instanceof FormData)) headers['Content-Type'] = 'application/json';

  const res = await fetch(API_BASE + path, { ...options, headers });
  if (!res.ok) {
    const err = await res.json().catch(() => ({ error: res.statusText }));
    throw new Error(err.error ?? res.statusText);
  }
  return res.json();
}

export const api = {
  auth: {
    login: (name: string) =>
      request<{ token: string; user: { id: number; name: string; isAdmin: boolean } }>(
        '/api/auth/login', { method: 'POST', body: JSON.stringify({ name }) }
      ),
  },
  books: {
    list: () => request<Book[]>('/api/books'),
    check: (files: File[]) =>
      request<{ ok: boolean }>('/api/books/check', {
        method: 'POST',
        body: JSON.stringify({ files: files.map(f => ({ name: f.name, size: f.size })) }),
      }),
    upload: (data: FormData, onProgress?: (pct: number) => void) =>
      new Promise<Book>((resolve, reject) => {
        const xhr = new XMLHttpRequest();
        xhr.open('POST', API_BASE + '/api/books');
        if (auth.token) xhr.setRequestHeader('Authorization', `Bearer ${auth.token}`);
        xhr.upload.onprogress = (e) => {
          if (e.lengthComputable) onProgress?.(Math.round(e.loaded / e.total * 100));
        };
        xhr.onload = () => {
          if (xhr.status >= 200 && xhr.status < 300) {
            resolve(JSON.parse(xhr.responseText));
          } else {
            let msg = `${xhr.status} ${xhr.statusText}`;
            try {
              const body = JSON.parse(xhr.responseText);
              if (body?.error) msg = body.error;
            } catch { /* не JSON — оставляем статус-строку */ }
            reject(new Error(msg));
          }
        };
        xhr.onerror = () => reject(new Error('Ошибка сети'));
        xhr.onabort = () => reject(new Error('Загрузка прервана'));
        xhr.send(data);
      }),
    delete: (id: number) => request<{ ok: boolean }>(`/api/books/${id}`, { method: 'DELETE' }),
  },
  library: {
    list: () => request<LibraryBook[]>('/api/library'),
    get: (bookId: number) => request<LibraryBook>(`/api/library/${bookId}`),
    add: (bookId: number) => request<{ ok: boolean }>(`/api/library/${bookId}`, { method: 'POST' }),
    remove: (bookId: number) => request<{ ok: boolean }>(`/api/library/${bookId}`, { method: 'DELETE' }),
  },
  progress: {
    last: () => request<LastProgress | null>('/api/progress/last'),
    get: (bookId: number) => request<Progress | null>(`/api/progress/${bookId}`),
    save: (bookId: number, body: { chapterPath: string; positionSec: number; chapterDuration?: number }) =>
      request<{ ok: boolean }>(`/api/progress/${bookId}`, { method: 'POST', body: JSON.stringify(body) }),
  },
};

export interface Chapter {
  id: number;
  filePath: string;
  sortOrder: number;
  durationSec: number | null;
}

export interface Progress {
  bookId: number;
  chapterPath: string;
  positionSec: number;
}

export interface LastProgress extends Progress {
  title: string;
  author: string;
  coverPath?: string | null;
  updatedAt: string;
}

export interface Book {
  id: number;
  title: string;
  author: string;
  narrator?: string;
  coverPath?: string | null;
  filePath: string;
  uploadedBy?: string;
  createdAt: string;
  chaptersCount: number;
  totalSec: number | null;
}

export interface LibraryBook extends Book {
  addedAt: string;
  chapters: Chapter[];
  progress: Progress | null;
}
