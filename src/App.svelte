<script lang="ts">
  import Overlay from './lib/Overlay.svelte';
  import Manager from './lib/Manager.svelte';
  import Quick from './lib/Quick.svelte';

  let { view }: { view: string } = $props();
</script>

{#if view === 'overlay'}
  <Overlay />
{:else if view === 'quick'}
  <Quick />
{:else}
  <Manager />
{/if}
