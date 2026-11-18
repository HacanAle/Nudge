/**
 * The theme catalogue. Looks are defined in `src/themes.css` (one block per id); the backend keeps a
 * matching id list in `src-tauri/src/settings.rs` so it can validate what gets saved.
 */

/** How the alert enters. Each theme has its own, so the personality survives the first 300 ms. */
export type Motion = 'slide' | 'spring' | 'fade' | 'snap' | 'slam' | 'flicker' | 'unfurl';

export interface ThemeInfo {
  id: string;
  /** The theme's own name. */
  name: string;
  /** The design style it answers to. */
  style: string;
  blurb: string;
  motion: Motion;
  /** Follows the Windows light/dark setting (the others are a single fixed palette). */
  followsSystem?: boolean;
}

export const THEMES: ThemeInfo[] = [
  {
    id: 'slab',
    name: 'Slab',
    style: 'Flat design',
    blurb: 'Solid surfaces, hairlines, no effects. Quiet on purpose.',
    motion: 'slide',
    followsSystem: true
  },
  {
    id: 'orchard',
    name: 'Orchard',
    style: 'Apple-like',
    blurb: 'Soft corners, system blue, pill buttons and grouped lists.',
    motion: 'spring',
    followsSystem: true
  },
  {
    id: 'vitrine',
    name: 'Vitrine',
    style: 'Glassmorphism',
    blurb: 'Frosted glass over a blurred copy of whatever is behind the alert.',
    motion: 'fade'
  },
  {
    id: 'beige',
    name: 'Beige Box',
    style: 'Retro',
    blurb: 'Bevelled grey, pixel type, and a title bar on every alert.',
    motion: 'snap'
  },
  {
    id: 'punk',
    name: 'Safety Pin',
    style: 'Punk',
    blurb: 'A photocopied flyer: thick ink, hot pink, a little crooked.',
    motion: 'slam'
  },
  {
    id: 'voltage',
    name: 'Voltage',
    style: 'Electric',
    blurb: 'Black glass, cyan current and a yellow button.',
    motion: 'flicker'
  },
  {
    id: 'handbill',
    name: 'Handbill',
    style: 'Old-fashioned ad',
    blurb: 'Cream paper, red ink and a rubber stamp for the status.',
    motion: 'unfurl'
  },
  {
    id: 'nudge',
    name: 'Nudge',
    style: 'The app icon',
    blurb: 'Black and grey, with one orange alert on top and the next ones stacked behind it.',
    motion: 'spring'
  }
];

export const DEFAULT_THEME = 'slab';

export const isTheme = (id: unknown): id is string => THEMES.some((t) => t.id === id);

export const themeInfo = (id: string): ThemeInfo => THEMES.find((t) => t.id === id) ?? THEMES[0];
