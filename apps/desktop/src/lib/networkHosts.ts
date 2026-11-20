import type { NetworkConfig } from '@duochat/protocol';

export interface HostRow {
  host: string;
  /** What it is used for, in words. */
  why: string;
}

function hostOf(url: string): string {
  try {
    return new URL(url.replace(/^(stuns?|turns?):/i, 'https://')).host || url;
  } catch {
    return url.replace(/^(stuns?|turns?):/i, '').split('?')[0]!;
  }
}

/**
 * Every internet host this device may talk to, from the settings that decide it. The app has no analytics, no update
 * check and no other servers; chats between friends go device to device.
 */
export function networkHosts(n: NetworkConfig): HostRow[] {
  const rows: HostRow[] = [];
  const add = (host: string, why: string) => {
    if (host && !rows.some((r) => r.host === host && r.why === why)) rows.push({ host, why });
  };
  if (n.transportMode !== 'BLUETOOTH_ONLY') {
    for (const s of n.signaling) {
      if (s.kind === 'nostr') for (const r of s.relays) add(hostOf(r), 'Finds your friend and passes sealed hellos (Nostr relay)');
      else add(hostOf(s.url), 'Finds your friend and passes sealed messages (your own relay)');
    }
    if (!n.forceRelay) for (const u of n.stun) add(hostOf(u), 'Tells your device its public address so a direct connection can start (STUN)');
    for (const t of n.turn) for (const u of t.urls) add(hostOf(u), 'Carries encrypted traffic when a direct connection is not possible (TURN)');
  }
  return rows;
}
