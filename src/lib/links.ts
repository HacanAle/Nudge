/**
 * Links in a reminder's note: split the text so links can be clicked, find a meeting link for "Join",
 * and shorten long links for display. Pure, so `npm test` covers it (links.test.ts).
 */

export interface Segment {
  text: string;
  /** Set when this piece is a link. */
  href?: string;
}

const LINK = /\b(?:https?:\/\/|www\.)[^\s<>"']+|\bmailto:[^\s<>"']+/gi;
/** Punctuation that ends the sentence around a link rather than the link itself. */
const TRAILING = '.,;:!?\'"”’»]}';

/** Drop trailing punctuation; a closing bracket stays only if the link opened one ("wiki/Foo_(bar)"). */
function trim(raw: string): string {
  let s = raw;
  for (;;) {
    const last = s.at(-1) ?? '';
    const unbalanced = last === ')' && (s.match(/\(/g)?.length ?? 0) < (s.match(/\)/g)?.length ?? 0);
    if (TRAILING.includes(last) || unbalanced) s = s.slice(0, -1);
    else return s;
  }
}

export function splitLinks(text: string): Segment[] {
  const out: Segment[] = [];
  let last = 0;
  for (const m of text.matchAll(LINK)) {
    const raw = trim(m[0]);
    if (!raw || raw.endsWith('//') || /^www\.?$/i.test(raw)) continue;
    const start = m.index ?? 0;
    if (start > last) out.push({ text: text.slice(last, start) });
    out.push({ text: raw, href: /^www\./i.test(raw) ? `https://${raw}` : raw });
    last = start + raw.length;
  }
  if (last < text.length) out.push({ text: text.slice(last) });
  return out;
}

const MEETING_HOST =
  /(^|\.)(teams\.microsoft\.com|teams\.live\.com|zoom\.us|meet\.google\.com|webex\.com|whereby\.com|meet\.jit\.si|gotomeeting\.com|meet\.goto\.com|chime\.aws)$/i;

/** The first video-meeting link in the text (Teams, Zoom, Meet, Webex…), for the alert's "Join" button. */
export function meetingLink(text: string): string | null {
  for (const part of splitLinks(text)) {
    if (!part.href?.startsWith('http')) continue;
    try {
      if (MEETING_HOST.test(new URL(part.href).hostname)) return part.href;
    } catch {
      /* not a parseable URL */
    }
  }
  return null;
}

/** "teams.microsoft.com/l/meetup-join/19%3am…" — readable in a note; the full link is in the tooltip. */
export function shortLink(href: string, max = 44): string {
  const bare = href.replace(/^(https?:\/\/|mailto:)/i, '').replace(/^www\./i, '').replace(/\/$/, '');
  return bare.length > max ? `${bare.slice(0, max - 1)}…` : bare;
}
