<script lang="ts">
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import Icon from './Icon.svelte';
  import Reminders from './Reminders.svelte';
  import Settings from './Settings.svelte';

  type Tab = 'reminders' | 'settings';

  const win = getCurrentWindow();
  const TABS: { id: Tab; label: string }[] = [
    { id: 'reminders', label: 'Reminders' },
    { id: 'settings', label: 'Settings' }
  ];

  let tab = $state<Tab>('reminders');
</script>

<div class="shell">
  <header class="titlebar" data-tauri-drag-region>
    <span class="wordmark">Nudge</span>
    <div class="window-buttons">
      <button class="wbtn" onclick={() => win.minimize()} aria-label="Minimize"><Icon name="minus" size={14} /></button>
      <button
        class="wbtn close"
        onclick={() => win.hide()}
        aria-label="Close to tray"
        title="Hide to tray. Reminders keep firing."
      >
        <Icon name="x" size={14} />
      </button>
    </div>
  </header>

  <div class="tabs" role="tablist" aria-label="Sections">
    <div class="tabs-inner">
      {#each TABS as t (t.id)}
        <button
          class="tab"
          role="tab"
          id="tab-{t.id}"
          aria-selected={tab === t.id}
          aria-controls="panel-{t.id}"
          onclick={() => (tab = t.id)}
        >
          {t.label}
        </button>
      {/each}
    </div>
  </div>

  <!-- Reminders stays mounted so a half-typed reminder survives a visit to Settings. -->
  <div class="content" id="panel-reminders" role="tabpanel" aria-labelledby="tab-reminders" hidden={tab !== 'reminders'}>
    <Reminders />
  </div>
  {#if tab === 'settings'}
    <div class="content" id="panel-settings" role="tabpanel" aria-labelledby="tab-settings">
      <Settings />
    </div>
  {/if}
</div>

<style>
  .shell {
    height: 100%;
    display: flex;
    flex-direction: column;
    background: var(--app-bg);
    border: 1px solid var(--border);
  }

  /* ── title bar ── */
  .titlebar {
    height: 36px;
    flex: none;
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding-left: 16px;
    background: var(--titlebar-bg);
    color: var(--titlebar-color);
  }
  .wordmark {
    font-family: var(--font-display);
    font-size: 12.5px;
    font-weight: var(--display-weight);
    letter-spacing: 0.01em;
    text-transform: var(--display-case);
    pointer-events: none;
  }
  .window-buttons {
    display: flex;
    align-items: center;
    height: 100%;
  }
  .wbtn {
    width: 46px;
    height: 100%;
    display: grid;
    place-items: center;
    color: inherit;
    transition: background 0.12s, color 0.12s;
  }
  .wbtn:hover {
    background: var(--hover);
    color: var(--text);
  }
  .wbtn.close:hover {
    background: #c42b1c;
    color: #fff;
  }

  /* ── tabs ── */
  .tabs {
    flex: none;
    border-bottom: 1px solid var(--border);
  }
  .tabs-inner {
    max-width: 720px;
    margin: 0 auto;
    padding: 0 32px;
    display: flex;
    gap: 26px;
  }
  .tab {
    padding: 10px 0 9px;
    font-family: var(--font-display);
    font-size: 14px;
    font-weight: var(--display-weight);
    letter-spacing: var(--display-tracking);
    text-transform: var(--display-case);
    color: var(--dim);
    border-bottom: 2px solid transparent;
    margin-bottom: -1px;
    transition: color 0.15s, border-color 0.15s;
  }
  .tab:hover {
    color: var(--text);
  }
  .tab[aria-selected='true'] {
    color: var(--text);
    border-bottom-color: var(--accent);
  }

  .content {
    flex: 1;
    overflow-y: auto;
  }
  .content[hidden] {
    display: none;
  }
</style>
