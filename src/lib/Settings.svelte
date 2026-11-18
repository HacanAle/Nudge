<script lang="ts">
  import { onMount } from 'svelte';
  import { api, errorMessage } from './api';
  import { DEFAULT_HOTKEY, settings } from './prefs.svelte';
  import { THEMES } from './themes';
  import ThemePreview from './ThemePreview.svelte';

  // ── start with Windows ──
  let startupError = $state('');

  async function toggleStartup() {
    startupError = (await settings.setAutostart(!settings.autostart)) ?? '';
  }

  // ── quiet when presenting ──
  let quietError = $state('');

  async function toggleQuiet() {
    quietError = (await settings.setQuietWhenPresenting(!settings.quietWhenPresenting)) ?? '';
  }

  // ── ring again ──
  const RECHIME = [
    { minutes: 0, label: 'Off' },
    { minutes: 2, label: 'Every 2 min' },
    { minutes: 5, label: 'Every 5 min' },
    { minutes: 10, label: 'Every 10 min' },
    { minutes: 15, label: 'Every 15 min' }
  ];
  let rechimeError = $state('');

  async function changeRechime(e: Event) {
    rechimeError = (await settings.setRechime(Number((e.currentTarget as HTMLSelectElement).value))) ?? '';
  }

  // ── backup ──
  let backupMessage = $state('');
  let backupError = $state('');

  async function exportBackup() {
    backupMessage = backupError = '';
    try {
      backupMessage = `Saved ${await api.exportReminders()} to your Downloads folder.`;
    } catch (e) {
      backupError = errorMessage(e);
    }
  }

  async function importBackup(e: Event) {
    const input = e.currentTarget as HTMLInputElement;
    const file = input.files?.[0];
    input.value = ''; // picking the same file again still counts as a change
    if (!file) return;
    backupMessage = backupError = '';
    if (file.size > 10_000_000) {
      backupError = "That file is too big to be a Nudge backup.";
      return;
    }
    try {
      const { added, skipped } = await api.importReminders(await file.text());
      const plural = added === 1 ? '' : 's';
      const rest = skipped ? ` ${skipped} skipped (already here, or not valid).` : '';
      backupMessage = added ? `Added ${added} reminder${plural}.${rest}` : `Nothing new to add.${rest}`;
    } catch (e) {
      backupError = errorMessage(e);
    }
  }

  // ── test alert ──
  let testError = $state('');

  async function sendTestAlert() {
    testError = '';
    try {
      await api.sendTestAlert();
    } catch (e) {
      testError = errorMessage(e);
    }
  }

  // ── quick-add shortcut ──
  let recording = $state(false);
  let hotkeyError = $state('');
  let saving = false;

  const MODIFIER_CODES = /^(Control|Shift|Alt|Meta|OS)(Left|Right)?$/;
  const NAMED_KEYS: Record<string, string> = {
    Space: 'Space',
    ArrowUp: 'Up',
    ArrowDown: 'Down',
    ArrowLeft: 'Left',
    ArrowRight: 'Right'
  };

  /** The shortcut-string name for a key, or null if Nudge can't use it. */
  function keyName(code: string): string | null {
    if (/^Key[A-Z]$/.test(code)) return code.slice(3);
    if (/^Digit\d$/.test(code)) return code.slice(5);
    if (/^F([1-9]|1\d|2[0-4])$/.test(code)) return code;
    return NAMED_KEYS[code] ?? null;
  }

  async function onKey(e: KeyboardEvent) {
    if (!recording) return;
    e.preventDefault();
    e.stopPropagation();

    if (e.code === 'Escape') {
      recording = false;
      return;
    }
    if (MODIFIER_CODES.test(e.code)) return; // wait for the real key

    const key = keyName(e.code);
    const mods = [e.ctrlKey && 'Ctrl', e.altKey && 'Alt', e.shiftKey && 'Shift', e.metaKey && 'Super'].filter(Boolean);

    if (!key) {
      hotkeyError = 'Use a letter, number, F-key, Space or an arrow with it.';
      return;
    }
    if (!mods.some((m) => m === 'Ctrl' || m === 'Alt' || m === 'Super')) {
      hotkeyError = 'Hold Ctrl, Alt or Win as well, otherwise it would steal normal typing.';
      return;
    }
    if (saving) return;

    saving = true;
    hotkeyError = (await settings.setHotkey([...mods, key].join('+'))) ?? '';
    saving = false;
    if (!hotkeyError) recording = false;
  }

  function startRecording() {
    hotkeyError = '';
    recording = true;
  }

  async function resetHotkey() {
    hotkeyError = (await settings.setHotkey(DEFAULT_HOTKEY)) ?? '';
  }

  const keys = $derived(settings.hotkey.split('+'));

  onMount(() => {
    // Capture phase, so the recorder sees keys before anything else in the page.
    window.addEventListener('keydown', onKey, true);
    return () => window.removeEventListener('keydown', onKey, true);
  });
</script>

<div class="wrap">
  <header class="head">
    <h1 class="page-title">Settings</h1>
    <p class="summary">Make it work and look the way you like.</p>
  </header>

  <section>
    <h2 class="group-title">
      General
      <span class="rule"></span>
    </h2>

    <div class="rows">
      <div class="srow">
        <div class="text">
          <div class="s-title">Start with Windows</div>
          <div class="s-desc">Opens quietly in the tray when you sign in, so a reminder is never missed because Nudge wasn't running.</div>
          {#if startupError}<div class="s-err" role="alert">{startupError}</div>{/if}
        </div>
        <button
          class="switch"
          role="switch"
          aria-checked={settings.autostart}
          aria-label="Start with Windows"
          onclick={toggleStartup}
        >
          <span class="track"><span class="thumb"></span></span>
        </button>
      </div>

      <div class="srow">
        <div class="text">
          <div class="s-title">Quiet during presentations</div>
          <div class="s-desc">
            While a full-screen app, game or presentation is up, alerts wait and arrive when you're done (high
            priority still comes through). Alerts and quick add are also left out of screen sharing and recordings:
            you see them, the people watching don't.
          </div>
          {#if quietError}<div class="s-err" role="alert">{quietError}</div>{/if}
        </div>
        <button
          class="switch"
          role="switch"
          aria-checked={settings.quietWhenPresenting}
          aria-label="Quiet during presentations"
          onclick={toggleQuiet}
        >
          <span class="track"><span class="thumb"></span></span>
        </button>
      </div>

      <div class="srow">
        <div class="text">
          <div class="s-title">Ring again</div>
          <div class="s-desc">If an alert is left unanswered, chime again until you answer it.</div>
          {#if rechimeError}<div class="s-err" role="alert">{rechimeError}</div>{/if}
        </div>
        <select class="select" value={settings.rechimeMinutes} onchange={changeRechime} aria-label="Ring again">
          {#each RECHIME as r (r.minutes)}<option value={r.minutes}>{r.label}</option>{/each}
        </select>
      </div>

      <div class="srow">
        <div class="text">
          <div class="s-title">Quick add</div>
          <div class="s-desc">
            Press this anywhere to add a reminder without opening Nudge. Type it the way you'd say it:
            “call mom tomorrow 5pm”.
          </div>
          {#if hotkeyError}
            <div class="s-err" role="alert">{hotkeyError}</div>
          {:else if !settings.hotkeyActive}
            <div class="s-err" role="alert">
              Windows won't let Nudge use {settings.hotkey} right now. Another app is probably using it. Pick a different shortcut.
            </div>
          {/if}
        </div>

        <div class="keys">
          {#if recording}
            <span class="recording" aria-live="polite">Press the new shortcut… <span class="faint">Esc cancels</span></span>
          {:else}
            <span class="caps" class:dead={!settings.hotkeyActive} aria-label="Current shortcut: {settings.hotkey}">
              {#each keys as k (k)}<kbd>{k}</kbd>{/each}
            </span>
            <button class="text-btn" onclick={startRecording}>Change</button>
            {#if settings.hotkey !== DEFAULT_HOTKEY}
              <button class="text-btn" onclick={resetHotkey}>Reset</button>
            {/if}
          {/if}
        </div>
      </div>

      <div class="srow">
        <div class="text">
          <div class="s-title">Answer from the keyboard</div>
          <div class="s-desc">
            While an alert is on screen, these work from any app. The rest of the time they're left free for other apps.
          </div>
        </div>
        <div class="key-list">
          <span class="key-line"><span class="caps"><kbd>Ctrl</kbd><kbd>Alt</kbd><kbd>D</kbd></span> Done</span>
          <span class="key-line"><span class="caps"><kbd>Ctrl</kbd><kbd>Alt</kbd><kbd>S</kbd></span> Snooze 10 min</span>
        </div>
      </div>

      <div class="srow">
        <div class="text">
          <div class="s-title">Backup</div>
          <div class="s-desc">
            Save all your reminders to a file, or add them back from one (on a new PC, say). Importing never
            duplicates a reminder that's already here.
          </div>
          {#if backupError}
            <div class="s-err" role="alert">{backupError}</div>
          {:else if backupMessage}
            <div class="s-ok" role="status">{backupMessage}</div>
          {/if}
        </div>
        <div class="keys">
          <button class="text-btn" onclick={exportBackup}>Export</button>
          <label class="text-btn file-btn">
            Import
            <input type="file" accept=".json,application/json" onchange={importBackup} />
          </label>
        </div>
      </div>

      <div class="srow">
        <div class="text">
          <div class="s-title">Test alert</div>
          <div class="s-desc">Pops up a sample reminder so you can see and hear what an alert looks like with your theme.</div>
          {#if testError}<div class="s-err" role="alert">{testError}</div>{/if}
        </div>
        <button class="text-btn" onclick={sendTestAlert}>Send test alert</button>
      </div>
    </div>
  </section>

  <section class="themes">
    <h2 class="group-title">
      Theme
      <span class="rule"></span>
      <span class="count">{THEMES.length}</span>
    </h2>
    <p class="hint">Applies to this window, the alerts and quick add, straight away.</p>

    <div class="grid" role="radiogroup" aria-label="Theme">
      {#each THEMES as t (t.id)}
        {@const selected = settings.theme === t.id}
        <button
          class="tile"
          class:selected
          role="radio"
          aria-checked={selected}
          onclick={() => settings.setTheme(t.id)}
        >
          <span class="frame"><ThemePreview id={t.id} /></span>
          <span class="caption">
            <span class="name">{t.name}</span>
            <span class="kind">{t.style}</span>
            <span class="mark">{selected ? 'In use' : ''}</span>
          </span>
          <span class="blurb">{t.blurb}{t.followsSystem ? ' Follows Windows light or dark.' : ''}</span>
        </button>
      {/each}
    </div>

    {#if settings.error}<p class="err" role="alert">{settings.error}</p>{/if}
  </section>
</div>

<style>
  .wrap {
    max-width: 720px;
    margin: 0 auto;
    padding: 18px 32px 56px;
  }

  .head {
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

  .group-title {
    display: flex;
    align-items: center;
    gap: 12px;
    font-family: var(--font-display);
    font-size: 13px;
    font-weight: var(--display-weight);
    letter-spacing: var(--display-tracking);
    text-transform: var(--display-case);
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
  .hint {
    margin: 8px 0 18px;
    font-size: 13px;
    color: var(--dim);
  }
  .themes {
    margin-top: 40px;
  }

  /* ── general rows ── */
  .rows {
    margin-top: 6px;
    padding: var(--list-pad);
    background: var(--list-bg);
    border: var(--list-border);
    border-radius: var(--radius-lg);
    box-shadow: var(--list-shadow);
    backdrop-filter: var(--panel-filter);
  }
  .srow {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 28px;
    padding: 16px 0;
    border-bottom: var(--row-border);
  }
  .srow:last-child {
    border-bottom: var(--row-last-border);
  }
  .text {
    min-width: 0;
  }
  .s-title {
    font-size: 15px;
    font-weight: 550;
  }
  .s-desc {
    margin-top: 3px;
    max-width: 430px;
    font-size: 13px;
    line-height: 1.5;
    color: var(--dim);
    text-wrap: pretty;
  }
  .s-err {
    margin-top: 8px;
    font-size: 13px;
    color: var(--danger);
  }
  .s-ok {
    margin-top: 8px;
    font-size: 13px;
    color: var(--accent-text);
  }

  .select {
    flex: none;
    height: 32px;
    padding: 0 8px;
    font-size: 13px;
    background: var(--input-bg);
    border: var(--input-border);
    border-radius: var(--radius);
    box-shadow: var(--input-shadow);
    outline: none;
    cursor: pointer;
  }
  .select:focus {
    border-color: var(--text);
  }

  .key-list {
    flex: none;
    display: flex;
    flex-direction: column;
    gap: 8px;
    font-size: 13px;
    color: var(--dim);
  }
  .key-line {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .file-btn {
    cursor: pointer;
  }
  .file-btn input {
    position: absolute;
    width: 1px;
    height: 1px;
    opacity: 0;
    pointer-events: none;
  }
  .file-btn:focus-within {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }

  .switch {
    flex: none;
    display: inline-flex;
    align-items: center;
    padding: 6px 0 6px 6px;
  }
  .track {
    position: relative;
    width: 38px;
    height: 20px;
    border: 1px solid var(--border-strong);
    border-radius: calc(var(--radius) + 8px);
    transition: background 0.18s var(--ease), border-color 0.18s;
  }
  .thumb {
    position: absolute;
    top: 3px;
    left: 3px;
    width: 12px;
    height: 12px;
    border-radius: 50%;
    background: var(--dim);
    transition: transform 0.18s var(--ease), background 0.18s;
  }
  .switch[aria-checked='true'] .track {
    background: var(--ink);
    border-color: var(--ink);
  }
  .switch[aria-checked='true'] .thumb {
    transform: translateX(18px);
    background: var(--on-ink);
  }

  .keys {
    flex: none;
    display: flex;
    align-items: center;
    gap: 14px;
  }
  .caps {
    display: inline-flex;
    gap: 5px;
  }
  .caps.dead kbd {
    opacity: 0.55;
    text-decoration: line-through;
  }
  kbd {
    min-width: 28px;
    padding: 4px 8px;
    text-align: center;
    font-family: var(--mono);
    font-size: 12.5px;
    color: var(--text);
    background: var(--input-bg);
    border: var(--input-border);
    border-bottom-width: 2px;
    border-radius: var(--radius);
    box-shadow: var(--input-shadow);
  }
  .recording {
    font-size: 13px;
    color: var(--accent-text);
  }
  .faint {
    color: var(--faint);
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

  /* ── theme grid ── */
  .grid {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 26px 20px;
  }

  .tile {
    display: flex;
    flex-direction: column;
    gap: 8px;
    text-align: left;
    border-radius: var(--radius);
    outline: 2px solid transparent;
    outline-offset: 5px;
    transition: outline-color 0.15s;
  }
  .tile:hover {
    outline-color: var(--border-strong);
  }
  .tile.selected {
    outline-color: var(--tile-selected);
  }

  .frame {
    display: block;
    overflow: hidden;
    border: 1px solid var(--border-strong);
    border-radius: var(--radius);
  }

  .caption {
    display: flex;
    align-items: baseline;
    gap: 10px;
    padding-top: 2px;
  }
  .name {
    font-family: var(--font-display);
    font-size: 15px;
    font-weight: var(--display-weight);
    letter-spacing: var(--display-tracking);
    text-transform: var(--display-case);
  }
  .kind {
    font-family: var(--mono);
    font-size: 12px;
    color: var(--dim);
  }
  .mark {
    margin-left: auto;
    font-family: var(--mono);
    font-size: 12px;
    color: var(--accent-text);
  }
  .blurb {
    font-size: 12.5px;
    line-height: 1.45;
    color: var(--dim);
    text-wrap: pretty;
  }

  .err {
    margin-top: 16px;
    font-size: 13px;
    color: var(--danger);
  }
</style>
