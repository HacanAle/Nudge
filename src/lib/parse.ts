/**
 * Natural-language reminders: "call mom tomorrow at 5pm", "stretch in 20 min",
 * "standup every weekday 9:30", "dentist friday 3pm !!".
 *
 * Pure and dependency-free so it can be tested on its own (parse.test.ts). `now` is injectable.
 *
 * What it understands
 *   when      in 20 min · in 2 hours · in an hour · in half an hour · in 1 hour 30 min · in 3 days · in 2 weeks
 *             today · tonight · tomorrow · monday … sunday (next/this/on) · oct 12 · 12 oct · 2026-12-25 · next week
 *   at        5pm · 5:30pm · 17:30 · at 5 · 5 o'clock · noon · midnight · morning / afternoon / evening
 *   repeat    every day · daily · every weekday · weekdays · every week · every month · every monday · every 45 min · hourly
 *   priority  !! or !high · !low
 * Anything left over is the title.
 */

export type Priority = 'low' | 'normal' | 'high';

export interface Parsed {
  title: string;
  /** Unix ms, or null when the text contains no time at all. */
  dueAt: number | null;
  /** `none` | `daily` | `weekdays` | `weekly` | `monthly` | `interval:<minutes>` (same format as the backend). */
  repeat: string;
  priority: Priority;
}

const NUMBER_WORDS: Record<string, number> = {
  one: 1, two: 2, three: 3, four: 4, five: 5, six: 6, seven: 7, eight: 8, nine: 9, ten: 10,
  fifteen: 15, twenty: 20, thirty: 30, forty: 40, sixty: 60
};
const NUM = `\\d+(?:\\.\\d+)?|a|an|half\\s+an?|${Object.keys(NUMBER_WORDS).join('|')}`;

const DAY_NAMES = ['sunday', 'monday', 'tuesday', 'wednesday', 'thursday', 'friday', 'saturday'];
const DAY_FULL = '(sunday|monday|tuesday|wednesday|thursday|friday|saturday)';
// Short forms ("sat", "sun") are also ordinary words, so they only count after on / next / this / every.
const DAY_ANY = '(sun(?:day)?|mon(?:day)?|tue(?:s(?:day)?)?|wed(?:nesday)?|thu(?:r(?:s(?:day)?)?)?|fri(?:day)?|sat(?:urday)?)';

const MONTH_NAMES = ['jan', 'feb', 'mar', 'apr', 'may', 'jun', 'jul', 'aug', 'sep', 'oct', 'nov', 'dec'];
const MONTH = '(jan(?:uary)?|feb(?:ruary)?|mar(?:ch)?|apr(?:il)?|may|june?|july?|aug(?:ust)?|sep(?:t(?:ember)?)?|oct(?:ober)?|nov(?:ember)?|dec(?:ember)?)';

const MAX_INTERVAL_MINUTES = 525_600; // matches the backend's one-year cap

const dayIndex = (word: string) => DAY_NAMES.findIndex((d) => d.startsWith(word.slice(0, 3).toLowerCase()));
const monthIndex = (word: string) => MONTH_NAMES.indexOf(word.slice(0, 3).toLowerCase());
const startOfDay = (d: Date) => new Date(d.getFullYear(), d.getMonth(), d.getDate());
const addDays = (d: Date, n: number) => new Date(d.getFullYear(), d.getMonth(), d.getDate() + n);
const addMonths = (d: Date, n: number) => new Date(d.getFullYear(), d.getMonth() + n, d.getDate());
/** Move a date-time forward n calendar months, keeping its wall-clock time. */
const shiftMonths = (d: Date, n: number) =>
  new Date(d.getFullYear(), d.getMonth() + n, d.getDate(), d.getHours(), d.getMinutes(), d.getSeconds(), d.getMilliseconds());
/** Move a date-time forward n calendar days, keeping its wall-clock time (DST-safe). */
const shiftDays = (d: Date, n: number) =>
  new Date(d.getFullYear(), d.getMonth(), d.getDate() + n, d.getHours(), d.getMinutes(), d.getSeconds(), d.getMilliseconds());
const withTime = (d: Date, h: number, m: number) => new Date(d.getFullYear(), d.getMonth(), d.getDate(), h, m, 0, 0);

function amount(word: string): number {
  const w = word.toLowerCase().replace(/\s+/g, ' ');
  if (w.startsWith('half')) return 0.5;
  if (w === 'a' || w === 'an') return 1;
  return w in NUMBER_WORDS ? NUMBER_WORDS[w] : parseFloat(w);
}

/** Hours to try for a bare "at 5": the likelier reading first. */
function candidateHours(h: number): number[] {
  if (h === 12) return [12, 0];
  return h >= 7 ? [h, h + 12] : [h + 12, h];
}

export function parseReminder(input: string, now: Date = new Date()): Parsed {
  // Matched phrases are blanked out (same length) so later patterns can't reuse them.
  let text = input;
  const take = (re: RegExp): RegExpExecArray | null => {
    const m = re.exec(text);
    if (m) text = text.slice(0, m.index) + ' '.repeat(m[0].length) + text.slice(m.index + m[0].length);
    return m;
  };

  // ── priority ──
  let priority: Priority = 'normal';
  if (take(/(^|\s)(?:!{2,}|!high|!urgent)(?=\s|$)/i)) priority = 'high';
  else if (take(/(^|\s)!low(?=\s|$)/i)) priority = 'low';

  // ── repeat ──
  let repeat = 'none';
  let repeatDay: number | null = null;
  let hint: number | null = null; // an hour implied by words like "every morning"
  let m: RegExpExecArray | null;

  if ((m = take(/\bevery\s+(\d+)\s*(minutes?|mins?|m|hours?|hrs?|h)\b/i))) {
    const minutes = Number(m[1]) * (/^h/i.test(m[2]) ? 60 : 1);
    if (minutes >= 1 && minutes <= MAX_INTERVAL_MINUTES) repeat = `interval:${minutes}`;
  } else if (take(/\b(?:every\s+hour|hourly)\b/i)) repeat = 'interval:60';
  else if (take(/\bevery\s+weekdays?\b|\bweekdays\b/i)) repeat = 'weekdays';
  else if ((m = take(/\bevery\s+(day|morning|evening|night)\b|\bdaily\b/i))) {
    repeat = 'daily';
    if (m[1]) hint = ({ morning: 9, evening: 18, night: 21 } as Record<string, number | undefined>)[m[1].toLowerCase()] ?? null;
  } else if (take(/\bevery\s+week\b|\bweekly\b/i)) repeat = 'weekly';
  else if (take(/\bevery\s+month\b|\bmonthly\b/i)) repeat = 'monthly';
  else if ((m = take(new RegExp(`\\bevery\\s+${DAY_ANY}\\b`, 'i')))) {
    repeat = 'weekly';
    repeatDay = dayIndex(m[1]);
  }

  // ── "in 20 minutes" ──
  const rel = { ms: null as number | null, days: 0, months: 0 };
  const unit = (n: number, u: string) => {
    const k = u.toLowerCase();
    if (/^s/.test(k)) rel.ms = (rel.ms ?? 0) + n * 1000;
    else if (/^mo/.test(k)) rel.months += Math.round(n);
    else if (/^m/.test(k)) rel.ms = (rel.ms ?? 0) + n * 60_000;
    else if (/^h/.test(k)) rel.ms = (rel.ms ?? 0) + n * 3_600_000;
    else if (/^d/.test(k)) rel.days += n;
    else if (/^w/.test(k)) rel.days += n * 7;
  };
  if (
    (m = take(
      new RegExp(
        `\\b(?:in|after)\\s+(?:about\\s+)?(${NUM})\\s*(seconds?|secs?|months?|minutes?|mins?|m|hours?|hrs?|h|days?|d|weeks?|w)\\b(?:\\s*(?:and\\s*)?(\\d+)\\s*(minutes?|mins?|m)\\b)?`,
        'i'
      )
    ))
  ) {
    unit(amount(m[1]), m[2]);
    if (m[3]) unit(Number(m[3]), m[4]);
  } else if ((m = take(/\b(\d+(?:\.\d+)?)\s*(minutes?|mins?|hours?|hrs?|days?)\s+from\s+now\b/i))) {
    unit(parseFloat(m[1]), m[2]);
  }

  // ── date ──
  let day: Date | null = null;
  let weekday: number | null = repeatDay;
  const thisYear = now.getFullYear();
  const rollYear = (month: number, d: number): Date => {
    const candidate = new Date(thisYear, month, d);
    // "oct 12" typed in December means next year.
    return candidate < startOfDay(now) ? new Date(thisYear + 1, month, d) : candidate;
  };

  if ((m = take(/\b(\d{4})-(\d{1,2})-(\d{1,2})\b/))) {
    day = new Date(Number(m[1]), Number(m[2]) - 1, Number(m[3]));
  } else if ((m = take(new RegExp(`\\b(?:on\\s+)?${MONTH}\\.?\\s+(\\d{1,2})(?:st|nd|rd|th)?\\b`, 'i')))) {
    day = rollYear(monthIndex(m[1]), Number(m[2]));
  } else if ((m = take(new RegExp(`\\b(?:on\\s+)?(\\d{1,2})(?:st|nd|rd|th)?\\s+(?:of\\s+)?${MONTH}\\b`, 'i')))) {
    day = rollYear(monthIndex(m[2]), Number(m[1]));
  } else if ((m = take(/\b(today|tonight|tomorrow|tmrw|tmr)\b/i))) {
    const w = m[1].toLowerCase();
    day = startOfDay(now);
    if (w === 'tonight') hint = 20;
    else if (w !== 'today') day = addDays(day, 1);
  } else if ((m = take(/\bnext\s+(week|month)\b/i))) {
    day = m[1].toLowerCase() === 'week' ? addDays(startOfDay(now), 7) : addMonths(startOfDay(now), 1);
  } else if ((m = take(new RegExp(`\\b(?:next|this|on)\\s+${DAY_ANY}\\b|\\b${DAY_FULL}\\b`, 'i')))) {
    weekday = dayIndex(m[1] ?? m[2]);
  }

  // ── time of day ──
  interface Clock { h: number; m: number; certain: boolean }
  let clock: Clock | null = null;

  if ((m = take(/\b(noon|midday)\b/i))) clock = { h: 12, m: 0, certain: true };
  else if (take(/\bmidnight\b/i)) clock = { h: 0, m: 0, certain: true };
  else if ((m = take(/(?:\b(?:at|by)\s+|@\s*|\b)(\d{1,2})(?::(\d{2}))?\s*(a\.?m\.?|p\.?m\.?)(?![a-z])/i))) {
    const h = Number(m[1]);
    if (h >= 1 && h <= 12) clock = { h: (h % 12) + (/^p/i.test(m[3]) ? 12 : 0), m: Number(m[2] ?? 0), certain: true };
  } else if ((m = take(/\b(\d{1,2})\s*o'?clock\b/i))) {
    const h = Number(m[1]);
    if (h >= 1 && h <= 12) clock = { h, m: 0, certain: false };
  } else if ((m = take(/(?:\b(?:at|by)\s+)?\b(0?\d|1\d|2[0-3]):([0-5]\d)\b(?!\s*(?:am|pm))/i))) {
    const h = Number(m[1]);
    const twentyFourHour = h === 0 || h >= 13 || m[1].startsWith('0');
    clock = { h, m: Number(m[2]), certain: twentyFourHour };
  } else if ((m = take(/\b(?:at|by|@)\s*(\d{1,2})(?::(\d{2}))?\b/i))) {
    const h = Number(m[1]);
    if (h <= 23) clock = { h, m: Number(m[2] ?? 0), certain: h === 0 || h >= 13 };
  }

  if ((m = take(/\b(?:this\s+|in\s+the\s+)?(morning|afternoon|evening)\b/i)) && hint === null) {
    hint = ({ morning: 9, afternoon: 15, evening: 18 } as Record<string, number>)[m[1].toLowerCase()];
  }

  return { title: cleanTitle(text), dueAt: resolve(), repeat, priority };

  // ── put it together ──
  function resolve(): number | null {
    const today = startOfDay(now);

    // "in 20 minutes" / "in 2 hours" (optionally plus days): an exact offset from now.
    if (rel.ms !== null) return shiftMonths(shiftDays(now, rel.days), rel.months).getTime() + rel.ms;

    // "every 45 min" with nothing else: the first one is one interval away.
    const interval = /^interval:(\d+)$/.exec(repeat);
    const anchored = day || weekday !== null || rel.days || rel.months || clock || hint !== null;
    if (interval && !anchored) return now.getTime() + Number(interval[1]) * 60_000;

    // Which calendar day?
    let base: Date | null = null;
    let explicit = true;
    const relative = rel.days > 0 || rel.months > 0;
    if (relative) base = addMonths(addDays(today, rel.days), rel.months);
    else if (day) base = day;
    else if (weekday !== null) base = addDays(today, (weekday - today.getDay() + 7) % 7);
    else if (clock || hint !== null || repeat !== 'none') {
      base = today;
      explicit = false;
    } else return null;

    // What time on that day?
    let h: number;
    let min: number;
    if (clock) {
      min = clock.m;
      if (clock.certain) h = clock.h;
      else {
        const options = clock.h >= 1 && clock.h <= 12 ? candidateHours(clock.h) : [clock.h];
        // One-offs prefer the next time that is still ahead today ("meeting at 9" at 10:30 → 21:00);
        // repeats keep the likeliest hour and just start tomorrow.
        const isToday = base.getTime() === today.getTime() && repeat === 'none';
        const later = isToday ? options.find((c) => withTime(base!, c, min) > now) : undefined;
        h = later ?? options[0];
      }
    } else if (hint !== null) {
      h = hint;
      min = 0;
    } else if (relative) {
      const t = new Date(Math.ceil(now.getTime() / 300_000) * 300_000);
      h = t.getHours();
      min = t.getMinutes();
    } else if (explicit && base.getTime() === today.getTime() && !weekday && repeat === 'none') {
      // "today" with no time: a sensible hour from now rather than 9am that already passed.
      const soon = new Date(Math.ceil((now.getTime() + 3_600_000) / 300_000) * 300_000);
      return soon.getTime();
    } else {
      h = 9;
      min = 0;
    }

    let due = withTime(base, h, min);

    // A bare time or a repeat that has already passed today starts tomorrow; a bare weekday starts next week.
    if (due <= now && (!explicit || weekday !== null)) due = shiftDays(due, explicit ? 7 : 1);
    // Weekday-only repeats never start on a weekend.
    if (repeat === 'weekdays') while (due.getDay() === 0 || due.getDay() === 6) due = shiftDays(due, 1);

    return due.getTime();
  }
}

/** Strip the filler left behind once the time phrases are gone, and tidy the rest into a title. */
function cleanTitle(raw: string): string {
  let t = raw.replace(/\s+/g, ' ').trim();
  t = t.replace(/^(?:please\s+)?(?:remind\s+me\s+(?:to|about|that|of)?|reminder\s*[:\-]?|remember\s+to)\s*/i, '');
  // Connectors stranded where a phrase was cut out: "call mom at", "and then", "to call".
  for (let i = 0; i < 4; i++) {
    t = t
      .replace(/^(?:to|and|then|that)\s+/i, '')
      .replace(/\s+(?:at|on|in|by|for|from|every|and|the|this|next|of|to|then)$/i, '');
  }
  t = t.replace(/^[\s,.\-–—:;]+|[\s,\-–—:;]+$/g, '').trim();
  return t ? t.charAt(0).toUpperCase() + t.slice(1) : '';
}
