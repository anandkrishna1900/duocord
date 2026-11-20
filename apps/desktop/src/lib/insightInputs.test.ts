import { describe, expect, it } from 'vitest';
import { activeMutes, engineSignaling, quietNow, titleFor, totalUnread } from './insightInputs';

describe('insightInputs', () => {
  it('maps signaling', () => {
    expect(engineSignaling('idle')).toBe('unconfigured');
    expect(engineSignaling('error')).toBe('offline');
    expect(engineSignaling('online')).toBe('online');
    expect(engineSignaling('connecting')).toBe('connecting');
  });
  it('quiet hours, also across midnight', () => {
    const at = (h: number, m = 0) => new Date(2026, 0, 1, h, m);
    const q = { enabled: true, from: 22 * 60, to: 7 * 60 };
    expect(quietNow(q, at(23))).toBe(true);
    expect(quietNow(q, at(6, 59))).toBe(true);
    expect(quietNow(q, at(7))).toBe(false);
    expect(quietNow({ enabled: true, from: 60, to: 120 }, at(1, 30))).toBe(true);
    expect(quietNow({ ...q, enabled: false }, at(23))).toBe(false);
  });
  it('counts live mutes and unread', () => {
    expect(activeMutes({ a: 10, b: 200, c: 300 }, 100)).toBe(2);
    expect(totalUnread({ contacts: [{ unread: 2 }], groups: [{ unread: 1 }], channels: [{ unread: 4 }] } as never)).toBe(7);
  });
  it('titles', () => {
    expect(titleFor(0)).toBe('DuoCord');
    expect(titleFor(3)).toBe('(3) DuoCord');
    expect(titleFor(250)).toBe('(99+) DuoCord');
  });
});
