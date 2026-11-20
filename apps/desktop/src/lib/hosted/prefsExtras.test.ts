import { describe, expect, it } from 'vitest';
import { shapeHops, shapePrefs, shapeReceipts } from './hostedEngine';

describe('silent chats, receipts and hops from the engine', () => {
  it('reads silent_chats (and defaults to none)', () => {
    expect(shapePrefs({ silent_chats: ['a', 3, 'b'] })?.silentChats).toEqual(['a', 'b']);
    expect(shapePrefs({})?.silentChats).toEqual([]);
  });
  it('shapes receipts and drops malformed rows', () => {
    const out = shapeReceipts([
      { device_id: 'd1', device_name: 'Laptop', state: 'read', at: 5 },
      { device_id: 'd2', device_name: null, state: 'waiting', at: 0 },
      { device_id: 'd3', state: 'weird', at: 1 },
      null,
    ]);
    expect(out).toEqual([
      { deviceId: 'd1', deviceName: 'Laptop', state: 'read', at: 5 },
      { deviceId: 'd2', deviceName: null, state: 'waiting', at: 0 },
    ]);
    expect(shapeReceipts(null)).toEqual([]);
  });
  it('shapes hops', () => {
    expect(shapeHops({ a: 2, b: -1, c: 'x', d: 1.7 })).toEqual({ a: 2, d: 1 });
    expect(shapeHops(undefined)).toEqual({});
  });
});
