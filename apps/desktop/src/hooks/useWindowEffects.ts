import { useEffect } from 'react';
import { titleFor } from '../lib/insightInputs';

/** Tauri window calls, never fatal: in the browser build the import or the call simply fails. */
async function withWindow(f: (w: import('@tauri-apps/api/window').Window) => Promise<unknown>): Promise<void> {
  try {
    const { getCurrentWindow } = await import('@tauri-apps/api/window');
    await f(getCurrentWindow());
  } catch {
    /* not the desktop app */
  }
}

/** "(N) DuoCord" in the page title and the window title (which the taskbar and tray show) while messages are unread. */
export function useUnreadTitle(unread: number): void {
  useEffect(() => {
    const title = titleFor(unread);
    document.title = title;
    void withWindow((w) => w.setTitle(title));
  }, [unread]);
}

/** Keep the window hidden from screenshots and recording in step with the preference. */
export function useSecureScreen(on: boolean): void {
  useEffect(() => {
    void withWindow((w) => w.setContentProtected(on));
  }, [on]);
}
