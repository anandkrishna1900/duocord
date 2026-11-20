import type { DeliveryState } from '@duochat/protocol';

export interface DeliveryRow {
  name: string;
  /** In words: "Read", "Delivered", "On its way", "Waiting for them to come online", "Could not be sent". */
  status: string;
}

const WORDS: Record<string, string> = {
  queued: 'Waiting for them to come online',
  sending: 'On its way',
  sent: 'Sent',
  delivered: 'Delivered',
  read: 'Read',
  failed: 'Could not be sent',
};

export function stateWords(s: DeliveryState | string): string {
  return WORDS[String(s).toLowerCase()] ?? String(s);
}

/** One row per recipient (a direct chat has one). `names` maps a uid to what we call them. */
export function deliveryRows(recipients: Record<string, DeliveryState> | undefined, fallback: DeliveryState, names: (uid: string) => string): DeliveryRow[] {
  const entries = Object.entries(recipients ?? {});
  if (entries.length === 0) return [{ name: 'Them', status: stateWords(fallback) }];
  return entries.map(([uid, s]) => ({ name: names(uid), status: stateWords(s) })).sort((a, b) => a.name.localeCompare(b.name));
}
