import { beforeEach, describe, expect, it, vi } from 'vitest';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a: unknown[]) => invoke(...a) }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn() }));

describe('files go to and from the shell as raw bytes', () => {
  beforeEach(() => invoke.mockReset());

  it('sends the bytes themselves, with the details in a hex-encoded header', async () => {
    invoke.mockResolvedValue('msg-1');
    const { tauriEngineBridge, HostedEngine } = await import('./engineBridge');
    const bridge = await tauriEngineBridge();
    const engine = new HostedEngine(bridge);
    expect(engine.rawFiles).toBe(true);
    const bytes = new Uint8Array([1, 2, 3, 250]);
    const id = await engine.sendFileBytes({ peer: 'abc', name: 'söng.mp3', mime: 'audio/mpeg' }, bytes);
    expect(id).toBe('msg-1');
    const [command, body, options] = invoke.mock.calls[0] as [string, Uint8Array, { headers: Record<string, string> }];
    expect(command).toBe('engine_send_file');
    expect(body).toBe(bytes); // not a copy, not text
    const hex = options.headers['x-privcord-file'] as string;
    expect(hex).toMatch(/^[0-9a-f]+$/);
    const decoded = new TextDecoder().decode(new Uint8Array(hex.match(/../g)!.map((h) => parseInt(h, 16))));
    expect(JSON.parse(decoded)).toEqual({ peer: 'abc', name: 'söng.mp3', mime: 'audio/mpeg' });
  });

  it('gets a stored file back as bytes, or null when it is gone', async () => {
    const { tauriEngineBridge } = await import('./engineBridge');
    const bridge = await tauriEngineBridge();
    invoke.mockResolvedValueOnce(new Uint8Array([9, 8, 7]).buffer);
    expect(Array.from((await bridge.loadFileBytes!('m1'))!)).toEqual([9, 8, 7]);
    expect(invoke).toHaveBeenCalledWith('engine_load_file', { messageId: 'm1' });
    invoke.mockRejectedValueOnce(new Error('that file is no longer stored'));
    expect(await bridge.loadFileBytes!('m2')).toBeNull();
  });

  it('a bridge without the raw calls (the browser build) is not treated as having them', async () => {
    const { HostedEngine } = await import('./engineBridge');
    const engine = new HostedEngine({ start: async () => {}, stop: async () => {}, invoke: async () => null, on: async () => () => {} });
    expect(engine.rawFiles).toBe(false);
  });
});
