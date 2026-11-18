<script lang="ts">
  import { onMount } from 'svelte';
  import { fade, slide } from 'svelte/transition';
  import { flip } from 'svelte/animate';
  import { api, errorMessage, on, type Priority, type Reminder } from './api';
  import {
    REPEAT_OPTIONS,
    atHour,
    formatDay,
    formatHeading,
    formatRelative,
    formatTime,
    formatWhen,
    fromLocalInput,
    inMinutes,
    repeatLabel,
    startOfDay,
    toLocalInput
  } from './format';
  import Icon from './Icon.svelte';
  import NoteText from './NoteText.svelte';
  import { settings } from './prefs.svelte';

  const DAY_GUESS = 36 * 3_600_000; // lands on the next calendar day from local midnight, DST-proof

  const PRIORITIES: { value: Priority; label: string }[] = [
    { value: 'low', label: 'Low' },
    { value: 'normal', label: 'Normal' },
    { value: 'high', label: 'High' }
  ];

  const QUICK_TIMES = [
    { id: '10m', label: '10 min', at: () => inMinutes(10) },
    { id: '1h', label: '1 hour', at: () => inMinutes(60) },
    {
      id: 'eve',
      label: 'This evening',
      at: () => (atHour(18, 0) > Date.now() + 60_000 ? atHour(18, 0) : atHour(18, 1))
    },
    { id: 'tom', label: 'Tomorrow 9:00', at: () => atHour(9, 1) }
  ];

  let upcoming = $state<Reminder[]>([]);
  let history = $state<Reminder[]>([]);
  let dnd = $state(false);
  let now = $state(Date.now());
  let showHistory = $state(false);

  // composer
  let title = $state('');
  let note = $state('');
  let showNote = $state(false);
  let priority = $state<Priority>('normal');
  let repeat = $state('none');
  let when = $state(toLocalInput(inMinutes(30)));
  let quick = $state('');
  let error = $state('');
  let flash = $state('');
  let saving = false;
  let titleInput = $state<HTMLInputElement>();
  let composer = $state<HTMLFormElement>();
  let flashTimer: ReturnType<typeof setTimeout> | undefined;

  /** The reminder loaded into the composer for editing, or null when the composer is adding a new one. */
  let editingId = $state<number | null>(null);

  // A reminder made by quick-add can repeat on a rule the dropdown has no preset for ("every 3 hours").
  // Keep that rule as a choice so editing doesn't show a blank dropdown or silently change it.
  const repeatOptions = $derived(
    REPEAT_OPTIONS.some((o) => o.value === repeat)
      ? REPEAT_OPTIONS
      : [...REPEAT_OPTIONS, { value: repeat, label: repeatLabel(repeat) }]
  );

  const groups = $derived.by(() => {
    const today = startOfDay(now);
    const tomorrow = startOfDay(today + DAY_GUESS);
    const dayAfter = startOfDay(tomorrow + DAY_GUESS);
    const buckets: { key: string; label: string; items: Reminder[] }[] = [
      { key: 'now', label: 'Needs attention', items: [] },
      { key: 'today', label: 'Today', items: [] },
      { key: 'tomorrow', label: 'Tomorrow', items: [] },
      { key: 'later', label: 'Later', items: [] }
    ];
    for (const r of upcoming) {
      if (r.status === 'active' || r.dueAt <= now) buckets[0].items.push(r);
      else if (r.dueAt < tomorrow) buckets[1].items.push(r);
      else if (r.dueAt < dayAfter) buckets[2].items.push(r);
      else buckets[3].items.push(r);
    }
    return buckets.filter((b) => b.items.length > 0);
  });

  const summary = $derived.by(() => {
    if (upcoming.length === 0) return 'Nothing scheduled';
    const next = upcoming.find((r) => r.status === 'pending' && r.dueAt > now);
    const count = `${upcoming.length} scheduled`;
    return next ? `${count} · next ${formatRelative(next.dueAt, now)}` : count;
  });

  async function refresh() {
    try {
      [upcoming, history] = await Promise.all([api.listUpcoming(), api.listHistory(30)]);
    } catch (e) {
      error = errorMessage(e);
      return;
    }
    // It was answered from the popup (or deleted) while its edit form was open: nothing left to save to.
    if (editingId !== null && !upcoming.some((r) => r.id === editingId)) {
      cancelEdit();
      say('That reminder was just completed or removed.');
    }
  }

  /** Back to an empty "new reminder" form. */
  function resetComposer() {
    title = '';
    note = '';
    showNote = false;
    priority = 'normal';
    repeat = 'none';
    when = toLocalInput(inMinutes(30));
    quick = '';
  }

  function startEdit(r: Reminder) {
    editingId = r.id;
    title = r.title;
    note = r.note;
    showNote = r.note !== '';
    priority = r.priority;
    repeat = r.repeat;
    when = toLocalInput(r.dueAt);
    quick = '';
    error = '';
    flash = '';
    composer?.scrollIntoView({ behavior: 'smooth', block: 'nearest' });
    titleInput?.focus();
  }

  function cancelEdit() {
    editingId = null;
    error = '';
    resetComposer();
  }

  function pickQuick(id: string, at: number) {
    quick = id;
    when = toLocalInput(at);
  }

  function say(message: string) {
    flash = message;
    clearTimeout(flashTimer);
    flashTimer = setTimeout(() => (flash = ''), 3200);
  }

  async function submit(e: Event) {
    e.preventDefault();
    if (saving) return;
    error = '';

    if (!title.trim()) {
      error = 'Give the reminder a title.';
      titleInput?.focus();
      return;
    }
    const dueAt = fromLocalInput(when);
    if (!Number.isFinite(dueAt)) {
      error = 'Pick a valid date and time.';
      return;
    }

    saving = true;
    try {
      if (editingId !== null) {
        await api.update(editingId, { title, note, priority, dueAt, repeat });
        say(dueAt <= Date.now() ? 'Saved. It will pop up right away.' : `Saved for ${formatWhen(dueAt)}.`);
        editingId = null;
        resetComposer();
      } else {
        await api.add({ title, note, priority, dueAt, repeat });
        say(dueAt <= Date.now() ? 'Added. It will pop up right away.' : `Added for ${formatWhen(dueAt)}.`);
        title = '';
        note = '';
        showNote = false;
        // A time left over from earlier may now be in the past; offer a fresh suggestion.
        if (fromLocalInput(when) <= Date.now()) {
          when = toLocalInput(inMinutes(30));
          quick = '';
        }
      }
      titleInput?.focus();
    } catch (err) {
      error = errorMessage(err);
    } finally {
      saving = false;
    }
  }

  function remove(id: number) {
    if (id === editingId) cancelEdit(); // deleting what you're editing closes the form quietly
    return api.remove(id).catch((e) => (error = errorMessage(e)));
  }
  const toggleDnd = () => api.setDnd(!dnd);

  /** Esc backs out of an edit without saving. */
  function onComposerKey(e: KeyboardEvent) {
    if (e.key === 'Escape' && editingId !== null) {
      e.preventDefault();
      cancelEdit();
    }
  }

  onMount(() => {
    refresh();
    api.getDnd().then((v) => (dnd = v));
    titleInput?.focus();

    const offs = [on('reminders-changed', refresh), on<boolean>('dnd-changed', (v) => (dnd = v))];
    const clock = setInterval(() => (now = Date.now()), 20_000);
    return () => {
      offs.forEach((off) => off());
      clearInterval(clock);
    };
  });
</script>

<div class="wrap">
  <header class="head">
    <div>
      <h1 class="page-title">{formatHeading(now)}</h1>
      <p class="summary">{summary}</p>
    </div>
    <div class="head-actions">
      <button class="switch" role="switch" aria-checked={dnd} onclick={toggleDnd} title="Hold alerts until you turn this off">
        <span class="track"><span class="thumb"></span></span>
        Do not disturb
      </button>
    </div>
  </header>

  <!-- Esc is picked up as it bubbles from the form's own inputs; the form itself isn't a control. -->
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <form class="composer" bind:this={composer} onsubmit={submit} onkeydown={onComposerKey}>
    {#if editingId !== null}
      <div class="edit-banner" transition:slide={{ duration: 150 }}>
        Editing reminder
        <span class="hint">Esc to cancel</span>
      </div>
    {/if}

    <input
      class="title-input"
      placeholder="Remind me to…"
      bind:value={title}
      bind:this={titleInput}
      maxlength="200"
      oninput={() => (error = '')}
      aria-label="Reminder title"
    />

    {#if showNote}
      <textarea
        class="note-input"
        placeholder="Note (optional)"
        bind:value={note}
        rows="2"
        maxlength="2000"
        transition:slide={{ duration: 150 }}
      ></textarea>
    {/if}

    <div class="fields">
      <label class="field">
        <span class="label">When</span>
        <input type="datetime-local" bind:value={when} oninput={() => (quick = '')} required />
      </label>

      <label class="field">
        <span class="label">Repeat</span>
        <select bind:value={repeat}>
          {#each repeatOptions as o (o.value)}
            <option value={o.value}>{o.label}</option>
          {/each}
        </select>
      </label>

      <div class="field">
        <span class="label">Priority</span>
        <div class="seg" role="radiogroup" aria-label="Priority">
          {#each PRIORITIES as p (p.value)}
            <button
              type="button"
              role="radio"
              aria-checked={priority === p.value}
              class:selected={priority === p.value}
              onclick={() => (priority = p.value)}
            >
              {p.label}
            </button>
          {/each}
        </div>
      </div>
    </div>

    <div class="quick">
      <span class="label">Quick</span>
      {#each QUICK_TIMES as q (q.id)}
        <button type="button" class:active={quick === q.id} onclick={() => pickQuick(q.id, q.at())}>
          {q.label}
        </button>
      {/each}
    </div>

    <div class="composer-foot">
      <button type="button" class="text-btn" onclick={() => (showNote = !showNote)}>
        {showNote ? 'Remove note' : 'Add a note'}
      </button>

      <span class="msg" class:err={!!error} role="status" aria-live="polite">
        {#if error}{error}{:else if flash}<span in:fade={{ duration: 150 }}>{flash}</span>{/if}
      </span>

      {#if editingId !== null}
        <button type="button" class="text-btn" onclick={cancelEdit}>Cancel</button>
      {/if}
      <button class="primary" type="submit">{editingId !== null ? 'Save changes' : 'Add reminder'}</button>
    </div>
  </form>

  {#if groups.length === 0}
    <section class="empty" in:fade={{ duration: 200 }}>
      <p class="empty-title">Nothing scheduled.</p>
      <p>Add a reminder above. When it's due, an alert drops in at the top of your screen.</p>
      <p class="tip">
        Or press
        <span class="caps">{#each settings.hotkey.split('+') as k (k)}<kbd>{k}</kbd>{/each}</span>
        anywhere to add one without opening this window.
      </p>
    </section>
  {/if}

  {#each groups as group (group.key)}
    <section class="group">
      <h2 class="group-title" class:attention={group.key === 'now'}>
        {group.label}
        <span class="rule"></span>
        <span class="count">{group.items.length}</span>
      </h2>

      <ul class="list">
        {#each group.items as r (r.id)}
          <li class="row" class:ringing={r.status === 'active'} animate:flip={{ duration: 200 }}>
            <time class="time" datetime={new Date(r.dueAt).toISOString()}>{formatTime(r.dueAt)}</time>

            <div class="main">
              <div class="r-title">{r.title}</div>
              <div class="r-meta">
                {#if group.key === 'now' || group.key === 'later'}<span>{formatDay(r.dueAt, now)}</span>{/if}
                <span>{formatRelative(r.dueAt, now)}</span>
                {#if r.repeat !== 'none'}<span>{repeatLabel(r.repeat)}</span>{/if}
                {#if r.priority === 'high'}<span class="hi">High priority</span>{/if}
                {#if r.priority === 'low'}<span>Low priority</span>{/if}
              </div>
              {#if r.note}<div class="r-note"><NoteText text={r.note} /></div>{/if}
            </div>

            <div class="actions">
              <button class="act" onclick={() => startEdit(r)} aria-label="Edit {r.title}">Edit</button>
              <button class="act del" onclick={() => remove(r.id)} aria-label="Delete {r.title}">Delete</button>
            </div>
          </li>
        {/each}
      </ul>
    </section>
  {/each}

  {#if history.length > 0}
    <section class="group">
      <h2 class="group-title">
        <button class="disclosure" onclick={() => (showHistory = !showHistory)} aria-expanded={showHistory}>
          Completed
          <span class="chev" class:open={showHistory}><Icon name="chevron" size={13} /></span>
        </button>
        <span class="rule"></span>
        {#if showHistory}
          <button class="text-btn small" onclick={() => api.clearHistory()} transition:fade={{ duration: 120 }}>
            Clear
          </button>
        {/if}
        <span class="count">{history.length}</span>
      </h2>

      {#if showHistory}
        <ul class="list" transition:slide={{ duration: 180 }}>
          {#each history as r (r.id)}
            <li class="row past">
              <time class="time">{r.completedAt ? formatTime(r.completedAt) : ''}</time>
              <div class="main">
                <div class="r-title">{r.title}</div>
                <div class="r-meta">
                  <span>{r.status === 'dismissed' ? 'Dismissed' : 'Done'}</span>
                  {#if r.completedAt}<span>{formatRelative(r.completedAt, now)}</span>{/if}
                </div>
              </div>
            </li>
          {/each}
        </ul>
      {/if}
    </section>
  {/if}
</div>

<style>
  .wrap {
    max-width: 720px;
    margin: 0 auto;
    padding: 18px 32px 56px;
  }

  .head {
    display: flex;
    align-items: flex-end;
    justify-content: space-between;
    gap: 24px;
    margin-bottom: 28px;
  }
  h1 {
    font-family: var(--font-display);
    font-size: var(--h1-size);
    font-weight: var(--display-weight);
    letter-spacing: var(--display-tracking);
    text-transform: var(--display-case);
    line-height: 1.1;
  }
  .summary {
    margin-top: 6px;
    font-family: var(--mono);
    font-size: 12.5px;
    color: var(--dim);
  }
  .head-actions {
    display: flex;
    align-items: center;
    gap: 20px;
    flex: none;
    padding-bottom: 2px;
  }

  .text-btn {
    font-size: 13px;
    font-weight: 500;
    color: var(--dim);
    text-decoration: underline;
    text-decoration-color: var(--border-strong);
    text-underline-offset: 3px;
    transition: color 0.15s, text-decoration-color 0.15s;
  }
  .text-btn:hover {
    color: var(--text);
    text-decoration-color: var(--text);
  }
  .text-btn.small {
    font-size: 12.5px;
  }

  .switch {
    display: inline-flex;
    align-items: center;
    gap: 9px;
    font-size: 13px;
    font-weight: 500;
    color: var(--dim);
    transition: color 0.15s;
  }
  .switch:hover,
  .switch[aria-checked='true'] {
    color: var(--text);
  }
  .track {
    position: relative;
    width: 30px;
    height: 16px;
    border: 1px solid var(--border-strong);
    border-radius: calc(var(--radius) + 6px);
    transition: background 0.18s var(--ease), border-color 0.18s;
  }
  .thumb {
    position: absolute;
    top: 2px;
    left: 2px;
    width: 10px;
    height: 10px;
    border-radius: 50%;
    background: var(--dim);
    transition: transform 0.18s var(--ease), background 0.18s;
  }
  .switch[aria-checked='true'] .track {
    background: var(--ink);
    border-color: var(--ink);
  }
  .switch[aria-checked='true'] .thumb {
    transform: translateX(14px);
    background: var(--on-ink);
  }

  /* ── composer ── */
  .composer {
    padding: 6px 18px 14px;
    background: var(--panel-bg);
    border: var(--panel-border);
    border-radius: var(--radius-lg);
    box-shadow: var(--panel-shadow);
    backdrop-filter: var(--panel-filter);
  }
  .title-input {
    width: 100%;
    padding: 14px 0;
    font-size: 19px;
    font-weight: 500;
    letter-spacing: -0.012em;
    background: none;
    border: 0;
    outline: none;
  }
  .title-input::placeholder {
    color: var(--faint);
  }
  .note-input {
    width: 100%;
    margin-bottom: 12px;
    padding: 9px 11px;
    resize: none;
    /* Grows with what you type, from two lines up to about eight, then scrolls. */
    field-sizing: content;
    min-height: calc(3em + 20px);
    max-height: calc(12em + 20px);
    font-size: 13.5px;
    line-height: 1.5;
    background: var(--input-bg);
    border: var(--input-border);
    border-radius: var(--radius);
    box-shadow: var(--input-shadow);
    outline: none;
  }
  .note-input:focus {
    border-color: var(--text);
  }

  .fields {
    display: grid;
    grid-template-columns: 1.3fr 1fr 1.3fr;
    gap: 14px;
    padding-top: 14px;
    border-top: var(--row-border);
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 6px;
    min-width: 0;
  }
  .label {
    font-size: 12px;
    color: var(--faint);
  }
  .field input,
  .field select {
    height: 36px;
    padding: 0 10px;
    background: var(--input-bg);
    border: var(--input-border);
    border-radius: var(--radius);
    box-shadow: var(--input-shadow);
    outline: none;
    font-size: 13.5px;
    transition: border-color 0.15s;
  }
  .field input[type='datetime-local'] {
    font-family: var(--mono);
    font-size: 13px;
    font-variant-numeric: tabular-nums;
  }
  .field input:focus,
  .field select:focus {
    border-color: var(--text);
  }
  .field select {
    cursor: pointer;
  }

  .seg {
    display: flex;
    gap: 0;
    height: 36px;
    padding: var(--seg-pad);
    background: var(--seg-bg);
    border: var(--seg-border);
    border-radius: var(--seg-radius);
    overflow: hidden;
  }
  .seg button {
    flex: 1;
    font-size: 13px;
    font-weight: 500;
    color: var(--dim);
    border-radius: max(0px, calc(var(--seg-radius) - var(--seg-pad)));
    box-shadow: var(--seg-shadow);
    transition: background 0.15s, color 0.15s;
  }
  .seg button + button {
    border-left: var(--seg-divider);
  }
  .seg button:hover {
    color: var(--text);
    background: var(--hover);
  }
  .seg button.selected {
    background: var(--seg-on-bg);
    color: var(--seg-on-color);
    box-shadow: var(--seg-on-shadow);
    border-left-color: transparent;
  }

  .quick {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 4px 16px;
    margin-top: 14px;
  }
  .quick button {
    font-size: 13px;
    color: var(--dim);
    padding: 2px 0;
    border-bottom: 2px solid transparent;
    transition: color 0.15s, border-color 0.15s;
  }
  .quick button:hover {
    color: var(--text);
  }
  .quick button.active {
    color: var(--text);
    border-bottom-color: var(--accent);
  }

  .composer-foot {
    display: flex;
    align-items: center;
    gap: 16px;
    margin-top: 18px;
  }
  .msg {
    flex: 1;
    min-width: 0;
    font-size: 13px;
    color: var(--dim);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .msg.err {
    color: var(--danger);
  }
  .primary {
    height: 36px;
    padding: 0 18px;
    border-radius: var(--btn-radius);
    font-size: 13.5px;
    font-weight: 600;
    color: var(--primary-color);
    background: var(--primary-bg);
    border: var(--primary-border);
    box-shadow: var(--primary-shadow);
    transition: opacity 0.15s, transform 0.1s, box-shadow 0.1s;
  }
  .primary:hover {
    opacity: 0.88;
  }
  .primary:active {
    transform: var(--press);
    box-shadow: var(--primary-shadow-active);
  }

  /* ── agenda ── */
  .group {
    margin-top: 34px;
  }
  .group-title {
    display: flex;
    align-items: center;
    gap: 12px;
    font-family: var(--font-display);
    font-size: 13px;
    font-weight: var(--display-weight);
    letter-spacing: var(--display-tracking);
    text-transform: var(--display-case);
    color: var(--text);
  }
  .group-title.attention {
    color: var(--accent-text);
  }
  .rule {
    flex: 1;
    height: 1px;
    background: var(--border);
  }
  .count {
    font-family: var(--mono);
    font-size: 12px;
    font-weight: 400;
    letter-spacing: 0;
    color: var(--faint);
  }
  .disclosure {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font: inherit;
    letter-spacing: inherit;
    text-transform: inherit;
    color: var(--dim);
  }
  .disclosure:hover {
    color: var(--text);
  }
  .chev {
    display: grid;
    transition: transform 0.18s var(--ease);
  }
  .chev.open {
    transform: rotate(180deg);
  }

  .list {
    list-style: none;
    margin-top: 8px;
    padding: var(--list-pad);
    background: var(--list-bg);
    border: var(--list-border);
    border-radius: var(--radius-lg);
    box-shadow: var(--list-shadow);
    backdrop-filter: var(--panel-filter);
  }
  .row {
    display: grid;
    grid-template-columns: 58px 1fr auto;
    column-gap: 20px;
    align-items: start;
    padding: var(--row-pad);
    margin: 0 var(--row-bleed);
    border-bottom: var(--row-border);
  }
  .row:last-child {
    border-bottom: var(--row-last-border);
  }
  .time {
    padding-top: 2px;
    font-family: var(--mono);
    font-size: 13px;
    font-variant-numeric: tabular-nums;
    color: var(--text);
    transition: color 0.12s;
  }
  /* Hovering a reminder lights up its time and title in the accent colour. Nothing moves or gets a box. */
  .row:not(.past):hover .time,
  .row:not(.past):hover .r-title {
    color: var(--accent-text);
  }
  .ringing .time {
    color: var(--accent-text);
    font-weight: 500;
  }
  .r-title {
    font-size: 15px;
    font-weight: 550;
    line-height: 1.35;
    overflow-wrap: anywhere;
    transition: color 0.12s;
  }
  .r-meta {
    display: flex;
    flex-wrap: wrap;
    gap: 0 8px;
    margin-top: 2px;
    font-size: 12.5px;
    color: var(--dim);
  }
  .r-meta span + span::before {
    content: '·';
    margin-right: 8px;
    color: var(--faint);
  }
  .r-meta .hi {
    color: var(--accent-text);
  }
  .r-note {
    margin-top: 4px;
    font-size: 13px;
    color: var(--dim);
    white-space: pre-line;
    text-wrap: pretty;
    overflow-wrap: anywhere;
    /* The full note is one click away (Edit); the list stays scannable. */
    display: -webkit-box;
    -webkit-line-clamp: 3;
    line-clamp: 3;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .actions {
    display: flex;
    gap: 10px;
  }
  .act {
    padding: 2px 4px;
    font-size: 12.5px;
    color: var(--faint);
    opacity: 0;
    transition: opacity 0.12s, color 0.12s;
  }
  .row:hover .act,
  .act:focus-visible {
    opacity: 1;
  }
  .act:hover {
    color: var(--text);
  }
  .act.del:hover {
    color: var(--danger);
  }

  .edit-banner {
    display: flex;
    align-items: baseline;
    gap: 12px;
    padding: 10px 0 0;
    font-size: 12.5px;
    font-weight: 600;
    color: var(--accent-text);
  }
  .edit-banner .hint {
    font-weight: 400;
    color: var(--faint);
  }
  .past .r-title {
    font-weight: 450;
    color: var(--dim);
  }
  .past .time {
    color: var(--faint);
  }

  /* ── empty ── */
  .empty {
    margin-top: 34px;
    padding-top: 18px;
    border-top: var(--row-border);
    max-width: 380px;
    font-size: 13.5px;
    color: var(--dim);
    text-wrap: pretty;
  }
  .empty-title {
    margin-bottom: 4px;
    font-size: 15px;
    font-weight: 550;
    color: var(--text);
  }

  .tip {
    margin-top: 10px;
    color: var(--faint);
  }
  .caps {
    display: inline-flex;
    gap: 4px;
    margin: 0 2px;
    vertical-align: baseline;
  }
  kbd {
    padding: 1px 6px;
    font-family: var(--mono);
    font-size: 12px;
    color: var(--dim);
    background: var(--input-bg);
    border: var(--input-border);
    border-bottom-width: 2px;
    border-radius: var(--radius);
    box-shadow: var(--input-shadow);
  }

  @media (max-width: 820px) {
    .fields {
      grid-template-columns: 1fr 1fr;
    }
    .head {
      flex-direction: column;
      align-items: flex-start;
      gap: 14px;
    }
  }
</style>
