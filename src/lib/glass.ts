import type { GlassSnapshot } from './api';

/**
 * Inline custom properties for a floating window's stage under the glass theme: the blurred screen
 * snapshot the backend took, or — if it couldn't — a solid card so text never floats over a busy desktop.
 */
export function glassStyle(theme: string, snapshot: GlassSnapshot | null): string {
  if (theme !== 'vitrine') return '';
  if (!snapshot) return '--card-bg: var(--card-fallback);';
  return `--snap: url("${snapshot.url}"); --snap-w: ${snapshot.width}px; --snap-h: ${snapshot.height}px;`;
}
