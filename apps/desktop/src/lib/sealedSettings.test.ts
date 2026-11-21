import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a: unknown[]) => invoke(...a) }));

async function fresh() {
  vi.resetModules();
  return import('./sealedSettings');
}

const store = new Map<string, string>();
const localStorageStub = {
  getItem: (k: string) => store.get(k) ?? null,
  setItem: (k: string, v: string) => void store.set(k, v),
  removeItem: (k: string) => void store.delete(k),
  get length() {
    return store.size;
  },
};
/** The page as the shell runs it (it has the shell's bridge) or as a plain browser has it. */
function page(inShell: boolean) {
  vi.stubGlobal('window', inShell ? { __TAURI_INTERNALS__: {} } : {});
}

describe('settings kept encrypted by the shell', () => {
  beforeEach(() => {
    invoke.mockReset();
    store.clear();
    vi.stubGlobal('localStorage', localStorageStub);
    page(false);
  });
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it('stays in local storage in a plain browser', async () => {
    const s = await fresh();
    await s.loadSealedSettings();
    s.setSealed('settings', '{"a":1}');
    expect(s.getSealed('settings')).toBe('{"a":1}');
    expect(localStorage.getItem('duochat.settings.v1')).toBe('{"a":1}');
  });

  it('in the shell reads and writes only through the encrypted file, never local storage', async () => {
    page(true);
    invoke.mockImplementation(async (cmd: string, args: { name: string }) => (cmd === 'settings_get' ? `saved-${args.name}` : undefined));
    const s = await fresh();
    await s.loadSealedSettings();
    expect(s.getSealed('network')).toBe('saved-network');
    s.setSealed('network', '{"turn":"hunter2"}');
    expect(s.getSealed('network')).toBe('{"turn":"hunter2"}');
    await new Promise((r) => setTimeout(r, 0)); // the save goes out asynchronously
    expect(invoke).toHaveBeenCalledWith('settings_set', { name: 'network', text: '{"turn":"hunter2"}' });
    expect(localStorage.length).toBe(0);
  });

  it('moves a plain-text copy from an older version into the encrypted file and deletes it', async () => {
    page(true);
    localStorage.setItem('duochat.network.v1', '{"turn":"old-password"}');
    invoke.mockImplementation(async (cmd: string) => (cmd === 'settings_get' ? null : undefined));
    const s = await fresh();
    await s.loadSealedSettings();
    expect(invoke).toHaveBeenCalledWith('settings_set', { name: 'network', text: '{"turn":"old-password"}' });
    expect(localStorage.getItem('duochat.network.v1')).toBeNull();
    expect(s.getSealed('network')).toBe('{"turn":"old-password"}');
  });

  it('keeps the old copy usable if it could not be encrypted, and never invents one', async () => {
    page(true);
    localStorage.setItem('duochat.settings.v1', '{"x":1}');
    invoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'settings_get') return null;
      throw new Error('no credential store');
    });
    const s = await fresh();
    await s.loadSealedSettings();
    expect(s.getSealed('settings')).toBe('{"x":1}');
    expect(s.getSealed('network')).toBeNull();
  });
});
