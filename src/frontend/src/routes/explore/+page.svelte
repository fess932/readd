<script lang="ts">
  import { onMount } from 'svelte';
  import { api, type Book } from '$lib/api';
  import { auth } from '$lib/auth.svelte';
  import { toast } from '$lib/toast.svelte';
  import Confirm from '$lib/Confirm.svelte';

  let books = $state<Book[]>([]);
  let loading = $state(true);
  let error = $state('');
  let dialogEl = $state<HTMLDialogElement | null>(null);
  let showUploadModal = $state(false);
  let addingId = $state<number | null>(null);
  let deletingId = $state<number | null>(null);
  let confirmDeleteId = $state<number | null>(null);

  // Upload form
  let uploadTitle = $state('');
  let uploadAuthor = $state('');
  let uploadNarrator = $state('');
  let uploadFiles = $state<File[]>([]);
  let coverPreview = $state<string | null>(null);
  let uploading = $state(false);
  let uploadProgress = $state(0);
  let uploadError = $state('');
  let uploadDone = $state(false);

const AUDIO_EXT = /\.(mp3|m4a|m4b|ogg|flac|wav|aac|opus)$/i;
  const IMAGE_EXT = /\.(jpg|jpeg|png|webp|avif)$/i;
  const COVER_NAME = /^(cover|folder|front|artwork|thumb)/i;

  function parseFolderName(name: string): { title: string; author: string } {
    const clean = name.replace(/\s*[\(\[][^\)\]]*[\)\]]/g, '').trim();
    const dash = clean.match(/^(.+?)\s+[-–—]\s+(.+)$/);
    if (dash) return { author: dash[1].trim(), title: dash[2].trim() };
    return { title: clean, author: '' };
  }

  function handleFolderSelect(e: Event) {
    const input = e.currentTarget as HTMLInputElement;
    const all = Array.from(input.files ?? []);
    if (all.length === 0) return;
    uploadFiles = all;
    uploadDone = false;
    uploadError = '';

    const folderName = all[0].webkitRelativePath.split('/')[0];
    const parsed = parseFolderName(folderName);
    uploadTitle = parsed.title;
    uploadAuthor = parsed.author;

    const coverFile =
      all.find(f => IMAGE_EXT.test(f.name) && COVER_NAME.test(f.name)) ??
      all.find(f => IMAGE_EXT.test(f.name));
    coverPreview = coverFile ? URL.createObjectURL(coverFile) : null;
  }

  function audioCount() {
    return uploadFiles.filter(f => AUDIO_EXT.test(f.name)).length;
  }

  async function submitUpload() {
    if (!uploadFiles.length || !uploadTitle || !uploadAuthor) return;
    uploading = true;
    uploadProgress = 0;
    uploadError = '';
    uploadDone = false;

    // Проверяем дубль ДО загрузки файлов — дёшево, без передачи данных
    try {
      await api.books.check(uploadFiles);
    } catch (e: any) {
      uploadError = e.message;
      toast('error', e.message);
      uploading = false;
      return;
    }

    const fd = new FormData();
    fd.append('title', uploadTitle);
    fd.append('author', uploadAuthor);
    if (uploadNarrator) fd.append('narrator', uploadNarrator);
    for (const f of uploadFiles) fd.append('files', f);

    try {
      const book = await api.books.upload(fd, (pct) => { uploadProgress = pct; });
      books = [book, ...books];
      uploadDone = true;
      setTimeout(() => {
        toast('success', `«${book.title}» загружена`);
        closeModal();
      }, 800);
    } catch (e: any) {
      uploadError = e.message;
      toast('error', e.message);
    } finally {
      uploading = false;
    }
  }

  function closeModal() {
    if (uploading) return;
    dialogEl?.close();
    showUploadModal = false;
    uploadTitle = '';
    uploadAuthor = '';
    uploadNarrator = '';
    uploadFiles = [];
    coverPreview = null;
    uploadProgress = 0;
    uploadError = '';
    uploadDone = false;
  }

  async function loadBooks() {
    loading = true;
    error = '';
    try {
      books = await api.books.list();
    } catch (e: any) {
      error = e.message;
    } finally {
      loading = false;
    }
  }

  async function addToLibrary(bookId: number) {
    addingId = bookId;
    try {
      await api.library.add(bookId);
      toast('success', 'Добавлено в вашу библиотеку');
    } catch (e: any) {
      toast('error', e.message);
    } finally {
      addingId = null;
    }
  }

  async function deleteBook(bookId: number) {
    deletingId = bookId;
    try {
      await api.books.delete(bookId);
      books = books.filter(b => b.id !== bookId);
      toast('success', 'Книга удалена');
    } catch (e: any) {
      toast('error', e.message);
    } finally {
      deletingId = null;
      confirmDeleteId = null;
    }
  }

  function formatDuration(sec: number | null | undefined) {
    if (!sec) return null;
    const h = Math.floor(sec / 3600);
    const m = Math.floor((sec % 3600) / 60);
    return h > 0 ? `${h} ч ${m} мин` : `${m} мин`;
  }

  onMount(loadBooks);
</script>

<main class="page">
  <div class="page-header">
    <h2>Общая библиотека</h2>
    <button class="btn-primary" onclick={() => { showUploadModal = true; dialogEl?.showModal(); }}>
      + Загрузить книгу
    </button>
  </div>

  {#if loading}
    <p class="hint">Загрузка...</p>
  {:else if error}
    <p class="error">{error}</p>
  {:else if books.length === 0}
    <p class="hint">Книг пока нет. Загрузите первую!</p>
  {:else}
    <div class="books-grid">
      {#each books as book (book.id)}
        <div class="book-card">
          {#if book.coverPath}
            <img src="/uploads/{book.coverPath}" alt={book.title} class="book-cover" />
          {:else}
            <div class="book-cover placeholder"></div>
          {/if}
          <div class="book-info">
            <h3>{book.title}</h3>
            <p class="author">{book.author}</p>
            {#if book.narrator}<p class="meta">Читает: {book.narrator}</p>{/if}
            <div class="stats">
              {#if book.chaptersCount > 0}
                <span>{book.chaptersCount} {book.chaptersCount === 1 ? 'глава' : book.chaptersCount < 5 ? 'главы' : 'глав'}</span>
              {/if}
              {#if formatDuration(book.totalSec)}
                <span>{formatDuration(book.totalSec)}</span>
              {/if}
            </div>
          </div>
          <div class="book-actions">
            <button class="btn-add" onclick={() => addToLibrary(book.id)} disabled={addingId === book.id}>
              {addingId === book.id ? '...' : '+ В моё'}
            </button>
            {#if auth.user?.isAdmin}
              <button class="btn-delete" onclick={() => confirmDeleteId = book.id} disabled={deletingId === book.id}>
                {deletingId === book.id ? '...' : 'Удалить'}
              </button>
            {/if}
          </div>
        </div>
      {/each}
    </div>
  {/if}
</main>

<dialog bind:this={dialogEl} class="modal" onclose={closeModal} onclick={(e) => { if (e.target === dialogEl) closeModal(); }}>
  {#if showUploadModal}
    <h3>Загрузить книгу</h3>
    <form onsubmit={(e) => { e.preventDefault(); submitUpload(); }}>

      <label class="folder-drop">
        <span>Выберите папку с книгой</span>
        <input type="file" webkitdirectory onchange={handleFolderSelect} />
      </label>

      {#if uploadFiles.length > 0}
        <div class="folder-summary">
          {#if coverPreview}
            <img src={coverPreview} alt="обложка" class="cover-preview" />
          {:else}
            <div class="cover-preview placeholder"></div>
          {/if}
          <div class="folder-stats">
            <p>{uploadFiles.length} файлов</p>
            <p class="hint-small">{audioCount()} аудио</p>
          </div>
        </div>
      {/if}

      <label>Название * <input type="text" bind:value={uploadTitle} required /></label>
      <label>Автор * <input type="text" bind:value={uploadAuthor} required /></label>
      <label>Диктор <input type="text" bind:value={uploadNarrator} /></label>

      {#if uploading || uploadDone}
        <div class="progress-wrap" class:done={uploadDone}>
          <div class="progress-bar" style="width: {uploadDone ? 100 : uploadProgress}%" class:indeterminate={uploading && uploadProgress === 100 && !uploadDone}></div>
        </div>
        <p class="progress-text">
          {#if uploadDone}
            ✓ Загружено
          {:else if uploadProgress === 100}
            Сохранение на сервере…
          {:else}
            Отправка {uploadProgress}%…
          {/if}
        </p>
      {/if}

      {#if uploadError}
        <div class="upload-error">
          <span>✕</span> {uploadError}
        </div>
      {/if}

      <div class="modal-actions">
        <button type="button" onclick={closeModal} disabled={uploading}>Отмена</button>
        <button
          type="submit"
          class="btn-primary"
          disabled={uploading || uploadDone || !uploadFiles.length || !uploadTitle || !uploadAuthor}
        >
          {uploading ? `${uploadProgress}%` : uploadDone ? 'Готово' : 'Загрузить'}
        </button>
      </div>
    </form>
  {/if}
</dialog>

{#if confirmDeleteId !== null}
  {@const book = books.find(b => b.id === confirmDeleteId)}
  <Confirm
    title="Удалить книгу?"
    message={book ? `«${book.title}» будет удалена из общей библиотеки. Это действие нельзя отменить.` : ''}
    confirmLabel="Удалить"
    danger
    onconfirm={() => deleteBook(confirmDeleteId!)}
    oncancel={() => confirmDeleteId = null}
  />
{/if}

<style>
  /* Layout */
  .page { max-width: 960px; margin: 0 auto; padding: 1.5rem; }
  .page-header { display: flex; align-items: center; justify-content: space-between; margin-bottom: 1.5rem; }
  h2 { font-size: 1.4rem; font-weight: 700; }

  .btn-primary {
    background: #fff; color: #000; border: none;
    padding: 0.5rem 1rem; border-radius: 8px;
    font-weight: 600; cursor: pointer; font-size: 0.9rem;
  }
  .btn-primary:hover { background: #e5e5e5; }
  .btn-primary:disabled { opacity: 0.4; cursor: not-allowed; }

  .hint { color: #555; text-align: center; padding: 3rem 0; }
  .error { color: #f87171; font-size: 0.85rem; }

  /* Grid */
  .books-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(200px, 1fr)); gap: 1rem; }
  .book-card { background: #1a1a1a; border: 1px solid #2a2a2a; border-radius: 10px; overflow: hidden; display: flex; flex-direction: column; }
  .book-cover { width: 100%; aspect-ratio: 1; object-fit: cover; }
  .book-cover.placeholder { background: linear-gradient(135deg, #2a2a2a, #1a1a1a); }
  .book-info { padding: 0.75rem; flex: 1; }
  .book-info h3 { font-size: 0.95rem; font-weight: 600; margin-bottom: 0.25rem; line-height: 1.3; }
  .author { color: #888; font-size: 0.85rem; }
  .meta { color: #555; font-size: 0.8rem; margin-top: 0.1rem; }
  .stats {
    display: flex;
    flex-wrap: wrap;
    gap: 0.3rem 0.5rem;
    margin-top: 0.35rem;
  }
  .stats span {
    font-size: 0.75rem;
    color: #555;
    background: #222;
    padding: 1px 6px;
    border-radius: 4px;
  }
  .book-actions { padding: 0.5rem 0.75rem 0.75rem; display: flex; gap: 0.5rem; }
  .btn-add { flex: 1; background: #2a2a2a; color: #fff; border: none; padding: 0.4rem 0.5rem; border-radius: 6px; cursor: pointer; font-size: 0.8rem; }
  .btn-add:hover:not(:disabled) { background: #333; }
  .btn-delete { background: #3a1a1a; color: #f87171; border: none; padding: 0.4rem 0.5rem; border-radius: 6px; cursor: pointer; font-size: 0.8rem; }
  .btn-delete:hover:not(:disabled) { background: #4a1a1a; }
  button:disabled { opacity: 0.5; cursor: not-allowed; }

  /* Dialog */
  .modal {
    background: #1a1a1a;
    border: 1px solid #2a2a2a;
    border-radius: 12px;
    padding: 1.5rem;
    width: min(420px, calc(100vw - 2rem));
    color: #fff;
    /* Center in viewport */
    position: fixed;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    margin: 0;
  }
  .modal::backdrop { background: rgba(0,0,0,0.7); }
  .modal h3 { margin-bottom: 1rem; font-size: 1.1rem; }

  .folder-drop {
    display: flex; flex-direction: column; align-items: center; justify-content: center;
    gap: 0.5rem; border: 2px dashed #333; border-radius: 8px;
    padding: 1.5rem; cursor: pointer; margin-bottom: 1rem;
    color: #888; font-size: 0.9rem; transition: border-color 0.15s;
  }
  .folder-drop:hover { border-color: #555; color: #aaa; }
  .folder-drop input { display: none; }

  .folder-summary { display: flex; align-items: center; gap: 0.75rem; background: #0f0f0f; border-radius: 8px; padding: 0.5rem 0.75rem; margin-bottom: 0.75rem; }
  .cover-preview { width: 48px; height: 48px; border-radius: 6px; object-fit: cover; flex-shrink: 0; }
  .cover-preview.placeholder { background: linear-gradient(135deg, #2a2a2a, #1a1a1a); }
  .folder-stats p { font-size: 0.9rem; color: #ccc; margin: 0; }
  .hint-small { color: #555 !important; font-size: 0.8rem !important; }

  .modal label { display: flex; flex-direction: column; gap: 0.3rem; margin-bottom: 0.75rem; font-size: 0.85rem; color: #888; }
  .modal input[type="text"] { background: #0f0f0f; border: 1px solid #333; border-radius: 6px; padding: 0.5rem 0.75rem; color: #fff; font-size: 0.95rem; outline: none; }
  .modal input[type="text"]:focus { border-color: #555; }

  .progress-wrap {
    height: 4px; background: #2a2a2a; border-radius: 2px;
    margin-bottom: 0.4rem; overflow: hidden;
  }
  .progress-bar { height: 100%; background: #fff; transition: width 0.15s ease; }
  .progress-wrap.done .progress-bar { background: #4ade80; }

  .progress-bar.indeterminate {
    width: 40% !important;
    background: #888;
    animation: indeterminate 1.2s ease-in-out infinite;
  }
  @keyframes indeterminate {
    0%   { margin-left: -40%; }
    100% { margin-left: 100%; }
  }

  .progress-text { font-size: 0.8rem; color: #888; margin-bottom: 0.75rem; }
  .progress-wrap.done + .progress-text { color: #4ade80; }

  .upload-error {
    display: flex; align-items: center; gap: 0.4rem;
    background: #3a1a1a; color: #f87171;
    border-radius: 6px; padding: 0.5rem 0.75rem;
    font-size: 0.85rem; margin-bottom: 0.75rem;
  }

  .modal-actions { display: flex; gap: 0.75rem; margin-top: 1rem; justify-content: flex-end; }
  .modal-actions button { background: #2a2a2a; color: #fff; border: none; padding: 0.5rem 1rem; border-radius: 6px; cursor: pointer; font-size: 0.9rem; }
  .modal-actions .btn-primary { background: #fff; color: #000; font-weight: 600; }
</style>
