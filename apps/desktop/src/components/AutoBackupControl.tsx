import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';

interface AutoBackupStatus {
  folder: string | null;
  lastAt: number;
  error: string | null;
  defaultFolder: string | null;
}

/**
 * Weekly automatic backups into a folder (desktop app only). The passphrase is kept in the system credential store so
 * backups are made with nobody there; the three newest files are kept.
 */
export function AutoBackupControl({ pass }: { pass: string }) {
  const [status, setStatus] = useState<AutoBackupStatus | null>(null);
  const [folder, setFolder] = useState('');
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<{ text: string; error: boolean } | null>(null);

  useEffect(() => {
    void invoke<AutoBackupStatus>('auto_backup_status').then((s) => {
      setStatus(s);
      setFolder(s.folder ?? s.defaultFolder ?? '');
    });
  }, []);

  async function act(command: string, args: Record<string, unknown>, done: string) {
    setBusy(true);
    setMessage(null);
    try {
      setStatus(await invoke<AutoBackupStatus>(command, args));
      setMessage({ text: done, error: false });
    } catch (caught) {
      setMessage({ text: typeof caught === 'string' ? caught : caught instanceof Error ? caught.message : 'That did not work.', error: true });
    } finally {
      setBusy(false);
    }
  }

  if (!status) return null;
  const on = !!status.folder;
  return (
    <div className="auto-backup">
      <strong className="small">Automatic backups</strong>
      {on ? (
        <>
          <p className="muted small">
            Every week into {status.folder}. The three newest are kept.
            {status.lastAt ? ` Last one ${new Date(status.lastAt).toLocaleString()}.` : ''}
          </p>
          {status.error ? (
            <p className="status-text error" role="alert">
              The last backup failed: {status.error}
            </p>
          ) : null}
          <div className="button-row">
            <button type="button" className="button secondary small" disabled={busy} onClick={() => void act('auto_backup_now', {}, 'Backup saved.')}>
              Back up now
            </button>
            <button type="button" className="button ghost small" disabled={busy} onClick={() => void act('auto_backup_disable', {}, 'Automatic backups are off.')}>
              Turn off
            </button>
          </div>
        </>
      ) : (
        <>
          <p className="muted small">
            A backup every week, locked with the passphrase above, into a folder of your choice (a synced folder keeps a
            copy off this computer). The passphrase is kept in this computer&apos;s credential store so backups happen on
            their own.
          </p>
          <label className="field">
            <span className="small">Folder</span>
            <input type="text" value={folder} spellCheck={false} onChange={(e) => setFolder(e.target.value)} />
          </label>
          <div className="button-row">
            <button
              type="button"
              className="button secondary small"
              disabled={busy || pass.length < 12 || folder.trim() === ''}
              onClick={() => void act('auto_backup_enable', { folder, passphrase: pass }, 'Automatic backups are on. The first one is saved.')}
            >
              Turn on
            </button>
          </div>
        </>
      )}
      {message ? (
        <p className={`status-text ${message.error ? 'error' : ''}`} role={message.error ? 'alert' : 'status'}>
          {message.text}
        </p>
      ) : null}
    </div>
  );
}
