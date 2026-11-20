export interface QuietHours {
  enabled: boolean;
  /** "HH:MM", 24 hours. The window may cross midnight (22:00 to 07:00). */
  from: string;
  to: string;
}

export const DEFAULT_QUIET: QuietHours = { enabled: false, from: '22:00', to: '07:00' };

const TIME = /^([01]\d|2[0-3]):([0-5]\d)$/;

function minutes(t: string): number {
  const m = TIME.exec(t);
  return m ? Number(m[1]) * 60 + Number(m[2]) : 0;
}

export function parseQuiet(raw: unknown): QuietHours {
  const r = (raw ?? {}) as Partial<QuietHours>;
  return {
    enabled: r.enabled === true,
    from: typeof r.from === 'string' && TIME.test(r.from) ? r.from : DEFAULT_QUIET.from,
    to: typeof r.to === 'string' && TIME.test(r.to) ? r.to : DEFAULT_QUIET.to,
  };
}

/** Is `now` inside the quiet window? (A window with equal ends is empty.) */
export function inQuietHours(q: QuietHours, now: Date = new Date()): boolean {
  if (!q.enabled) return false;
  const a = minutes(q.from);
  const b = minutes(q.to);
  if (a === b) return false;
  const t = now.getHours() * 60 + now.getMinutes();
  return a < b ? t >= a && t < b : t >= a || t < b;
}

/** Milliseconds until the window next opens or closes (for re-arming the tray's notification switch). */
export function msUntilChange(q: QuietHours, now: Date = new Date()): number {
  const t = now.getHours() * 60 + now.getMinutes() + now.getSeconds() / 60;
  const edges = [minutes(q.from), minutes(q.to)];
  const waits = edges.map((e) => ((e - t) % 1440 + 1440) % 1440 || 1440);
  return Math.max(1000, Math.round(Math.min(...waits) * 60_000));
}
