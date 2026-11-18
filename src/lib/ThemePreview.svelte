<script lang="ts">
  import AlertCard from './AlertCard.svelte';

  let { id }: { id: string } = $props();

  const due = new Date();
  due.setHours(10, 30, 0, 0);

  // The same component the real alert uses, rendered under this theme's own tokens.
  const sample = {
    title: 'Stand-up with the design team',
    note: 'Room 4B. Bring the revised mockups.',
    priority: 'normal' as const,
    dueAt: due.getTime(),
    repeat: 'none'
  };
</script>

<div class="preview" data-theme={id} aria-hidden="true">
  <div class="scale">
    <AlertCard reminder={sample} status="Now" preview />
  </div>
</div>

<style>
  .preview {
    height: 168px;
    display: grid;
    place-items: center;
    overflow: hidden;
    background: var(--app-bg);
    font-family: var(--font);
    color: var(--text);
  }
  .scale {
    width: 448px;
    zoom: 0.62;
  }
</style>
