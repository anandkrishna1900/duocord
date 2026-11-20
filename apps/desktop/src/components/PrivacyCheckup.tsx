import { useEffect, useState } from 'react';
import { loadNetworkConfig } from '../lib/network';
import { privacyCheckup } from '../lib/privacyCheckup';
import type { AppSettings } from '../lib/settings';
import type { ChatState } from '../lib/engine';
import { isLockEnabled } from '../lib/appLock';
import type { SettingsSection } from './SettingsDialog';
import { AlertIcon, CheckIcon } from './Icons';

export const LAST_BACKUP_KEY = 'duochat.lastBackupAt';

async function lastBackupAt(): Promise<number> {
  let best = 0;
  try {
    best = Number(localStorage.getItem(LAST_BACKUP_KEY)) || 0;
  } catch {
    /* no storage */
  }
  try {
    const { invoke } = await import('@tauri-apps/api/core');
    const s = await invoke<{ lastAt: number }>('auto_backup_status');
    if (s.lastAt > best) best = s.lastAt;
  } catch {
    /* not the desktop app */
  }
  return best;
}

/** A short list of privacy basics, how many hold, and a button to fix each one. Everything is read on this device. */
export function PrivacyCheckup({
  settings,
  state,
  go,
}: {
  settings: AppSettings;
  state: ChatState;
  go: (section: SettingsSection) => void;
}) {
  const [backupAt, setBackupAt] = useState(0);
  useEffect(() => {
    void lastBackupAt().then(setBackupAt);
  }, []);
  const { items, score, total } = privacyCheckup({
    appLock: isLockEnabled(),
    lastBackupAt: backupAt,
    hideIp: loadNetworkConfig().forceRelay === true,
    previewsOff: !settings.notificationPreviews,
    groupOnly: settings.groupOnly,
    panicWipe: settings.panicWipe,
    staleDevices: (state.devices ?? []).filter((d) => d.stale).length,
  });
  return (
    <div className="card-box checkup">
      <div className="card-title">
        <strong>Privacy checkup</strong>
        <span className="checkup-score" aria-label={`${score} of ${total} in place`}>
          {score}/{total}
        </span>
      </div>
      <div className="checkup-bar" role="presentation">
        <span style={{ width: `${(score / total) * 100}%` }} />
      </div>
      <ul className="checkup-list">
        {items.map((item) => (
          <li key={item.id} className={item.ok ? 'ok' : 'todo'}>
            <span className="checkup-mark">{item.ok ? <CheckIcon size={15} /> : <AlertIcon size={15} />}</span>
            <span className="checkup-text">
              <strong>{item.label}</strong>
              <span className="muted small">{item.hint}</span>
            </span>
            {item.ok ? null : (
              <button type="button" className="button ghost small" onClick={() => go(item.section)}>
                Fix
              </button>
            )}
          </li>
        ))}
      </ul>
    </div>
  );
}
