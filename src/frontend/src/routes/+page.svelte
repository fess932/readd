<script lang="ts">
  import { goto } from '$app/navigation';
  import { auth } from '$lib/auth.svelte';
  import { onMount } from 'svelte';
  onMount(() => goto(auth.user ? '/library' : '/login'));
</script>
