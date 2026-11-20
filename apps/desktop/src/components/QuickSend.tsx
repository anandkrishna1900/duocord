import { useEffect, useMemo, useRef, useState, type KeyboardEvent } from 'react';
import { isLockEnabled } from '../lib/appLock';
import { HostedEngine, tauriEngineBridge } from '../lib/engineBridge';
import { filterTargets, moveIndex, snapshotTargets, type SendTarget } from '../lib/quickSend';
import type { SnapshotJson } from '../lib/hosted/shape';

async function closeWindow(): Promise<void> {
  try {
    const { getCurrentWindow } = await import('@tauri-apps/api/window');
    await getCurrentWindow().close();
  } catch {
    window.close();
  }
}

async function openMain(): Promise<void> {
  try {
    const { invoke } = await import('@tauri-apps/api/core');
    await invoke('open_main');
  } catch {
    void closeWindow();
  }
}

type Phase = 'loading' | 'ready' | 'closed' | 'locked';

/**
 * The Ctrl+Alt+D popup (its own small window, `?quick=1`). It does not start an engine: the engine runs in the Tauri
 * host, so this only asks it for the friend list and sends through the same commands the main window uses.
 */
export function QuickSend() {
  const [phase, setPhase] = useState<Phase>('loading');
  const [engine, setEngine] = useState<HostedEngine | null>(null);
  const [targets, setTargets] = useState<SendTarget[]>([]);
  const [query, setQuery] = useState('');
  const [index, setIndex] = useState(0);
  const [chosen, setChosen] = useState<SendTarget | null>(null);
  const [text, setText] = useState('');
  const [status, setStatus] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const messageRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    // The popup would otherwise get round the app lock: with a lock set, the person opens the app and unlocks it there.
    if (isLockEnabled()) {
      setPhase('locked');
      return;
    }
    let cancelled = false;
    void (async () => {
      try {
        const host = new HostedEngine(await tauriEngineBridge());
        const snapshot = await host.snapshot<SnapshotJson>();
        if (cancelled) return;
        setEngine(host);
        setTargets(snapshotTargets(snapshot));
        setPhase('ready');
      } catch {
        if (!cancelled) setPhase('closed');
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    const onKey = (event: globalThis.KeyboardEvent) => {
      if (event.key === 'Escape') void closeWindow();
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, []);

  useEffect(() => {
    if (chosen) messageRef.current?.focus();
  }, [chosen]);

  const shown = useMemo(() => filterTargets(targets, query), [targets, query]);
  const active = Math.min(index, Math.max(0, shown.length - 1));

  function onPickKey(event: KeyboardEvent<HTMLInputElement>) {
    if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
      event.preventDefault();
      setIndex(moveIndex(active, event.key === 'ArrowDown' ? 1 : -1, shown.length));
    } else if (event.key === 'Enter' && shown[active]) {
      event.preventDefault();
      setChosen(shown[active]!);
    }
  }

  async function send() {
    const body = text.trim();
    if (!engine || !chosen || !body || busy) return;
    setBusy(true);
    setStatus(null);
    try {
      if (chosen.group) await engine.call('send_group_message', { group: chosen.id, text: body });
      else await engine.sendMessage(chosen.id, body);
      setStatus('Sent');
      window.setTimeout(() => void closeWindow(), 600);
    } catch {
      setStatus('Could not send. Open DuoCord to see why.');
      setBusy(false);
    }
  }

  return (
    <main className="quick-send" aria-label="Quick send">
      {phase === 'loading' ? <p className="muted small">Opening…</p> : null}
      {phase === 'closed' || phase === 'locked' ? (
        <p className="quick-note">{phase === 'locked' ? 'DuoCord is locked. Open it and unlock it first.' : 'Open DuoCord first.'}</p>
      ) : null}
      {phase === 'ready' && !chosen ? (
        <>
          <input
            className="quick-input"
            autoFocus
            type="search"
            placeholder="Send to… (a friend or a group)"
            aria-label="Search friends and groups"
            value={query}
            onChange={(event) => {
              setQuery(event.target.value);
              setIndex(0);
            }}
            onKeyDown={onPickKey}
          />
          <ul className="quick-list" role="listbox" aria-label="Friends and groups">
            {shown.length === 0 ? <li className="muted small quick-empty">{targets.length === 0 ? 'No friends yet.' : 'Nobody found.'}</li> : null}
            {shown.map((t, i) => (
              <li key={t.id} role="option" aria-selected={i === active}>
                <button type="button" className={`quick-row ${i === active ? 'active' : ''}`} onMouseEnter={() => setIndex(i)} onClick={() => setChosen(t)}>
                  {t.name}
                  {t.group ? <span className="muted small"> group</span> : null}
                </button>
              </li>
            ))}
          </ul>
        </>
      ) : null}
      {phase === 'ready' && chosen ? (
        <>
          <button type="button" className="quick-chip" onClick={() => setChosen(null)} aria-label={`Sending to ${chosen.name}, change`}>
            To: <strong>{chosen.name}</strong>
          </button>
          <input
            ref={messageRef}
            className="quick-input"
            placeholder="Message"
            aria-label="Message"
            value={text}
            disabled={busy}
            onChange={(event) => setText(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === 'Enter') {
                event.preventDefault();
                void send();
              }
            }}
          />
          <p className="muted small" role="status">
            {status ?? 'Enter to send, Esc to close.'}
          </p>
        </>
      ) : null}
      <button type="button" className="button ghost small quick-open" onClick={() => void openMain()}>
        Open DuoCord
      </button>
    </main>
  );
}
