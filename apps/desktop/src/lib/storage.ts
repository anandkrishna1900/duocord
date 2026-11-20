export interface ChatStorage {
  id: string;
  name: string;
  messages: number;
  files: number;
  /** Text bytes plus file bytes. */
  bytes: number;
  fileBytes: number;
  fileIds: string[];
}

interface Msg {
  messageId: string;
  text: string;
  deleted?: boolean;
  file?: { size: number };
}

/** What each chat takes on this device, biggest first. Counts what is held: tombstones of deleted messages are free. */
export function storageByChat(messages: Record<string, Msg[]>, nameOf: (id: string) => string): ChatStorage[] {
  const enc = new TextEncoder();
  return Object.entries(messages)
    .map(([id, list]) => {
      let bytes = 0;
      let fileBytes = 0;
      const fileIds: string[] = [];
      let count = 0;
      for (const m of list) {
        if (m.deleted) continue;
        count += 1;
        bytes += enc.encode(m.text ?? '').length;
        if (m.file) {
          fileBytes += m.file.size;
          fileIds.push(m.messageId);
        }
      }
      return { id, name: nameOf(id), messages: count, files: fileIds.length, bytes: bytes + fileBytes, fileBytes, fileIds };
    })
    .filter((c) => c.messages > 0)
    .sort((a, b) => b.bytes - a.bytes);
}
