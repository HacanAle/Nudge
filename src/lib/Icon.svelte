<script lang="ts" module>
  // Deliberately tiny: window controls and the snooze disclosure. Everything else is words.
  const PATHS = {
    x: '<path d="M3.5 3.5l9 9M12.5 3.5l-9 9"/>',
    minus: '<path d="M3 8h10"/>',
    chevron: '<path d="M4 6.25l4 4 4-4"/>'
  } as const;

  export type IconName = keyof typeof PATHS;
</script>

<script lang="ts">
  let { name, size = 16 }: { name: IconName; size?: number } = $props();
</script>

<svg
  viewBox="0 0 16 16"
  width={size}
  height={size}
  fill="none"
  stroke="currentColor"
  stroke-width="1.4"
  stroke-linecap="square"
  stroke-linejoin="miter"
  aria-hidden="true"
>
  {@html PATHS[name]}
</svg>
