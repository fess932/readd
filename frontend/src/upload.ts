// Uploading an audiobook: files go in small parallel batches, each in its own request.
//
// One request for a whole book (a gigabyte) dies with any hiccup on the way and the
// browser will not say why. Separate requests can be retried one by one, a stalled one
// can be detected and restarted, and a failure can name the file and the reason.

import { API_BASE, request, type Book } from './api';
import { auth } from './stores/auth';
import { formatSize } from './utils/format';

/** Files in flight at once: enough to keep the line busy, few enough not to choke it. */
const PARALLEL = 3;
/** Pauses before the 2nd, 3rd and 4th try of a file. */
const RETRY_DELAYS_MS = [1000, 3000, 8000];
/** No bytes moving for this long means the request is stuck. */
const STALL_MS = 30_000;

export interface BookUpload {
  title: string;
  author: string;
  narrator?: string;
  files: File[];
}

export interface UploadProgress {
  /** Share of all bytes sent, 0–100. */
  percent: number;
  filesDone: number;
  filesTotal: number;
}

type Failure =
  | { kind: 'http'; status: number; message: string }
  | { kind: 'network' }
  | { kind: 'stalled' }
  | { kind: 'cancelled' };

const pathOf = (file: File) => file.webkitRelativePath || file.name;
const sleep = (ms: number) => new Promise(resolve => setTimeout(resolve, ms));

/** Sends one file as the body of a PUT. Rejects with a `Failure`. */
function putFile(url: string, file: File, onSent: (bytes: number) => void, signal: AbortSignal): Promise<void> {
  return new Promise((resolve, reject) => {
    const xhr = new XMLHttpRequest();
    let failure: Failure = { kind: 'network' };
    let watchdog = 0;

    const abortWith = (reason: Failure) => {
      failure = reason;
      xhr.abort();
    };
    const armWatchdog = () => {
      clearTimeout(watchdog);
      watchdog = window.setTimeout(() => abortWith({ kind: 'stalled' }), STALL_MS);
    };
    const onCancel = () => abortWith({ kind: 'cancelled' });
    const finish = (settle: () => void) => {
      clearTimeout(watchdog);
      signal.removeEventListener('abort', onCancel);
      settle();
    };

    xhr.open('PUT', url);
    if (auth.token) xhr.setRequestHeader('Authorization', `Bearer ${auth.token}`);
    xhr.setRequestHeader('Content-Type', 'application/octet-stream');
    xhr.upload.onprogress = (e) => {
      armWatchdog();
      onSent(e.loaded);
    };
    xhr.onload = () => finish(() => {
      if (xhr.status >= 200 && xhr.status < 300) return resolve();
      let message = xhr.statusText;
      try { message = JSON.parse(xhr.responseText).error ?? message; } catch { /* not JSON: a proxy's error page */ }
      reject({ kind: 'http', status: xhr.status, message } satisfies Failure);
    });
    xhr.onerror = () => finish(() => reject(failure));
    xhr.onabort = () => finish(() => reject(failure));

    signal.addEventListener('abort', onCancel);
    if (signal.aborted) return onCancel();
    armWatchdog();
    xhr.send(file);
  });
}

/** Whether trying the same file again can help. */
function isRetryable(failure: Failure): boolean {
  if (failure.kind === 'cancelled') return false;
  if (failure.kind === 'http') return failure.status >= 500 || failure.status === 408 || failure.status === 429;
  return true;
}

/** A message that names the file and says what actually went wrong. */
async function describe(file: File, failure: Failure, sentBytes: number): Promise<string> {
  const name = `«${pathOf(file)}»`;
  const attempts = `${RETRY_DELAYS_MS.length + 1} попытки`;
  const progress = `${formatSize(sentBytes)} из ${formatSize(file.size)}`;

  if (failure.kind === 'http') {
    if (failure.status === 413) {
      return `${name} (${formatSize(file.size)}): прокси перед сервером не пропускает запросы такого размера (413)`;
    }
    return `${name}: сервер ответил ${failure.status} — ${failure.message || 'без пояснения'}`;
  }
  if (failure.kind === 'stalled') {
    return `${name}: передача зависает (${progress}), ${attempts}. Проверьте сеть или прокси перед сервером`;
  }

  // The browser reveals nothing about a broken connection, so ask the server whether it is alive
  const serverAlive = await fetch(`${API_BASE}/api/health`).then(r => r.ok, () => false);
  return serverAlive
    ? `${name}: соединение обрывается во время передачи (${progress}), ${attempts}. Сам сервер отвечает — запрос рвёт сеть или прокси`
    : `${name}: нет связи с сервером, ${attempts}`;
}

/**
 * Uploads the files and creates the book. `signal` cancels it; a cancelled or failed
 * upload is removed from the server.
 */
export async function uploadBook(
  upload: BookUpload,
  onProgress: (progress: UploadProgress) => void,
  signal: AbortSignal,
): Promise<Book> {
  const { files } = upload;
  const { uploadId } = await request<{ uploadId: string }>('/api/books/uploads', { method: 'POST' });
  const base = `${API_BASE}/api/books/uploads/${uploadId}`;

  const totalBytes = files.reduce((sum, f) => sum + f.size, 0) || 1;
  const sent = files.map(() => 0);
  let filesDone = 0;
  const report = () => onProgress({
    percent: Math.min(100, Math.round((sent.reduce((a, b) => a + b, 0) / totalBytes) * 100)),
    filesDone,
    filesTotal: files.length,
  });

  // One failed file stops the others: no point in sending the rest of a book that cannot complete
  const stopOthers = new AbortController();
  const onCancel = () => stopOthers.abort();
  signal.addEventListener('abort', onCancel);

  async function sendWithRetries(index: number) {
    const file = files[index];
    for (let attempt = 0; ; attempt++) {
      try {
        await putFile(`${base}/files/${index}`, file, (bytes) => { sent[index] = bytes; report(); }, stopOthers.signal);
        sent[index] = file.size;
        filesDone++;
        report();
        return;
      } catch (err) {
        const failure = err as Failure;
        if (!isRetryable(failure) || attempt >= RETRY_DELAYS_MS.length) {
          if (failure.kind === 'cancelled') throw new Error('Загрузка отменена');
          throw new Error(await describe(file, failure, sent[index]));
        }
        sent[index] = 0;
        report();
        await sleep(RETRY_DELAYS_MS[attempt]);
        if (stopOthers.signal.aborted) throw new Error('Загрузка отменена');
      }
    }
  }

  try {
    // A fixed number of workers pull files off a shared queue
    let next = 0;
    const worker = async () => {
      while (next < files.length) await sendWithRetries(next++);
    };
    await Promise.all(Array.from({ length: Math.min(PARALLEL, files.length) }, worker));

    return await request<Book>(`/api/books/uploads/${uploadId}/finish`, {
      method: 'POST',
      body: JSON.stringify({
        title: upload.title,
        author: upload.author,
        narrator: upload.narrator,
        files: files.map((f, index) => ({ index, name: pathOf(f), size: f.size })),
      }),
    });
  } catch (err) {
    stopOthers.abort();
    // Best effort: the server sweeps abandoned uploads anyway
    request(`/api/books/uploads/${uploadId}`, { method: 'DELETE' }).catch(() => {});
    throw err;
  } finally {
    signal.removeEventListener('abort', onCancel);
  }
}
