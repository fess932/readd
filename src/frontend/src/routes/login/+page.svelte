<script lang="ts">
  import { goto } from '$app/navigation';
  import { api } from '$lib/api';
  import { setAuth } from '$lib/auth.svelte';

  let name = $state('');
  let loading = $state(false);
  let errorMsg = $state('');

  async function submit() {
    if (!name.trim()) return;
    loading = true;
    errorMsg = '';
    try {
      const res = await api.auth.login(name.trim());
      setAuth(res.token, res.user);
      goto('/explore');
    } catch (e: any) {
      errorMsg = e.message ?? 'Ошибка входа';
    } finally {
      loading = false;
    }
  }
</script>

<main class="login-page">
  <div class="login-card">
    <h1>readd</h1>
    <p class="subtitle">Аудиокниги</p>

    <form onsubmit={(e) => { e.preventDefault(); submit(); }}>
      <input
        type="text"
        placeholder="Ваше имя"
        bind:value={name}
        disabled={loading}
        autofocus
      />
      {#if errorMsg}
        <p class="error">{errorMsg}</p>
      {/if}
      <button type="submit" disabled={loading || !name.trim()}>
        {loading ? 'Входим...' : 'Войти'}
      </button>
    </form>
  </div>
</main>

<style>
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

  input:focus {
    border-color: #555;
  }

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

  button:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  .error {
    color: #f87171;
    font-size: 0.85rem;
    margin: 0.5rem 0 0;
  }
</style>
