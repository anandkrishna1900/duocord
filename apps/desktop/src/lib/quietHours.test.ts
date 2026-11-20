import { describe, expect, it } from 'vitest';
import { inQuietHours, msUntilChange, parseQuiet } from './quietHours';

const at = (h: number, m = 0) => new Date(2026, 0, 5, h, m, 0);
const night = { enabled: true, from: '22:00', to: '07:00' };

describe('quiet hours', () => {
  it('covers a window that crosses midnight', () => {
    expect(inQuietHours(night, at(23))).toBe(true);
    expect(inQuietHours(night, at(3))).toBe(true);
    expect(inQuietHours(night, at(7))).toBe(false);
    expect(inQuietHours(night, at(12))).toBe(false);
    expect(inQuietHours(night, at(22))).toBe(true);
  });
  it('covers a same-day window and is off when disabled or empty', () => {
    expect(inQuietHours({ enabled: true, from: '13:00', to: '14:00' }, at(13, 30))).toBe(true);
    expect(inQuietHours({ enabled: true, from: '13:00', to: '14:00' }, at(14))).toBe(false);
    expect(inQuietHours({ ...night, enabled: false }, at(23))).toBe(false);
    expect(inQuietHours({ enabled: true, from: '08:00', to: '08:00' }, at(8))).toBe(false);
  });
  it('says when the window next changes', () => {
    expect(msUntilChange(night, at(21, 30))).toBe(30 * 60_000);
    expect(msUntilChange(night, at(6, 0))).toBe(60 * 60_000);
  });
  it('repairs a bad saved value', () => {
    expect(parseQuiet({ enabled: true, from: '25:99', to: 'x' })).toEqual({ enabled: true, from: '22:00', to: '07:00' });
    expect(parseQuiet(undefined).enabled).toBe(false);
  });
});

import { deliveryRows, stateWords } from './deliveryDetails';

describe('delivery details', () => {
  it('names each recipient and puts the state in words', () => {
    const rows = deliveryRows({ b: 'delivered' as never, a: 'read' as never }, 'delivered' as never, (u) => (u === 'a' ? 'Ann' : 'Ben'));
    expect(rows).toEqual([{ name: 'Ann', status: 'Read' }, { name: 'Ben', status: 'Delivered' }]);
    expect(deliveryRows(undefined, 'queued' as never, () => '?')[0]!.status).toBe('Waiting for them to come online');
    expect(stateWords('mystery')).toBe('mystery');
  });
});
