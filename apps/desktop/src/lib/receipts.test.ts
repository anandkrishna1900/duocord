import { describe, expect, it } from 'vitest';
import { nearbyText, receiptLine } from './receipts';

const t = () => '12:03';
describe('receipts', () => {
  it('words each device line', () => {
    expect(receiptLine({ deviceId: 'a', deviceName: 'Laptop', state: 'read', at: 1 }, t)).toBe('Laptop: read 12:03');
    expect(receiptLine({ deviceId: 'a', deviceName: 'Phone', state: 'delivered', at: 1 }, t)).toBe('Phone: delivered');
    expect(receiptLine({ deviceId: 'a', deviceName: 'Tablet', state: 'waiting', at: 0 }, t)).toBe('Tablet: waiting');
    expect(receiptLine({ deviceId: 'abcdef', deviceName: null, state: 'waiting', at: 0 }, t)).toBe('Device abcd: waiting');
  });
  it('words the hops', () => {
    expect(nearbyText('Sam', undefined)).toContain('directly to Sam');
    expect(nearbyText('Sam', 1)).toContain('through 1 nearby device to');
    expect(nearbyText('Sam', 2)).toContain('through 2 nearby devices to');
  });
});
