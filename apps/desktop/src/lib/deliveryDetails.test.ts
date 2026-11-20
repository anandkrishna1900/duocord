import { describe, expect, it } from 'vitest';
import { deliveryRows, stateWords } from './deliveryDetails';

describe('delivery details', () => {
  it('names each recipient and puts the state in words', () => {
    const rows = deliveryRows({ b: 'delivered' as never, a: 'read' as never }, 'delivered' as never, (u) => (u === 'a' ? 'Ann' : 'Ben'));
    expect(rows).toEqual([{ name: 'Ann', status: 'Read' }, { name: 'Ben', status: 'Delivered' }]);
    expect(deliveryRows(undefined, 'queued' as never, () => '?')[0]!.status).toBe('Waiting for them to come online');
    expect(stateWords('mystery')).toBe('mystery');
  });
});
