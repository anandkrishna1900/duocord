import {
  isPermissionGranted,
  requestPermission,
  sendNotification,
} from '@tauri-apps/plugin-notification';
import { isTauri } from '../config';
import { loadSettings } from './settings';
import { inQuietHours, msUntilChange } from './quietHours';
import { pushNativeConfig } from './network';

function preview(text: string): string {
  const single = text.replace(/\s+/g, ' ').trim();
  return single.length > 140 ? `${single.slice(0, 137)}…` : single;
}

/**
 * Windows gives the page no click event for a notification. Clicking one brings the window to the front, so the chat
 * that notified last is remembered for a short while and opened when the window next gains focus.
 */
const OPEN_WINDOW_MS = 30_000;
let lastNotified: { conversation: string; at: number } | null = null;

export function takeRecentNotified(now = Date.now()): string | null {
  const recent = lastNotified && now - lastNotified.at <= OPEN_WINDOW_MS ? lastNotified.conversation : null;
  lastNotified = null;
  return recent;
}

export async function ensureNotificationPermission(): Promise<boolean> {
  if (isTauri()) {
    if (await isPermissionGranted()) return true;
    return (await requestPermission()) === 'granted';
  }
  if (!('Notification' in window)) return false;
  if (Notification.permission === 'granted') return true;
  if (Notification.permission === 'denied') return false;
  return (await Notification.requestPermission()) === 'granted';
}

export async function notifyIncoming(
  message: { text: string; messageId: string; conversationId?: string },
  title: string,
): Promise<void> {
  const settings = loadSettings();
  if (!settings.notificationsEnabled) return;
  if (inQuietHours(settings.quietHours)) return;
  if (document.visibilityState === 'visible' && document.hasFocus()) return;
  const granted = await ensureNotificationPermission();
  if (!granted) return;
  const body = settings.notificationPreviews ? preview(message.text) : 'Open PrivCord to read it.';
  try {
    if (message.conversationId) lastNotified = { conversation: message.conversationId, at: Date.now() };
    if (isTauri()) {
      sendNotification({ title, body });
    } else {
      new Notification(title, { body, tag: `duochat-${message.messageId}` });
    }
  } catch {
    // Notifications are best-effort.
  }
}

let quietTimer: number | undefined;

/**
 * The tray shows notifications itself while the window is closed. It only knows "on" or "off", so this switches it off
 * during quiet hours and back on after, and re-arms itself for the next change.
 */
export function syncQuietHours(): void {
  window.clearTimeout(quietTimer);
  const s = loadSettings();
  void pushNativeConfig({ enabled: s.notificationsEnabled && !inQuietHours(s.quietHours), previews: s.notificationPreviews });
  if (s.quietHours.enabled) quietTimer = window.setTimeout(syncQuietHours, msUntilChange(s.quietHours) + 500);
}
