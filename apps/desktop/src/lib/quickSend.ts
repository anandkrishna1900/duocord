/** Who a quick message or a dropped file can go to: friends we can message and groups we are still in. */
export interface SendTarget {
  id: string;
  name: string;
  group: boolean;
}

interface TargetSource {
  contacts: Array<{ uid: string; name: string; direct: boolean }>;
  groups: Array<{ groupId: string; name: string; status: string }>;
}

export function sendTargets(s: TargetSource): SendTarget[] {
  return [
    ...s.contacts.filter((c) => c.direct).map((c) => ({ id: c.uid, name: c.name, group: false })),
    ...s.groups.filter((g) => g.status === 'member').map((g) => ({ id: g.groupId, name: g.name, group: true })),
  ].sort((a, b) => a.name.localeCompare(b.name));
}

/** The same list from the engine's raw snapshot (the popup has no ChatState). */
export function snapshotTargets(s: {
  own_uid: string | null;
  contacts: Array<{ uid: string; name: string | null; display_name: string; can_message: boolean }>;
  groups: Array<{ id: string; name: string; members: Array<{ uid: string }> }>;
}): SendTarget[] {
  return sendTargets({
    contacts: s.contacts.map((c) => ({ uid: c.uid, name: c.name || c.display_name, direct: c.can_message })),
    groups: s.groups.map((g) => ({ groupId: g.id, name: g.name, status: g.members.some((m) => m.uid === s.own_uid) ? 'member' : 'removed' })),
  });
}

/** Names containing every word typed (case-insensitive); an empty query keeps all. */
export function filterTargets(list: SendTarget[], query: string): SendTarget[] {
  const words = query.toLowerCase().split(/\s+/).filter(Boolean);
  return words.length === 0 ? list : list.filter((t) => words.every((w) => t.name.toLowerCase().includes(w)));
}

/** Arrow-key movement in a list of `length` rows, wrapping round; -1 when the list is empty. */
export function moveIndex(index: number, delta: number, length: number): number {
  if (length <= 0) return -1;
  return (((index + delta) % length) + length) % length;
}
