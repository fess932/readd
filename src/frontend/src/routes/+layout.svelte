<script lang="ts">
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import { page } from '$app/stores';
  import { auth, clearAuth } from '$lib/auth.svelte';
  import { player } from '$lib/playerState.svelte';
  import Player from '$lib/Player.svelte';
  import Toasts from '$lib/Toasts.svelte';

  let { children } = $props();

  const publicRoutes = ['/login'];

  function checkAuth() {
    const current = $page.url.pathname;
    if (!auth.user && !publicRoutes.includes(current)) goto('/login');
    if (auth.user && current === '/login') goto('/library');
  }

  onMount(checkAuth);
  $effect(() => { checkAuth(); });
</script>

<svelte:head><title>readd</title></svelte:head>

{#if auth.user || $page.url.pathname === '/login'}
  {#if auth.user}
    <nav>
      <a href="/library" class:active={$page.url.pathname === '/library'}>Моя библиотека</a>
      <a href="/explore" class:active={$page.url.pathname === '/explore'}>Все книги</a>
      <span class="spacer"></span>
      <span class="user-name">{auth.user.name}</span>
      <button onclick={() => { clearAuth(); goto('/login'); }} class="logout">Выйти</button>
    </nav>
  {/if}

  <div class="content" class:with-player={!!player.book}>
    {@render children()}
  </div>

  <Player />
  <Toasts />
{/if}

<style>
  :global(*, *::before, *::after) { box-sizing: border-box; margin: 0; padding: 0; }
  :global(body) { background: #0f0f0f; color: #fff; font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif; }
  :global(a) { color: inherit; text-decoration: none; }

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
  .logout { background: none; border: 1px solid #2a2a2a; color: #555; padding: 0.25rem 0.6rem; border-radius: 6px; cursor: pointer; font-size: 0.8rem; }
  .logout:hover { color: #fff; border-color: #444; }

  .content { min-height: calc(100vh - 48px); }
  .content.with-player { padding-bottom: 80px; }
</style>
