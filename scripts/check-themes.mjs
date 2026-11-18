// Guards src/themes.css:
//   1. every theme defines every token the first (Slab) block defines — a theme that forgets one
//      would silently inherit it from another theme;
//   2. every theme id in themes.ts has a block, and vice versa;
//   3. no colour in the stylesheet is purple / violet / indigo.
import { readFileSync } from 'node:fs';

const css = readFileSync(new URL('../src/themes.css', import.meta.url), 'utf8').replace(/\/\*[\s\S]*?\*\//g, '');
const ts = readFileSync(new URL('../src/lib/themes.ts', import.meta.url), 'utf8');

const failures = [];

// ── split into rule blocks (the file has no nested braces except @media/@font-face) ──
const blocks = [];
const re = /([^{}]+)\{([^{}]*)\}/g;
for (let m; (m = re.exec(css)); ) blocks.push({ selector: m[1].trim(), body: m[2] });

const tokensOf = (body) => new Set([...body.matchAll(/(--[\w-]+)\s*:/g)].map((x) => x[1]));

// Theme token blocks are the ones whose selector is exactly `[data-theme='x']` (Slab also lists :root).
// Skip @media overrides: they intentionally redefine only a subset.
const themeBlocks = new Map();
for (const b of blocks) {
  const m = /^(?::root,\s*)?\[data-theme='([\w-]+)'\]$/.exec(b.selector.replace(/\s+/g, ' '));
  if (m && !themeBlocks.has(m[1]) && tokensOf(b.body).size > 20) themeBlocks.set(m[1], tokensOf(b.body));
}

const baseline = themeBlocks.get('slab');
if (!baseline) failures.push('no Slab token block found');

// Tokens that only some themes need (extras), so they are not required everywhere.
const OPTIONAL = new Set(['--bevel-out', '--bevel-in', '--card-fallback', '--snap-x', '--snap-y']);

for (const [id, tokens] of themeBlocks) {
  for (const t of baseline ?? []) if (!tokens.has(t)) failures.push(`theme "${id}" is missing ${t}`);
  for (const t of tokens) if (!baseline?.has(t) && !OPTIONAL.has(t)) failures.push(`theme "${id}" defines unknown token ${t}`);
}

// ── ids agree with themes.ts ──
const declared = [...ts.matchAll(/^\s+id:\s*'([\w-]+)'/gm)].map((m) => m[1]);
for (const id of declared) if (!themeBlocks.has(id)) failures.push(`themes.ts lists "${id}" but themes.css has no block for it`);
for (const id of themeBlocks.keys()) if (!declared.includes(id)) failures.push(`themes.css has a "${id}" block that themes.ts doesn't list`);

// ── no purple ──
function hue(r, g, b) {
  r /= 255; g /= 255; b /= 255;
  const max = Math.max(r, g, b), min = Math.min(r, g, b), d = max - min;
  if (d === 0) return { h: 0, s: 0 };
  const l = (max + min) / 2;
  const s = d / (1 - Math.abs(2 * l - 1));
  let h = max === r ? ((g - b) / d) % 6 : max === g ? (b - r) / d + 2 : (r - g) / d + 4;
  h = (h * 60 + 360) % 360;
  return { h, s };
}
const colours = [
  ...[...css.matchAll(/#([0-9a-f]{6}|[0-9a-f]{3})\b/gi)].map((m) => {
    let x = m[1];
    if (x.length === 3) x = [...x].map((c) => c + c).join('');
    return { text: m[0], rgb: [0, 2, 4].map((i) => parseInt(x.slice(i, i + 2), 16)) };
  }),
  ...[...css.matchAll(/rgba?\(\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)/g)].map((m) => ({ text: m[0] + ')', rgb: [+m[1], +m[2], +m[3]] }))
];
for (const { text, rgb } of colours) {
  const { h, s } = hue(...rgb);
  if (s > 0.15 && h >= 255 && h <= 315) failures.push(`purple-ish colour ${text} (hue ${Math.round(h)}°)`);
}

if (failures.length) {
  console.error(`✗ themes.css has ${failures.length} problem(s):\n  - ${failures.join('\n  - ')}`);
  process.exit(1);
}
console.log(`✓ ${themeBlocks.size} themes × ${baseline.size} tokens, ${colours.length} colours checked, no purple`);
