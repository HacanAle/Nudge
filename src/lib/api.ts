import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';

export type Priority = 'low' | 'normal' | 'high';
export type Status = 'pending' | 'active' | 'done' | 'dismissed';

/** `none` | `daily` | `weekdays` | `weekly` | `monthly` | `interval:<minutes>` */
export type Repeat = string;

export interface Reminder {
  id: number;
  title: string;
  note: string;
  priority: Priority;
  dueAt: number;
  anchorAt: number;
  repeat: Repeat;
  status: Status;
  createdAt: number;
  completedAt: number | null;
}

export interface NewReminder {
  title: string;
  note: string;
  priority: Priority;
  dueAt: number;
  repeat: Repeat;
}

export interface Settings {
  theme: string;
  /** The quick-add shortcut, e.g. "Ctrl+Alt+N". */
  hotkey: string;
  /** False when Windows refused the shortcut (another app owns it). */
  hotkeyActive: boolean;
  /** Hold alerts during full-screen apps / presentations and keep them out of screen shares. */
  quietWhenPresenting: boolean;
  /** Chime again every N minutes while an alert waits (0 = off). */
  rechimeMinutes: number;
}

export interface ImportResult {
  added: number;
  skipped: number;
}

/** A copy of the screen behind the alert, for the glass theme. */
export interface GlassSnapshot {
  url: string;
  width: number;
  height: number;
}

export const api = {
  getSettings: () => invoke<Settings>('get_settings'),
  setTheme: (theme: string) => invoke<Settings>('set_theme', { theme }),
  setHotkey: (hotkey: string) => invoke<Settings>('set_hotkey', { hotkey }),
  getAutostart: () => invoke<boolean>('get_autostart'),
  setAutostart: (enabled: boolean) => invoke<boolean>('set_autostart', { enabled }),
  hideQuick: () => invoke<void>('hide_quick'),
  setQuietWhenPresenting: (on: boolean) => invoke<Settings>('set_quiet_when_presenting', { on }),
  setRechime: (minutes: number) => invoke<Settings>('set_rechime', { minutes }),
  openLink: (url: string) => invoke<void>('open_link', { url }),
  exportReminders: () => invoke<string>('export_reminders'),
  importReminders: (json: string) => invoke<ImportResult>('import_reminders', { json }),
  listUpcoming: () => invoke<Reminder[]>('list_upcoming'),
  listHistory: (limit = 50) => invoke<Reminder[]>('list_history', { limit }),
  getActive: () => invoke<Reminder[]>('get_active'),
  add: (input: NewReminder) => invoke<Reminder>('add_reminder', { input }),
  update: (id: number, input: NewReminder) => invoke<Reminder>('update_reminder', { id, input }),
  remove: (id: number) => invoke<void>('delete_reminder', { id }),
  clearHistory: () => invoke<void>('clear_history'),
  finish: (id: number, dismissed: boolean) => invoke<void>('finish_reminder', { id, dismissed }),
  snooze: (id: number, until: number) => invoke<void>('snooze_reminder', { id, until }),
  hideOverlay: () => invoke<void>('hide_overlay'),
  getDnd: () => invoke<boolean>('get_dnd'),
  setDnd: (enabled: boolean) => invoke<void>('set_dnd', { enabled }),
  sendTestAlert: () => invoke<void>('send_test_alert')
};

/** Subscribe to a backend event; returns an unsubscribe function that is safe to call immediately. */
export function on<T>(event: string, handler: (payload: T) => void): () => void {
  let off: UnlistenFn | undefined;
  let cancelled = false;
  listen<T>(event, (e) => handler(e.payload)).then((fn) => {
    if (cancelled) fn();
    else off = fn;
  });
  return () => {
    cancelled = true;
    off?.();
  };
}

export function errorMessage(e: unknown): string {
  return typeof e === 'string' ? e : e instanceof Error ? e.message : 'Something went wrong';
}
