/** Pinned chats first (in the order they were pinned), then everything else as given. The engine holds the pins. */
export function withPinsFirst<T extends { id: string }>(rows: T[], pinned: string[]): T[] {
  const pins = pinned.map((id) => rows.find((r) => r.id === id)).filter((r): r is T => r !== undefined);
  const rest = rows.filter((r) => !pinned.includes(r.id));
  return [...pins, ...rest];
}
