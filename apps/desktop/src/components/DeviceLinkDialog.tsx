import { useEffect, useState } from 'react';
import type { ChatState } from '../lib/engine';
import type { ChatEngineApi } from '../lib/engineApi';
import { Modal } from './Modal';
import { QrCode } from './QrCode';
import { AlertIcon, CheckIcon } from './Icons';

/**
 * Linking another device of the account (docs/design/syncengine.md §2).
 *
 * `host`: on the device holding the account key. It shows a code (QR and text) for the new device, then the 6 digits
 * both screens show; the person confirms here only if they match.
 * `join`: on the new device. The person types the code from the other device, compares the 6 digits, and waits for
 * the other device to confirm. The account key never leaves the other device.
 */
export function DeviceLinkDialog({
  open,
  mode,
  engine,
  state,
  onClose,
  onLinked,
}: {
  open: boolean;
  mode: 'host' | 'join';
  engine: ChatEngineApi | null;
  state: ChatState | null;
  onClose: () => void;
  /** The new device is now part of the account (join mode). */
  onLinked?: () => void;
}) {
  const [code, setCode] = useState<string | null>(null);
  const [typed, setTyped] = useState('');
  const [name, setName] = useState(defaultName());
  const [joined, setJoined] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const link = state?.deviceLink ?? null;

  useEffect(() => {
    if (!open) return;
    setCode(null);
    setTyped('');
    setJoined(false);
    setError(null);
    if (mode === 'host' && engine?.startDeviceLink) {
      engine.startDeviceLink().then(setCode, (e: unknown) => setError(message(e)));
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open, mode]);

  useEffect(() => {
    if (open && mode === 'join' && link?.stage === 'done') onLinked?.();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open, mode, link?.stage]);

  const close = () => {
    if (link?.stage !== 'done') engine?.cancelDeviceLink?.();
    onClose();
  };

  const join = async () => {
    if (!engine?.joinDeviceLink) return;
    setBusy(true);
    setError(null);
    try {
      await engine.joinDeviceLink(typed, name);
      setJoined(true);
    } catch (e) {
      setError(message(e));
    } finally {
      setBusy(false);
    }
  };

  const confirm = async (accept: boolean) => {
    if (!engine?.confirmDeviceLink) return;
    setBusy(true);
    try {
      await engine.confirmDeviceLink(accept);
      if (!accept) onClose();
    } catch (e) {
      setError(message(e));
    } finally {
      setBusy(false);
    }
  };

  const ended = link && link.stage !== 'code' ? link.stage : null;

  return (
    <Modal open={open} onClose={close} title={mode === 'host' ? 'Link a new device' : 'Link to your other device'}>
      {error ? (
        <p className="status-text error" role="alert">
          <AlertIcon size={15} /> {error}
        </p>
      ) : null}

      {ended === 'done' ? (
        <p className="status-text">
          <CheckIcon size={15} /> {mode === 'host' ? `${link?.name ?? 'The new device'} is linked.` : 'Linked. Your chats are on their way.'}
        </p>
      ) : ended ? (
        <p className="status-text error" role="alert">
          {ended === 'expired' ? 'The code expired. Start again.' : ended === 'cancelled' ? 'Linking was cancelled.' : 'Linking did not work. Start again.'}
        </p>
      ) : link?.stage === 'code' && link.code ? (
        <div className="link-compare">
          <p>{mode === 'host' ? `${link.name ?? 'A device'} wants to join. Does it show these digits?` : 'Check that your other device shows the same digits, then confirm there.'}</p>
          <p className="link-digits" aria-label="Six digit code">
            {link.code.slice(0, 3)} {link.code.slice(3)}
          </p>
          {mode === 'host' ? (
            <div className="button-row">
              <button type="button" className="button primary" disabled={busy} onClick={() => void confirm(true)}>
                <CheckIcon size={15} /> They match, link it
              </button>
              <button type="button" className="button" disabled={busy} onClick={() => void confirm(false)}>
                They don't match
              </button>
            </div>
          ) : (
            <p className="muted small">If they differ, cancel: someone else may have your code.</p>
          )}
        </div>
      ) : mode === 'host' ? (
        code ? (
          <div className="link-offer">
            <QrCode value={code} label="Code for the new device" />
            <p>On the new device, choose "Link to a device you already use" and scan or type this code:</p>
            <p className="link-code">{code}</p>
            <p className="muted small">It works once, for ten minutes. Your account key stays on this device.</p>
          </div>
        ) : !error ? (
          <p className="muted">Making a code…</p>
        ) : null
      ) : joined ? (
        <p className="muted">Waiting for your other device…</p>
      ) : (
        <div className="link-join">
          <label className="field">
            <span>Code from your other device</span>
            <input value={typed} onChange={(e) => setTyped(e.target.value)} placeholder="Settings › Devices › Link a new device" />
          </label>
          <label className="field">
            <span>Name for this device</span>
            <input value={name} maxLength={64} onChange={(e) => setName(e.target.value)} />
          </label>
          <div className="button-row">
            <button type="button" className="button primary" disabled={busy || typed.trim().length < 10} onClick={() => void join()}>
              Link
            </button>
          </div>
        </div>
      )}
    </Modal>
  );
}

function defaultName(): string {
  const p = typeof navigator === 'undefined' ? '' : navigator.userAgent;
  if (/Windows/i.test(p)) return 'Windows PC';
  if (/Mac/i.test(p)) return 'Mac';
  if (/Linux/i.test(p)) return 'Linux PC';
  return 'Computer';
}

function message(e: unknown): string {
  return e instanceof Error ? e.message : String(e);
}
