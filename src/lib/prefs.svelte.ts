import { api, errorMessage, on, type Settings } from './api';
import { DEFAULT_THEME, isTheme } from './themes';

const CACHE_KEY = 'nudge.theme';

/** Keep in step with `DEFAULT_HOTKEY` in src-tauri/src/settings.rs. */
export const DEFAULT_HOTKEY = 'Ctrl+Alt+N';

/** The theme the last session used — applied before anything paints, so there is no flash of the wrong look. */
export function cachedTheme(): string {
  try {
    const cached = localStorage.getItem(CACHE_KEY);
    return isTheme(cached) ? cached : DEFAULT_THEME;
  } catch {
    return DEFAULT_THEME;
  }
}

function apply(theme: string) {
  document.documentElement.dataset.theme = theme;
  try {
    localStorage.setItem(CACHE_KEY, theme);
  } catch {
    /* storage can be unavailable; the backend is the source of truth anyway */
  }
}

let theme = $state(cachedTheme());
let hotkey = $state(DEFAULT_HOTKEY);
let hotkeyActive = $state(true);
let autostart = $state(false);
let quietWhenPresenting = $state(true);
let rechimeMinutes = $state(0);
let error = $state('');

function adopt(next: Settings) {
  if (isTheme(next.theme)) {
    theme = next.theme;
    apply(theme);
  }
  hotkey = next.hotkey;
  hotkeyActive = next.hotkeyActive;
  quietWhenPresenting = next.quietWhenPresenting;
  rechimeMinutes = next.rechimeMinutes;
}

/** Reactive app settings, shared by every window through the backend. */
export const settings = {
  get theme() {
    return theme;
  },
  get hotkey() {
    return hotkey;
  },
  /** False when Windows refused the quick-add shortcut. */
  get hotkeyActive() {
    return hotkeyActive;
  },
  get autostart() {
    return autostart;
  },
  get quietWhenPresenting() {
    return quietWhenPresenting;
  },
  get rechimeMinutes() {
    return rechimeMinutes;
  },
  get error() {
    return error;
  },

  /** Load the saved settings and follow changes made from another window. */
  async init(): Promise<() => void> {
    apply(theme);
    try {
      adopt(await api.getSettings());
      autostart = await api.getAutostart();
    } catch (e) {
      error = errorMessage(e);
    }
    const offs = [on<Settings>('settings-changed', adopt), on<boolean>('autostart-changed', (v) => (autostart = v))];
    return () => offs.forEach((off) => off());
  },

  async setTheme(id: string) {
    if (!isTheme(id) || id === theme) return;
    const previous = theme;
    error = '';
    theme = id; // optimistic: the whole UI restyles instantly
    apply(id);
    try {
      adopt(await api.setTheme(id));
    } catch (e) {
      theme = previous;
      apply(previous);
      error = errorMessage(e);
    }
  },

  /** Returns an error message, or null on success. The old shortcut stays active if this fails. */
  async setHotkey(spec: string): Promise<string | null> {
    try {
      adopt(await api.setHotkey(spec));
      return null;
    } catch (e) {
      return errorMessage(e);
    }
  },

  /** Returns an error message, or null on success. */
  async setQuietWhenPresenting(on: boolean): Promise<string | null> {
    try {
      adopt(await api.setQuietWhenPresenting(on));
      return null;
    } catch (e) {
      return errorMessage(e);
    }
  },

  /** Returns an error message, or null on success. */
  async setRechime(minutes: number): Promise<string | null> {
    try {
      adopt(await api.setRechime(minutes));
      return null;
    } catch (e) {
      return errorMessage(e);
    }
  },

  /** Returns an error message, or null on success. */
  async setAutostart(enabled: boolean): Promise<string | null> {
    try {
      autostart = await api.setAutostart(enabled);
      return null;
    } catch (e) {
      return errorMessage(e);
    }
  }
};
