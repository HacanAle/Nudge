<script lang="ts">
  import { slide } from 'svelte/transition';
  import { atHour, formatTime, repeatLabel } from './format';
  import { api, type Priority } from './api';
  import Icon from './Icon.svelte';
  import { meetingLink } from './links';
  import NoteText from './NoteText.svelte';

  interface Props {
    reminder: { title: string; note: string; priority: Priority; dueAt: number; repeat: string };
    /** "Now", "5 min ago"… */
    status: string;
    /** How many alerts are waiting in total (shows "1 of N" above 1). */
    count?: number;
    snoozeMinutes?: number;
    expanded?: boolean;
    /** A static, non-interactive rendering (theme previews). */
    preview?: boolean;
    onDone?: () => void;
    onDismiss?: () => void;
    onSnooze?: (minutes: number) => void;
    onSnoozeUntil?: (at: number) => void;
  }

  let {
    reminder,
    status,
    count = 1,
    snoozeMinutes = 10,
    expanded = $bindable(false),
    preview = false,
    onDone,
    onDismiss,
    onSnooze,
    onSnoozeUntil
  }: Props = $props();

  const CHOICES = [
    { label: '5 min', minutes: 5 },
    { label: '15 min', minutes: 15 },
    { label: '30 min', minutes: 30 },
    { label: '1 hour', minutes: 60 }
  ];

  // A Teams / Zoom / Meet link in the note gets a "Join" button: it opens the meeting and marks the reminder done.
  const meeting = $derived(preview ? null : meetingLink(reminder.note));

  function join() {
    if (meeting) api.openLink(meeting).then(() => onDone?.(), () => {});
  }

  // A long note shows its first lines; "Show more" opens the rest (scrollable past a few more lines).
  let noteEl = $state<HTMLParagraphElement>();
  let noteOpen = $state(false);
  let noteClipped = $state(false);

  // Is the collapsed note hiding anything? Re-measured when the text changes (an edit while it rings),
  // or when the width or the theme's font changes the line count.
  $effect(() => {
    void reminder.note;
    const el = noteEl;
    if (!el || preview) return;
    const measure = () => {
      if (!noteOpen) noteClipped = el.scrollHeight > el.clientHeight + 1;
    };
    const observer = new ResizeObserver(measure);
    observer.observe(el);
    measure();
    return () => observer.disconnect();
  });
</script>

<div
  class="card"
  class:preview
  role={preview ? undefined : 'alertdialog'}
  aria-labelledby={preview ? undefined : 'alert-title'}
  aria-hidden={preview ? 'true' : undefined}
  inert={preview}
>
  <div class="head">
    <span class="status"><span class="dot"></span>{status}</span>
    <span class="clock">{formatTime(reminder.dueAt)}</span>
    {#if reminder.priority === 'high'}<span class="high">High priority</span>{/if}
    {#if reminder.repeat !== 'none'}<span class="quiet repeat">{repeatLabel(reminder.repeat)}</span>{/if}
    <span class="spacer"></span>
    {#if count > 1}<span class="quiet">1 of {count}</span>{/if}
    <button class="close" onclick={onDismiss} aria-label="Dismiss" title="Dismiss">
      <Icon name="x" size={14} />
    </button>
  </div>

  <h2 class="alert-title" id={preview ? undefined : 'alert-title'}>{reminder.title}</h2>
  {#if reminder.note}
    <p class="note" class:open={noteOpen} bind:this={noteEl}><NoteText text={reminder.note} /></p>
    {#if noteClipped || noteOpen}
      <button class="note-toggle" onclick={() => (noteOpen = !noteOpen)} aria-expanded={noteOpen}>
        {noteOpen ? 'Show less' : 'Show more'}
      </button>
    {/if}
  {/if}

  <div class="foot">
    <div class="split">
      <button class="snooze" onclick={() => onSnooze?.(snoozeMinutes)} title="Snooze (Ctrl+Alt+S from anywhere)">
        Snooze {snoozeMinutes} min
      </button>
      <button
        class="more"
        onclick={() => (expanded = !expanded)}
        aria-label="More snooze options"
        aria-expanded={expanded}
      >
        <span class="chev" class:open={expanded}><Icon name="chevron" size={14} /></span>
      </button>
    </div>
    <div class="answer">
      {#if meeting}
        <button class="join" onclick={join} title="Open the meeting link and mark this done">Join</button>
      {/if}
      <button class="done" onclick={onDone} title="Done (Ctrl+Alt+D from anywhere)">Done</button>
    </div>
  </div>

  {#if expanded}
    <div class="chips" transition:slide={{ duration: 160 }}>
      <span class="chips-label">Snooze for</span>
      {#each CHOICES as choice (choice.minutes)}
        <button class="chip" onclick={() => onSnooze?.(choice.minutes)}>{choice.label}</button>
      {/each}
      <button class="chip" onclick={() => onSnoozeUntil?.(atHour(9, 1))}>Tomorrow 9:00</button>
    </div>
  {/if}
</div>

<style>
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
  .clock {
    font-variant-numeric: tabular-nums;
  }
  .high {
    color: var(--accent-text);
  }
  .quiet {
    color: var(--faint);
  }
  /* The header never wraps; if it runs out of room the repeat rule gives way, never the ✕. */
  .head > * {
    flex: none;
  }
  .head > .repeat {
    flex: 0 1 auto;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .head > .spacer {
    flex: 1;
  }
  .close {
    width: 28px;
    height: 28px;
    display: grid;
    place-items: center;
    border-radius: var(--btn-radius);
    color: inherit;
    opacity: 0.7;
    background: var(--close-bg);
    box-shadow: var(--close-shadow);
    transition: background 0.15s, opacity 0.15s;
  }
  .close:hover {
    background: var(--hover);
    opacity: 1;
  }

  .alert-title {
    padding: 6px var(--pad-x) 0;
    font-family: var(--font-display);
    font-size: var(--title-size);
    font-weight: var(--display-weight);
    line-height: var(--title-lh);
    letter-spacing: var(--display-tracking);
    text-transform: var(--display-case);
    text-wrap: balance;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
    overflow-wrap: anywhere;
  }
  .note {
    margin: 4px var(--pad-x) 0;
    font-size: 13.5px;
    line-height: 1.5;
    color: var(--dim);
    /* Keep the line breaks typed into the note (lists, addresses). */
    white-space: pre-line;
    text-wrap: pretty;
    display: -webkit-box;
    -webkit-line-clamp: 3;
    line-clamp: 3;
    -webkit-box-orient: vertical;
    overflow: hidden;
    overflow-wrap: anywhere;
    /* A note often holds a number or a reference worth copying. */
    user-select: text;
    cursor: text;
  }
  .note.open {
    display: block;
    -webkit-line-clamp: unset;
    line-clamp: none;
    max-height: calc(1.5em * 8);
    overflow-y: auto;
    overscroll-behavior: contain;
  }
  .note::-webkit-scrollbar {
    width: 8px;
  }
  .note::-webkit-scrollbar-thumb {
    border-width: 2px;
  }
  .note-toggle {
    margin: 3px var(--pad-x) 0;
    font-size: 12.5px;
    font-weight: 500;
    color: var(--accent-text);
    text-decoration: underline;
    text-decoration-color: color-mix(in srgb, currentColor 40%, transparent);
    text-underline-offset: 3px;
    transition: text-decoration-color 0.15s;
  }
  .note-toggle:hover {
    text-decoration-color: currentColor;
  }

  .foot {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    margin-top: 16px;
    padding: 10px calc(var(--pad-x) - 6px) 10px var(--pad-x);
    background: var(--foot-bg);
    border-top: var(--foot-border);
  }

  .split {
    display: flex;
    height: 32px;
    background: var(--btn-bg);
    border: var(--btn-border);
    border-radius: var(--btn-radius);
    box-shadow: var(--btn-shadow);
    color: var(--btn-color);
    overflow: hidden;
  }
  .split:active {
    box-shadow: var(--btn-shadow-active);
  }
  .split button {
    height: 100%;
    transition: background 0.15s;
  }
  .split button:hover {
    background: var(--hover);
  }
  .snooze {
    padding: 0 14px;
    font-size: 13px;
    font-weight: 500;
  }
  .more {
    width: 30px;
    display: grid;
    place-items: center;
    border-left: var(--split-divider);
  }
  .chev {
    display: grid;
    transition: transform 0.18s var(--ease);
  }
  .chev.open {
    transform: rotate(180deg);
  }

  .answer {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .join {
    height: 32px;
    padding: 0 16px;
    font-size: 13px;
    font-weight: 600;
    color: var(--btn-color);
    background: var(--btn-bg);
    border: var(--btn-border);
    border-radius: var(--btn-radius);
    box-shadow: var(--btn-shadow);
    transition: background 0.15s, transform 0.1s;
  }
  .join:hover {
    background: var(--hover);
  }
  .join:active {
    transform: var(--press);
    box-shadow: var(--btn-shadow-active);
  }

  .done {
    height: 32px;
    padding: 0 20px;
    border-radius: var(--btn-radius);
    font-size: 13px;
    font-weight: 600;
    color: var(--primary-color);
    background: var(--primary-bg);
    border: var(--primary-border);
    box-shadow: var(--primary-shadow);
    transition: opacity 0.15s, transform 0.1s, box-shadow 0.1s;
  }
  .done:hover {
    opacity: 0.88;
  }
  .done:active {
    transform: var(--press);
    box-shadow: var(--primary-shadow-active);
  }

  .chips {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
    padding: 10px calc(var(--pad-x) - 6px) 12px var(--pad-x);
    border-top: var(--foot-border);
  }
  .chips-label {
    margin-right: 4px;
    font-size: 12.5px;
    color: var(--faint);
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

  /* Previews are scaled-down, static copies. */
  .preview .close,
  .preview .chips {
    pointer-events: none;
  }
</style>
