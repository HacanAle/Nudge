<script lang="ts">
  import { onMount, tick } from 'svelte';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import { LogicalSize } from '@tauri-apps/api/dpi';
  import { api, errorMessage, on, type GlassSnapshot } from './api';
  import { formatWhen, inMinutes, repeatLabel } from './format';
  import { glassStyle } from './glass';
  import { arrive } from './motion';
  import { parseReminder } from './parse';
  import { settings } from './prefs.svelte';

  /** Must match the quick window `width` in tauri.conf.json. */
  const WINDOW_WIDTH = 608;
  /** Phrases the chips append, for when the text names no time. */
  const SUGGESTIONS = [
    { label: '10 min', phrase: 'in 10 min' },
    { label: '1 hour', phrase: 'in 1 hour' },
    { label: 'This evening', phrase: 'this evening' },
    { label: 'Tomorrow 9:00', phrase: 'tomorrow 9am' }
  ];

  let text = $state('');
  let openId = $state(0);
  let added = $state('');
  let error = $state('');
  let snapshot = $state<GlassSnapshot | null>(null);
  let input = $state<HTMLInputElement>();
  let stage = $state<HTMLDivElement>();
  let saving = false;

  const parsed = $derived(parseReminder(text));
  const stageStyle = $derived(glassStyle(settings.theme, snapshot));
  const passed = $derived(parsed.dueAt !== null && parsed.dueAt <= Date.now());

  async function opened() {
    text = '';
    added = '';
    error = '';
    openId++; // replays the entrance animation
    await tick();
    input?.focus();
  }

  function close() {
    api.hideQuick();
  }

  async function submit() {
    if (saving || added) return;
    error = '';
    if (!parsed.title) {
      error = 'Type what to remind you about.';
      input?.focus();
      return;
    }
    const dueAt = parsed.dueAt ?? inMinutes(60);
    saving = true;
    try {
      await api.add({ title: parsed.title, note: '', priority: parsed.priority, dueAt, repeat: parsed.repeat });
      added = dueAt <= Date.now() ? 'Added. It will pop up right away.' : `Added for ${formatWhen(dueAt)}.`;
      // Close after a beat, unless it has been closed and opened again for the next reminder meanwhile.
      const shown = openId;
      setTimeout(() => {
        if (openId === shown) close();
      }, 900);
    } catch (e) {
      error = errorMessage(e);
    } finally {
      saving = false;
    }
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      e.preventDefault();
      close();
    } else if (e.key === 'Enter') {
      e.preventDefault();
      submit();
    }
  }

  function suggest(phrase: string) {
    text = `${text.trimEnd()} ${phrase}`;
    input?.focus();
  }

  onMount(() => {
    const win = getCurrentWindow();
    const offs = [
      on<null>('quick-open', opened),
      on<GlassSnapshot | null>('glass-snapshot', (s) => (snapshot = s))
    ];

    // The window is exactly as tall as its content.
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
      offs.forEach((off) => off());
      observer.disconnect();
    };
  });
</script>

<svelte:window onfocus={() => input?.focus()} />

<div class="stage" bind:this={stage} style={stageStyle}>
  {#key openId}
    <div class="enter" in:arrive|global>
      <div class="card" role="dialog" aria-label="New reminder">
        <div class="head">
          <span class="status"><span class="dot"></span>New reminder</span>
          <span class="spacer"></span>
          <span class="quiet">{settings.hotkey}</span>
        </div>

        <input
          class="field"
          bind:this={input}
          bind:value={text}
          onkeydown={onKey}
          oninput={() => (error = '')}
          placeholder="call mom tomorrow 5pm"
          maxlength="240"
          autocomplete="off"
          spellcheck="false"
          aria-label="Reminder, with when"
        />

        <p class="line" role="status" aria-live="polite">
          {#if error}
            <span class="err">{error}</span>
          {:else if added}
            <span class="when">{added}</span>
          {:else if !text.trim()}
            <span class="hint">Say it the way you'd say it: “stretch in 20 min”, “standup every weekday 9:30”.</span>
          {:else}
            <span class="what">{parsed.title || 'Untitled'}</span>
            {#if parsed.dueAt !== null}
              <span class="when">{formatWhen(parsed.dueAt)}</span>
              {#if passed}<span class="meta">already passed, so it fires right away</span>{/if}
            {:else}
              <span class="meta">no time found · will use in 1 hour</span>
            {/if}
            {#if parsed.repeat !== 'none'}<span class="meta">{repeatLabel(parsed.repeat)}</span>{/if}
            {#if parsed.priority !== 'normal'}<span class="meta">{parsed.priority === 'high' ? 'High' : 'Low'} priority</span>{/if}
          {/if}
        </p>

        {#if text.trim() && parsed.dueAt === null && !added}
          <div class="chips">
            {#each SUGGESTIONS as s (s.phrase)}
              <button class="chip" onclick={() => suggest(s.phrase)}>{s.label}</button>
            {/each}
          </div>
        {/if}

        <div class="foot">
          <span class="hint keys">Enter to add · Esc to close</span>
          <button class="add" onclick={submit} disabled={!!added}>Add</button>
        </div>
      </div>
    </div>
  {/key}
</div>

<style>
  /* Transparent room around the card for its shadow; the same as the alert (see Overlay.svelte). */
  .stage {
    padding: 16px 40px 64px;
    min-height: 120px;
  }

  .head {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: var(--head-pad);
    background: var(--head-bg);
    color: var(--head-color);
    font-family: var(--mono);
    font-size: 12px;
    white-space: nowrap;
  }
  .status {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    color: var(--accent-text);
    font-weight: 500;
  }
  .dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--accent);
  }
  .quiet {
    color: var(--faint);
  }
  .spacer {
    flex: 1;
  }

  .field {
    width: 100%;
    padding: 12px var(--pad-x) 4px;
    font-family: var(--font);
    font-size: 21px;
    font-weight: 500;
    letter-spacing: -0.01em;
    color: var(--text);
    background: none;
    border: 0;
    outline: none;
  }
  .field::placeholder {
    color: var(--faint);
  }

  .line {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 2px 12px;
    min-height: 26px;
    padding: 0 var(--pad-x);
    font-size: 13.5px;
    color: var(--dim);
  }
  .what {
    font-weight: 600;
    color: var(--text);
  }
  .when {
    font-family: var(--mono);
    font-variant-numeric: tabular-nums;
    color: var(--accent-text);
  }
  .meta {
    color: var(--faint);
  }
  .hint {
    color: var(--faint);
  }
  .err {
    color: var(--danger);
  }

  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    padding: 8px var(--pad-x) 0;
  }
  .chip {
    height: 28px;
    padding: 0 10px;
    font-family: var(--mono);
    font-size: 12px;
    color: var(--btn-color);
    background: var(--btn-bg);
    border: var(--btn-border);
    border-radius: var(--btn-radius);
    box-shadow: var(--btn-shadow);
    transition: background 0.15s, transform 0.1s;
  }
  .chip:hover {
    background: var(--hover);
  }
  .chip:active {
    transform: var(--press);
    box-shadow: var(--btn-shadow-active);
  }

  .foot {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    margin-top: 12px;
    padding: 10px calc(var(--pad-x) - 6px) 10px var(--pad-x);
    background: var(--foot-bg);
    border-top: var(--foot-border);
  }
  .keys {
    font-family: var(--mono);
    font-size: 12px;
  }

  .add {
    height: 32px;
    padding: 0 22px;
    border-radius: var(--btn-radius);
    font-size: 13px;
    font-weight: 600;
    color: var(--primary-color);
    background: var(--primary-bg);
    border: var(--primary-border);
    box-shadow: var(--primary-shadow);
    transition: opacity 0.15s, transform 0.1s, box-shadow 0.1s;
  }
  .add:hover:not(:disabled) {
    opacity: 0.88;
  }
  .add:active:not(:disabled) {
    transform: var(--press);
    box-shadow: var(--primary-shadow-active);
  }
  .add:disabled {
    opacity: 0.5;
    cursor: default;
  }
</style>
