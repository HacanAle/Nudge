<script lang="ts">
  import { onMount } from 'svelte';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import { LogicalSize } from '@tauri-apps/api/dpi';
  import { api, on, type GlassSnapshot, type Reminder } from './api';
  import AlertCard from './AlertCard.svelte';
  import { formatRelative } from './format';
  import { glassStyle } from './glass';
  import { arrive } from './motion';
  import { settings } from './prefs.svelte';

  /** Must match the overlay `width` in tauri.conf.json. */
  const WINDOW_WIDTH = 528;
  const DEFAULT_SNOOZE_MIN = 10;

  let alerts = $state<Reminder[]>([]);
  let now = $state(Date.now());
  let expanded = $state(false);
  let snapshot = $state<GlassSnapshot | null>(null);
  let stage = $state<HTMLDivElement>();
  let busy = false;

  const current = $derived(alerts[0]);

  const status = $derived.by(() => {
    if (!current) return '';
    return now - current.dueAt < 45_000 ? 'Now' : formatRelative(current.dueAt, now);
  });

  const stageStyle = $derived(glassStyle(settings.theme, snapshot));

  async function run(task: () => Promise<unknown>) {
    if (busy || !current) return;
    busy = true;
    try {
      await task();
    } catch (e) {
      console.error(e);
    } finally {
      busy = false;
      expanded = false;
    }
  }

  function done() {
    const id = current?.id;
    if (id !== undefined) run(() => api.finish(id, false));
  }

  function dismiss() {
    const id = current?.id;
    if (id !== undefined) run(() => api.finish(id, true));
  }

  function snoozeUntil(until: number) {
    const id = current?.id;
    if (id !== undefined) run(() => api.snooze(id, until));
  }

  const snoozeFor = (minutes: number) => snoozeUntil(Date.now() + minutes * 60_000);

  /** Keyboard shortcuts work once the popup has been clicked (it never grabs focus by itself). */
  function onKey(e: KeyboardEvent) {
    if (!current || e.ctrlKey || e.metaKey || e.altKey) return;
    const onButton = (e.target as HTMLElement | null)?.closest('button');
    if (e.key === 'Enter' && !onButton) {
      e.preventDefault();
      done();
    } else if (e.key === 'Escape') {
      if (expanded) expanded = false;
      else dismiss();
    } else if (e.key.toLowerCase() === 's' && !onButton) {
      snoozeFor(DEFAULT_SNOOZE_MIN);
    }
  }

  /** Once the last card has finished animating out, let the backend hide the (now empty) window. */
  function afterOutro() {
    if (alerts.length === 0) api.hideOverlay();
  }

  onMount(() => {
    const win = getCurrentWindow();
    api.getActive().then((list) => (alerts = list));
    const offAlerts = on<Reminder[]>('alerts-changed', (list) => (alerts = list));
    const offGlass = on<GlassSnapshot | null>('glass-snapshot', (s) => (snapshot = s));
    const clock = setInterval(() => (now = Date.now()), 15_000);

    // The window is exactly as tall as its content (card + snooze options).
    let lastHeight = 0;
    const observer = new ResizeObserver(() => {
      if (!stage) return;
      const height = Math.ceil(stage.getBoundingClientRect().height);
      if (height !== lastHeight) {
        lastHeight = height;
        win.setSize(new LogicalSize(WINDOW_WIDTH, height)).catch(() => {});
      }
    });
    if (stage) observer.observe(stage);

    return () => {
      offAlerts();
      offGlass();
      clearInterval(clock);
      observer.disconnect();
    };
  });
</script>

<svelte:window onkeydown={onKey} />

<div class="stage" bind:this={stage} style={stageStyle}>
  {#if current}
    {#key current.id}
      <div class="enter" in:arrive|global out:arrive|global={{ leaving: true }} onoutroend={afterOutro}>
        <AlertCard
          reminder={current}
          {status}
          count={alerts.length}
          snoozeMinutes={DEFAULT_SNOOZE_MIN}
          bind:expanded
          onDone={done}
          onDismiss={dismiss}
          onSnooze={snoozeFor}
          onSnoozeUntil={snoozeUntil}
        />
      </div>
    {/key}
  {/if}
</div>

<style>
  /* Transparent room around the card for its shadow: the softest themes' shadows reach ~40 px to the sides and
     ~65 px below, and anything cut off at the window edge shows as a faint grey box. Keep in step with Quick
     (the glass theme's --snap-x/--snap-y assume this padding). */
  .stage {
    display: grid;
    align-items: start;
    padding: 16px 40px 64px;
    min-height: 150px;
  }
  /* Answering one of several alerts swaps the card: the leaving and arriving cards share one cell, so the
     next card takes the old one's place instead of appearing below it and making the window jump. */
  .enter {
    grid-area: 1 / 1;
    min-width: 0;
  }
</style>
