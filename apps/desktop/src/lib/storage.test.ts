import { describe, expect, it } from 'vitest';
import { storageByChat } from './storage';

describe('storage by chat', () => {
  it('adds text and files per chat, biggest first, skipping deleted messages and empty chats', () => {
    const rows = storageByChat(
      {
        a: [{ messageId: '1', text: 'hello' }, { messageId: '2', text: '', file: { size: 1000 } }, { messageId: '3', text: 'gone', deleted: true }],
        b: [{ messageId: '4', text: 'x'.repeat(5000) }],
        c: [{ messageId: '5', text: 'bye', deleted: true }],
      },
      (id) => id.toUpperCase(),
    );
    expect(rows.map((r) => r.name)).toEqual(['B', 'A']);
    expect(rows[1]).toMatchObject({ messages: 2, files: 1, fileBytes: 1000, bytes: 1005, fileIds: ['2'] });
  });
});
