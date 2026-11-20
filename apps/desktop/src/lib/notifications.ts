import {
  isPermissionGranted,
  requestPermission,
  sendNotification,
} from '@tauri-apps/plugin-notification';
import { isTauri } from '../config';
import { loadSettings } from './settings';

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
  silent = false,
): Promise<void> {
  const settings = loadSettings();
  if (!settings.notificationsEnabled) return;
  if (document.visibilityState === 'visible' && document.hasFocus()) return;
  const granted = await ensureNotificationPermission();
  if (!granted) return;
  const body = settings.notificationPreviews ? preview(message.text) : 'Open PrivCord to read it.';
  try {
    if (message.conversationId) lastNotified = { conversation: message.conversationId, at: Date.now() };
    if (isTauri()) {
      // `silent` is honoured on mobile; the desktop plugin (2.8) has no switch for it on Windows, so it is best-effort there.
      sendNotification({ title, body, ...(silent ? { silent: true } : {}) });
    } else {
      new Notification(title, { body, tag: `duochat-${message.messageId}` });
    }
  } catch {
    // Notifications are best-effort.
  }
}
