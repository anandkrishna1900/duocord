import type { SettingsSection } from '../components/SettingsDialog';

/** What the privacy checkup looks at (everything is known on this device; nothing is sent anywhere). */
export interface CheckupInput {
  appLock: boolean;
  /** Seconds since epoch of the last backup, or 0 if none was ever made. */
  lastBackupAt: number;
  hideIp: boolean;
  previewsOff: boolean;
  groupOnly: boolean;
  panicWipe: boolean;
  /** Devices on the account that were not heard from for 90 days. */
  staleDevices: number;
  now?: number;
}

export interface CheckupItem {
  id: string;
  label: string;
  ok: boolean;
  /** What to do (shown when not ok) or what is on (when ok). */
  hint: string;
  /** Where in Settings to fix it. */
  section: SettingsSection;
}

const WEEK = 7 * 24 * 3600 * 1000;

export function privacyCheckup(i: CheckupInput): { items: CheckupItem[]; score: number; total: number } {
  const now = i.now ?? Date.now();
  const items: CheckupItem[] = [
    { id: 'lock', label: 'App lock', ok: i.appLock, hint: i.appLock ? 'Opening DuoCord asks for your passkey.' : 'Anyone at this computer can open your chats. Turn on the lock.', section: 'security' },
    {
      id: 'backup',
      label: 'Backup',
      ok: i.lastBackupAt > 0 && now - i.lastBackupAt < 4 * WEEK,
      hint: i.lastBackupAt > 0 && now - i.lastBackupAt < 4 * WEEK ? 'A recent backup exists.' : 'No recent backup. Without one, a lost computer means lost chats (your 12 words only restore your identity).',
      section: 'backup',
    },
    { id: 'ip', label: 'IP address hidden from friends', ok: i.hideIp, hint: i.hideIp ? 'Connections go through a relay.' : 'Friends can see your IP address on a direct connection. Optional, slower.', section: 'connection' },
    { id: 'previews', label: 'Notifications hide message text', ok: i.previewsOff, hint: i.previewsOff ? 'Notifications only say a message arrived.' : 'Message text can appear on a locked screen. Turn off message previews.', section: 'notifications' },
    { id: 'grouponly', label: 'Group-only with people met in groups', ok: i.groupOnly, hint: i.groupOnly ? 'Strangers from a group cannot message you directly.' : 'People you only know from a group can message you directly.', section: 'security' },
    { id: 'panic', label: 'Panic wipe ready', ok: i.panicWipe, hint: i.panicWipe ? 'Three quick Ctrl+Shift+Backspace erases this device.' : 'Optional: a one-gesture wipe for emergencies.', section: 'security' },
    { id: 'devices', label: 'No forgotten devices', ok: i.staleDevices === 0, hint: i.staleDevices === 0 ? 'Every linked device was seen recently.' : `${i.staleDevices} linked device${i.staleDevices === 1 ? '' : 's'} not seen for 90 days. Remove ${i.staleDevices === 1 ? 'it' : 'them'} if lost.`, section: 'devices' },
  ];
  const score = items.filter((x) => x.ok).length;
  return { items, score, total: items.length };
}
