/*
 * Built-in colour themes. Each theme is five colours; everything else (hover states, borders, muted text,
 * filled buttons) is derived so a theme cannot end up with unreadable text. "classic" is not in this list:
 * it is the original look that follows the system light/dark setting and the accent colour picker.
 */

export interface ThemeDef {
  id: string;
  name: string;
  /** Light themes get the light variant of scrollbars, form controls and shadows. */
  dark: boolean;
  bg: string;
  surface: string;
  text: string;
  accent: string;
}

export const THEMES: ThemeDef[] = [
  { id: 'midnight', name: 'Midnight', dark: true, bg: '#0f1220', surface: '#181b2e', text: '#dfe3f7', accent: '#7aa2f7' },
  { id: 'graphite', name: 'Graphite', dark: true, bg: '#141414', surface: '#1e1e20', text: '#e8e8ea', accent: '#8ab4f8' },
  { id: 'amoled', name: 'AMOLED black', dark: true, bg: '#000000', surface: '#0c0c0e', text: '#f1f1f3', accent: '#c4a1ff' },
  { id: 'ocean', name: 'Ocean', dark: true, bg: '#07131b', surface: '#0e212d', text: '#dcf0f7', accent: '#3cc3e8' },
  { id: 'forest', name: 'Forest', dark: true, bg: '#1e2326', surface: '#272e33', text: '#d3c6aa', accent: '#a7c080' },
  { id: 'sunset', name: 'Sunset', dark: true, bg: '#1c1210', surface: '#2b1c18', text: '#fbeee6', accent: '#ff8a4c' },
  { id: 'rose', name: 'Rosé', dark: true, bg: '#191724', surface: '#1f1d2e', text: '#e0def4', accent: '#eb6f92' },
  { id: 'lavender', name: 'Lavender', dark: true, bg: '#181825', surface: '#1e1e2e', text: '#cdd6f4', accent: '#cba6f7' },
  { id: 'nord', name: 'Nord', dark: true, bg: '#2e3440', surface: '#3b4252', text: '#eceff4', accent: '#88c0d0' },
  { id: 'dracula', name: 'Dracula', dark: true, bg: '#282a36', surface: '#343746', text: '#f8f8f2', accent: '#bd93f9' },
  { id: 'gruvbox', name: 'Gruvbox', dark: true, bg: '#282828', surface: '#3c3836', text: '#ebdbb2', accent: '#fabd2f' },
  { id: 'solarized-dark', name: 'Solarized dark', dark: true, bg: '#002b36', surface: '#073642', text: '#eee8d5', accent: '#2f95c9' },
  { id: 'blossom', name: 'Blossom pink', dark: false, bg: '#fff1f5', surface: '#ffffff', text: '#2d1b24', accent: '#d6336c' },
  { id: 'paper', name: 'Paper', dark: false, bg: '#f4efe6', surface: '#fffcf6', text: '#2b2620', accent: '#b45f06' },
  { id: 'sky', name: 'Sky', dark: false, bg: '#eaf3fb', surface: '#ffffff', text: '#10222e', accent: '#1478c8' },
  { id: 'mint', name: 'Mint', dark: false, bg: '#e9f6f0', surface: '#ffffff', text: '#10261c', accent: '#14805a' },
  { id: 'latte', name: 'Latte', dark: false, bg: '#eff1f5', surface: '#ffffff', text: '#262940', accent: '#8839ef' },
  { id: 'solarized-light', name: 'Solarized light', dark: false, bg: '#fdf6e3', surface: '#eee8d5', text: '#073642', accent: '#1f78b4' },
];

export const DEFAULT_PALETTE = 'classic';

/** The default look ("plum & apricot"): warm paper by day, warm near-black by night; the accent comes from the picker. */
export function classicTheme(dark: boolean, accent: string): ThemeDef {
  return dark
    ? { name: 'Plum & paper', id: 'classic', dark: true, bg: '#131112', surface: '#1c191b', text: '#f1ecea', accent }
    : { name: 'Plum & paper', id: 'classic', dark: false, bg: '#f7f3ee', surface: '#ffffff', text: '#221c1f', accent };
}

/** "New": unread counts and your reactions. Dark text on it. */
export const APRICOT = '#f2a65a';

export function themeById(id: string): ThemeDef | undefined {
  return THEMES.find((t) => t.id === id);
}

/* ---------------- colour maths ---------------- */

type Rgb = [number, number, number];

export function parseHex(hex: string): Rgb {
  const v = hex.replace('#', '');
  return [0, 2, 4].map((i) => Number.parseInt(v.slice(i, i + 2), 16)) as Rgb;
}

const toHex = (c: Rgb): string => `#${c.map((n) => Math.max(0, Math.min(255, Math.round(n))).toString(16).padStart(2, '0')).join('')}`;

/** `t` of the way from a to b. */
export function mix(a: string, b: string, t: number): string {
  const x = parseHex(a);
  const y = parseHex(b);
  return toHex([0, 1, 2].map((i) => x[i]! + (y[i]! - x[i]!) * t) as Rgb);
}

const lum = (hex: string): number => {
  const [r, g, b] = parseHex(hex).map((n) => {
    const s = n / 255;
    return s <= 0.03928 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4;
  }) as Rgb;
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
};

/** WCAG contrast ratio, 1 to 21. */
export function contrast(a: string, b: string): number {
  const [hi, lo] = [lum(a), lum(b)].sort((p, q) => q - p) as [number, number];
  return (hi + 0.05) / (lo + 0.05);
}

const rgba = (hex: string, alpha: number): string => {
  const [r, g, b] = parseHex(hex);
  return `rgba(${r}, ${g}, ${b}, ${alpha})`;
};

/** Darken the accent until white text on it reaches 4.5:1, so filled buttons and own message bubbles stay readable. */
export function accentFill(accent: string): string {
  let fill = accent;
  for (let i = 0; i < 20 && contrast('#ffffff', fill) < 4.5; i += 1) fill = mix(fill, '#000000', 0.08);
  return fill;
}

/** The accent as text and lines on the background: lightened (dark themes) or darkened until it reads at 4.5:1. */
export function accentText(accent: string, bg: string | string[], dark: boolean): string {
  const grounds = Array.isArray(bg) ? bg : [bg];
  let c = accent;
  for (let i = 0; i < 25 && grounds.some((g) => contrast(c, g) < 4.5); i += 1) c = mix(c, dark ? '#ffffff' : '#000000', 0.1);
  return c;
}

/** Every CSS variable a theme sets. Removing these puts the classic look back. */
export function themeVars(t: ThemeDef): Record<string, string> {
  const fill = accentFill(t.accent);
  const elevated = mix(t.bg, t.surface, 0.5);
  // Surfaces step by tone, not shadow: "tonal" is the incoming bubble, fields and chips; "ink" is neutral controls.
  const tonal = mix(t.bg, t.text, t.dark ? 0.1 : 0.055);
  const tonalStrong = mix(t.bg, t.text, t.dark ? 0.15 : 0.09);
  const surfaceHover = mix(t.surface, t.text, 0.06);
  const surfaceStrong = mix(t.surface, t.text, 0.12);
  // Every colour text can sit on: secondary text and the accent as text must read on all of them (themes.test.ts).
  const grounds = [t.bg, elevated, mix(t.bg, t.surface, 0.3), t.surface, surfaceHover, surfaceStrong, tonal, tonalStrong];
  let mutedAmount = 0.3;
  let muted = mix(t.text, t.bg, mutedAmount);
  while (mutedAmount > 0 && grounds.some((g) => contrast(muted, g) < 4.5)) {
    mutedAmount = Math.max(0, mutedAmount - 0.02);
    muted = mix(t.text, t.bg, mutedAmount);
  }
  const danger = accentText(t.dark ? '#ff8a80' : '#b42318', grounds, t.dark);
  return {
    '--bg': t.bg,
    '--bg-elevated': elevated,
    '--sidebar': mix(t.bg, t.surface, 0.3),
    '--surface': t.surface,
    '--surface-hover': surfaceHover,
    '--surface-strong': surfaceStrong,
    '--text': t.text,
    '--text-soft': mix(t.text, t.bg, 0.16),
    // Secondary text: as light as it can be while still reading at 4.5:1 on the ground and on tonal surfaces.
    '--muted': muted,
    '--border': rgba(t.text, 0.1),
    '--border-strong': rgba(t.text, 0.2),
    // Text, links, dots and outlines use a readable accent; filled things (buttons, your bubbles) use --accent-fill.
    '--accent': accentText(t.accent, grounds, t.dark),
    '--accent-soft': rgba(t.accent, 0.18),
    '--accent-strong': mix(t.accent, '#000000', 0.14),
    '--accent-fill': fill,
    '--accent-fill-strong': mix(fill, '#000000', 0.2),
    '--bubble-own': fill,
    '--bubble-theirs': tonal,
    '--tonal': tonal,
    '--tonal-strong': tonalStrong,
    '--ink': t.text,
    '--on-ink': t.bg,
    '--apricot': APRICOT,
    '--on-apricot': '#2b1608',
    '--apricot-container': t.dark ? '#3a2516' : '#fce6d6',
    '--on-apricot-container': t.dark ? '#ffd2b3' : '#5a2a0c',
    '--accent-container': mix(t.bg, t.accent, t.dark ? 0.22 : 0.14),
    '--on-accent-container': t.dark ? mix(t.accent, '#ffffff', 0.55) : mix(t.accent, '#000000', 0.45),
    '--danger': danger,
    '--danger-soft': t.dark ? 'rgba(248, 113, 113, 0.14)' : 'rgba(220, 38, 38, 0.09)',
    '--warning': t.dark ? '#fbbf24' : '#b45309',
    '--success': t.dark ? '#34d399' : '#059669',
    '--shadow': t.dark ? '0 24px 64px rgba(0, 0, 0, 0.5)' : '0 24px 64px rgba(34, 28, 31, 0.14)',
  };
}
