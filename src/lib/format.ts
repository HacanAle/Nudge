const MIN = 60_000;
const HOUR = 60 * MIN;

const timeFmt = new Intl.DateTimeFormat(undefined, { hour: 'numeric', minute: '2-digit' });
const dayFmt = new Intl.DateTimeFormat(undefined, { weekday: 'short', month: 'short', day: 'numeric' });

export function startOfDay(ms: number): number {
  const d = new Date(ms);
  d.setHours(0, 0, 0, 0);
  return d.getTime();
}

export function formatTime(ms: number): string {
  return timeFmt.format(ms);
}

/** "Today 3:30 PM", "Tomorrow 9:00 AM", "Mon, Mar 16 · 9:00 AM" */
export function formatWhen(ms: number, now = Date.now()): string {
  const dayDiff = Math.round((startOfDay(ms) - startOfDay(now)) / (24 * HOUR));
  if (dayDiff === 0) return `Today ${formatTime(ms)}`;
  if (dayDiff === 1) return `Tomorrow ${formatTime(ms)}`;
  if (dayDiff === -1) return `Yesterday ${formatTime(ms)}`;
  return `${dayFmt.format(ms)} · ${formatTime(ms)}`;
}

/** "Today", "Tomorrow", "Mon 12 Oct" */
export function formatDay(ms: number, now = Date.now()): string {
  const dayDiff = Math.round((startOfDay(ms) - startOfDay(now)) / (24 * HOUR));
  if (dayDiff === 0) return 'Today';
  if (dayDiff === 1) return 'Tomorrow';
  if (dayDiff === -1) return 'Yesterday';
  return dayFmt.format(ms);
}

const headingFmt = new Intl.DateTimeFormat(undefined, { weekday: 'long', day: 'numeric', month: 'long' });

/** "Thursday, 8 October" — the manager's heading. */
export function formatHeading(ms = Date.now()): string {
  return headingFmt.format(ms);
}

/** "in 12 min", "in 2 h", "5 min ago" — short and glanceable. */
export function formatRelative(ms: number, now = Date.now()): string {
  const diff = ms - now;
  const abs = Math.abs(diff);
  let text: string;
  if (abs < 45_000) return 'now';
  if (abs < HOUR) text = `${Math.round(abs / MIN)} min`;
  else if (abs < 24 * HOUR) {
    const h = abs / HOUR;
    text = `${h < 10 ? Math.round(h * 10) / 10 : Math.round(h)} h`;
  } else text = `${Math.round(abs / (24 * HOUR))} d`;
  return diff > 0 ? `in ${text}` : `${text} ago`;
}

export const REPEAT_OPTIONS: { value: string; label: string }[] = [
  { value: 'none', label: "Doesn't repeat" },
  { value: 'daily', label: 'Every day' },
  { value: 'weekdays', label: 'Weekdays' },
  { value: 'weekly', label: 'Every week' },
  { value: 'monthly', label: 'Every month' },
  { value: 'interval:15', label: 'Every 15 min' },
  { value: 'interval:30', label: 'Every 30 min' },
  { value: 'interval:45', label: 'Every 45 min' },
  { value: 'interval:60', label: 'Every hour' },
  { value: 'interval:120', label: 'Every 2 hours' }
];

export function repeatLabel(rule: string): string {
  const preset = REPEAT_OPTIONS.find((o) => o.value === rule);
  if (preset) return preset.label;
  const m = /^interval:(\d+)$/.exec(rule);
  return m ? `Every ${m[1]} min` : rule;
}

/** `<input type="datetime-local">` wants local time as YYYY-MM-DDTHH:mm. */
export function toLocalInput(ms: number): string {
  const d = new Date(ms);
  const p = (n: number) => String(n).padStart(2, '0');
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}T${p(d.getHours())}:${p(d.getMinutes())}`;
}

export function fromLocalInput(value: string): number {
  return new Date(value).getTime();
}

/** `minutes` from now, rounded up to the next 5-minute mark so times look intentional. */
export function inMinutes(minutes: number, now = Date.now()): number {
  const t = now + minutes * MIN;
  const step = 5 * MIN;
  return Math.ceil(t / step) * step;
}

export function atHour(hour: number, daysAhead: number, now = Date.now()): number {
  const d = new Date(now);
  d.setDate(d.getDate() + daysAhead);
  d.setHours(hour, 0, 0, 0);
  return d.getTime();
}
