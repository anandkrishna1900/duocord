import { describe, expect, it } from 'vitest';
import { filterTargets, moveIndex, sendTargets, snapshotTargets } from './quickSend';

describe('quick send targets', () => {
  const list = sendTargets({
    contacts: [
      { uid: 'b', name: 'Ben', direct: true },
      { uid: 'x', name: 'Introduced', direct: false },
      { uid: 'a', name: 'Ann Lee', direct: true },
    ],
    groups: [
      { groupId: 'g1', name: 'Trip', status: 'member' },
      { groupId: 'g2', name: 'Old', status: 'removed' },
    ],
  });
  it('keeps friends we can message and groups we are in, by name', () => {
    expect(list.map((t) => t.id)).toEqual(['a', 'b', 'g1']);
    expect(list[2]!.group).toBe(true);
  });
  it('filters on every word', () => {
    expect(filterTargets(list, '').length).toBe(3);
    expect(filterTargets(list, 'an le').map((t) => t.id)).toEqual(['a']);
    expect(filterTargets(list, 'zzz')).toEqual([]);
  });
  it('wraps the arrow keys', () => {
    expect(moveIndex(0, -1, 3)).toBe(2);
    expect(moveIndex(2, 1, 3)).toBe(0);
    expect(moveIndex(0, 1, 0)).toBe(-1);
  });
  it('reads the raw snapshot', () => {
    const t = snapshotTargets({
      own_uid: 'me',
      contacts: [{ uid: 'a', name: null, display_name: 'Ann', can_message: true }],
      groups: [{ id: 'g', name: 'Trip', members: [{ uid: 'me' }] }, { id: 'h', name: 'Gone', members: [] }],
    });
    expect(t.map((x) => x.name)).toEqual(['Ann', 'Trip']);
  });
});
