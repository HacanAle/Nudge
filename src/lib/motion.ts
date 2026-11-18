import type { TransitionConfig } from 'svelte/transition';
import { backOut, bounceOut, cubicIn, cubicOut, linear } from 'svelte/easing';
import { settings } from './prefs.svelte';
import { themeInfo } from './themes';

/**
 * How a floating window (alert, quick-add) arrives and leaves. Each theme has its own, so the
 * personality shows in the first third of a second.
 */
export function arrive(_node: Element, { leaving = false } = {}): TransitionConfig {
  switch (themeInfo(settings.theme).motion) {
    case 'spring':
      return leaving
        ? { duration: 170, easing: cubicIn, css: (t) => `opacity: ${t}; transform: scale(${0.96 + 0.04 * t});` }
        : {
            duration: 460,
            easing: backOut,
            css: (t, u) => `opacity: ${Math.min(1, t * 2)}; transform: translateY(${-22 * u}px) scale(${0.94 + 0.06 * t});`
          };
    case 'fade':
      return {
        duration: leaving ? 200 : 380,
        easing: cubicOut,
        css: (t, u) => `opacity: ${t}; transform: translateY(${-8 * u}px) scale(${0.985 + 0.015 * t});`
      };
    case 'snap':
      // Stepped, like a window being redrawn on old hardware.
      return {
        duration: leaving ? 90 : 160,
        easing: (t) => Math.round(t * 4) / 4,
        css: (t, u) => `opacity: ${t > 0 ? 1 : 0}; transform: translateY(${-10 * u}px);`
      };
    case 'slam':
      return leaving
        ? { duration: 190, easing: cubicIn, css: (t, u) => `opacity: ${t}; transform: translateY(${-34 * u}px) rotate(${7 * u}deg);` }
        : {
            duration: 560,
            easing: bounceOut,
            css: (t, u) => `opacity: ${Math.min(1, t * 5)}; transform: translateY(${-70 * u}px) rotate(${-6 * u}deg);`
          };
    case 'flicker': {
      const frames = [0, 0.9, 0.1, 1, 0.35, 1, 0.7, 1];
      return {
        duration: leaving ? 140 : 400,
        easing: linear,
        css: (t, u) => {
          const lit = leaving ? t : frames[Math.min(frames.length - 1, Math.floor(t * frames.length))];
          return `opacity: ${lit}; transform: translateY(${-6 * u}px);`;
        }
      };
    }
    case 'unfurl':
      return {
        duration: leaving ? 200 : 420,
        easing: cubicOut,
        css: (t, u) => `opacity: ${t}; transform-origin: top center; transform: translateY(${-10 * u}px) scaleY(${0.8 + 0.2 * t});`
      };
    default:
      return {
        duration: leaving ? 160 : 260,
        easing: leaving ? cubicIn : cubicOut,
        css: (t, u) => `opacity: ${t}; transform: translateY(${-14 * u}px);`
      };
  }
}
