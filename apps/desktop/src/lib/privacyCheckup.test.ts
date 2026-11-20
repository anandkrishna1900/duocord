import { describe, expect, it } from 'vitest';
import { privacyCheckup } from './privacyCheckup';
import { MAX_PINS, togglePin, withPinsFirst } from './pins';
import { networkHosts } from './networkHosts';
import { DEFAULT_NETWORK_CONFIG } from '@duochat/protocol';

const base = { appLock: false, lastBackupAt: 0, hideIp: false, previewsOff: false, groupOnly: false, panicWipe: false, staleDevices: 0, now: 1_000_000_000_000 };

describe('privacy checkup', () => {
  it('scores what is on and says what to fix', () => {
    const none = privacyCheckup(base);
    expect(none.score).toBe(1); // only "no forgotten devices" holds
    expect(none.items.find((i) => i.id === 'lock')?.section).toBe('security');
    const all = privacyCheckup({ ...base, appLock: true, lastBackupAt: base.now - 1000, hideIp: true, previewsOff: true, groupOnly: true, panicWipe: true });
    expect(all.score).toBe(all.total);
  });
  it('treats an old backup as no backup, and names stale devices', () => {
    expect(privacyCheckup({ ...base, lastBackupAt: base.now - 60 * 24 * 3600 * 1000 }).items.find((i) => i.id === 'backup')?.ok).toBe(false);
    expect(privacyCheckup({ ...base, staleDevices: 2 }).items.find((i) => i.id === 'devices')?.hint).toContain('2 linked devices');
  });
});

describe('pins', () => {
  const rows = ['a', 'b', 'c', 'd'].map((id) => ({ id }));
  it('puts pinned chats first in pin order', () => {
    expect(withPinsFirst(rows, ['c', 'a']).map((r) => r.id)).toEqual(['c', 'a', 'b', 'd']);
    expect(withPinsFirst(rows, ['gone']).map((r) => r.id)).toEqual(['a', 'b', 'c', 'd']);
  });
  it('keeps at most three, dropping the oldest', () => {
    let p: string[] = [];
    for (const id of ['a', 'b', 'c', 'd']) p = togglePin(p, id);
    expect(p).toEqual(['b', 'c', 'd']);
    expect(p.length).toBe(MAX_PINS);
    expect(togglePin(p, 'c')).toEqual(['b', 'd']);
  });
});

describe('network hosts', () => {
  it('lists relays and STUN for the default config, nothing in Bluetooth-only mode', () => {
    const hosts = networkHosts(DEFAULT_NETWORK_CONFIG).map((h) => h.host);
    expect(hosts).toContain('relay.damus.io');
    expect(hosts.length).toBeGreaterThan(3);
    expect(networkHosts({ ...DEFAULT_NETWORK_CONFIG, transportMode: 'BLUETOOTH_ONLY' })).toEqual([]);
  });
  it('drops STUN when everything goes through a relay', () => {
    const rows = networkHosts({ ...DEFAULT_NETWORK_CONFIG, forceRelay: true, turn: [{ urls: ['turn:turn.example.org:3478'], username: 'u', credential: 'c' }] });
    expect(rows.some((r) => r.why.includes('STUN'))).toBe(false);
    expect(rows.find((r) => r.why.includes('TURN'))?.host).toBe('turn.example.org:3478');
  });
});
