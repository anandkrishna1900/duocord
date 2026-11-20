export const MAX_PINS = 3;

/** Pinned chats first (in the order they were pinned), then everything else as given. */
export function withPinsFirst<T extends { id: string }>(rows: T[], pinned: string[]): T[] {
  const pins = pinned
    .map((id) => rows.find((r) => r.id === id))
    .filter((r): r is T => r !== undefined);
  const rest = rows.filter((r) => !pinned.includes(r.id));
  return [...pins, ...rest];
}

/** Pin or unpin: at most MAX_PINS, the oldest pin drops off when a fourth is added. */
export function togglePin(pinned: string[], id: string): string[] {
  if (pinned.includes(id)) return pinned.filter((p) => p !== id);
  return [...pinned, id].slice(-MAX_PINS);
}
