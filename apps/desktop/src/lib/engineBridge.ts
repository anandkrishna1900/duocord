/**
 * The page's side of the Rust engine hosted in the Tauri shell (milestone M7, docs/rust/M7_DESKTOP_PLAN.md).
 * Nothing in the app uses it yet: `engine.ts` still runs the app. It exists so the rewire can happen one command at a time.
 *
 * The page keeps owning WebRTC and the relay sockets. It reports what happened to the engine (`link_up`, `frame_received`,
 * `signal_blob`) and does what the engine asks (events `frame`, `close`, `signal-put`, ...), exactly as the Android adapter
 * does. Two lessons from Android are built in: frames reach the engine strictly in order, and a "Busy" answer (the engine's
 * queue is full) is retried instead of dropping the frame.
 */

/** The seam over the Tauri commands and events, so this is testable without a shell. */
export interface EngineBridge {
  start(): Promise<void>;
  stop(): Promise<void>;
  /** Emergency wipe: stop the engine and delete its database and identity, whether or not it is running. */
  erase?(): Promise<void>;
  /** Tell the engine whether the page's relay sockets are online (it routes through contacts when they are not). */
  signalStatus?(online: boolean): Promise<void>;
  invoke(command: string, args?: Record<string, unknown>): Promise<unknown>;
  /**
   * Send a file whose bytes go across as they are (no base64 text inside a JSON command: for a 50 MB file that would be a
   * 67 MB string). `details` names the person or group, the file's name and type. Only the desktop shell has this.
   */
  sendFileBytes?(details: FileSendDetails, bytes: Uint8Array): Promise<string>;
  /** A stored file's bytes as they are, or `null` if it is no longer stored. */
  loadFileBytes?(messageId: string): Promise<Uint8Array | null>;
  /** Subscribe to an engine-to-page event (`event`, `frame`, `close`, `signal-subscribe`, `signal-put`, `notify`, ...). */
  on(event: string, handler: (payload: unknown) => void): Promise<() => void>;
}

export interface FileSendDetails {
  peer?: string;
  group?: string;
  name: string;
  mime: string;
  reply_to?: string;
}

export type PathName = 'direct_lan' | 'direct_nat' | 'relayed' | 'bluetooth' | 'unknown';
export type LinkKindName = 'encrypted' | 'open' | 'layered';

const BUSY_RETRY_MS = 5;
const BUSY_GIVE_UP_TRIES = 4000; // about 20 s: the same patience as the Android adapter

const isBusy = (error: unknown) => String(error).includes('Busy');
const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

/** Typed access to the hosted engine. */
export class HostedEngine {
  /** Everything that flows INTO the engine from the links goes through this one chain, in order. */
  private inbound: Promise<void> = Promise.resolve();

  constructor(private readonly bridge: EngineBridge) {}

  start(): Promise<void> {
    return this.bridge.start();
  }

  stop(): Promise<void> {
    return this.bridge.stop();
  }

  signalStatus(online: boolean): Promise<void> {
    return this.bridge.signalStatus?.(online) ?? Promise.resolve();
  }

  /** Whether files can go to and from the engine as raw bytes (the desktop shell can; the plain-browser build cannot). */
  get rawFiles(): boolean {
    return typeof this.bridge.sendFileBytes === 'function' && typeof this.bridge.loadFileBytes === 'function';
  }

  sendFileBytes(details: FileSendDetails, bytes: Uint8Array): Promise<string> {
    const send = this.bridge.sendFileBytes;
    if (!send) return Promise.reject(new Error('raw files are not available'));
    return send.call(this.bridge, details, bytes);
  }

  loadFileBytes(messageId: string): Promise<Uint8Array | null> {
    return this.bridge.loadFileBytes?.(messageId) ?? Promise.resolve(null);
  }

  call<T = unknown>(command: string, args: Record<string, unknown> = {}): Promise<T> {
    return this.bridge.invoke(command, args) as Promise<T>;
  }

  /** Queue a call that must stay in order with the other inbound calls, retrying while the engine is busy. */
  private ordered(command: string, args: Record<string, unknown>): Promise<void> {
    const run = async () => {
      for (let tries = 0; ; tries += 1) {
        try {
          await this.bridge.invoke(command, args);
          return;
        } catch (error) {
          if (!isBusy(error) || tries >= BUSY_GIVE_UP_TRIES) throw error;
          await sleep(BUSY_RETRY_MS);
        }
      }
    };
    const next = this.inbound.then(run, run);
    // A failed call must not stop the ones behind it.
    this.inbound = next.catch(() => undefined);
    return next;
  }

  linkUp(peer: string, path: PathName, kind: LinkKindName): Promise<void> {
    return this.ordered('link_up', { peer, path, kind });
  }

  linkDown(peer: string): Promise<void> {
    return this.ordered('link_down', { peer });
  }

  frameReceived(peer: string, frame: string): Promise<void> {
    return this.ordered('frame_received', { peer, frame });
  }

  signalBlob(mailbox: string, blob: string): Promise<void> {
    return this.ordered('signal_blob', { mailbox, blob });
  }

  /** Everything the screens show; with `perChat`, only the newest messages of each chat (`older` counts the rest). */
  snapshot<T = unknown>(perChat?: number): Promise<T> {
    return perChat === undefined ? this.call<T>('snapshot') : this.call<T>('snapshot_recent', { per_chat: perChat });
  }

  /** Up to `limit` messages of a chat before `before` (the newest when empty), oldest first. */
  messagesBefore<T = unknown>(conversation: string, before: string | null, limit: number): Promise<T> {
    return this.call<T>('messages_before', { conversation, before, limit });
  }

  sendMessage(peer: string, text: string, replyTo?: string): Promise<string> {
    return this.call<string>('send_message', { peer, text, reply_to: replyTo });
  }

  on(event: string, handler: (payload: unknown) => void): Promise<() => void> {
    return this.bridge.on(event, handler);
  }
}

function toHex(bytes: Uint8Array): string {
  let out = '';
  for (const b of bytes) out += b.toString(16).padStart(2, '0');
  return out;
}

/** The real bridge: Tauri commands and events. Loaded lazily so unit tests never need the Tauri runtime. */
export async function tauriEngineBridge(): Promise<EngineBridge> {
  const { invoke } = await import('@tauri-apps/api/core');
  const { listen } = await import('@tauri-apps/api/event');
  return {
    start: () => invoke('engine_start'),
    stop: () => invoke('engine_stop'),
    erase: () => invoke('engine_erase'),
    signalStatus: (online) => invoke('engine_signal_status', { online }),
    invoke: (command, args = {}) => invoke('engine_invoke', { command, args }),
    sendFileBytes: (details, bytes) =>
      invoke<string>('engine_send_file', bytes, { headers: { 'x-privcord-file': toHex(new TextEncoder().encode(JSON.stringify(details))) } }),
    loadFileBytes: async (messageId) => {
      try {
        return new Uint8Array(await invoke<ArrayBuffer>('engine_load_file', { messageId }));
      } catch {
        return null;
      }
    },
    on: async (event, handler) => listen(`privcord://${event}`, (e) => handler(e.payload)),
  };
}
