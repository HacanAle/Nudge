<script lang="ts">
  // A note's text with its web and mail links clickable (they open in the default browser / mail app).
  // The markup below is one line on purpose: the note keeps its own whitespace (pre-line), so the template
  // must add none.
  import { api } from './api';
  import { shortLink, splitLinks } from './links';

  let { text }: { text: string } = $props();

  const parts = $derived(splitLinks(text));

  function open(e: MouseEvent, href: string) {
    // Never navigate the webview itself (left, middle or modified click): hand the link to Windows.
    e.preventDefault();
    e.stopPropagation();
    if (e.button <= 1) api.openLink(href).catch(() => {});
  }
</script>

{#each parts as part, i (i)}{#if part.href}<a class="link" href={part.href} title={part.href} draggable="false" onclick={(e) => open(e, part.href!)} onauxclick={(e) => open(e, part.href!)}>{shortLink(part.href)}</a>{:else}{part.text}{/if}{/each}

<style>
  .link {
    color: var(--accent-text);
    text-decoration: underline;
    text-decoration-color: color-mix(in srgb, currentColor 45%, transparent);
    text-underline-offset: 2px;
    cursor: pointer;
    overflow-wrap: anywhere;
  }
  .link:hover {
    text-decoration-color: currentColor;
  }
</style>
