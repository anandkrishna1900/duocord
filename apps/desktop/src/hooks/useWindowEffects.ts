import { useEffect } from 'react';
import { badgeText, titleFor, tooltipFor } from '../lib/insightInputs';

/** Tauri window calls, never fatal: in the browser build the import or the call simply fails. */
async function withWindow(f: (w: import('@tauri-apps/api/window').Window) => Promise<unknown>): Promise<void> {
  try {
    const { getCurrentWindow } = await import('@tauri-apps/api/window');
    await f(getCurrentWindow());
  } catch {
    /* not the desktop app */
  }
}

/** A 32px red circle with the number (RGBA bytes), for the Windows taskbar overlay. */
function badgeRgba(text: string): Uint8Array | null {
  const canvas = document.createElement('canvas');
  canvas.width = 32;
  canvas.height = 32;
  const g = canvas.getContext('2d');
  if (!g) return null;
  g.fillStyle = '#d93025';
  g.beginPath();
  g.arc(16, 16, 16, 0, Math.PI * 2);
  g.fill();
  g.fillStyle = '#ffffff';
  g.font = `bold ${text.length > 1 ? 18 : 22}px sans-serif`;
  g.textAlign = 'center';
  g.textBaseline = 'middle';
  g.fillText(text, 16, 17);
  return new Uint8Array(g.getImageData(0, 0, 32, 32).data.buffer);
}

/**
 * "(N) DuoCord" in the page title and the window title (which the taskbar shows) while messages are unread, the same
 * count in the tray tooltip, and a number badge on the taskbar button. Each Tauri call is best-effort: the browser build
 * has none of them.
 */
export function useUnreadTitle(unread: number): void {
  useEffect(() => {
    const title = titleFor(unread);
    document.title = title;
    void withWindow((w) => w.setTitle(title));
    void (async () => {
      try {
        const { TrayIcon } = await import('@tauri-apps/api/tray');
        await (await TrayIcon.getById('main'))?.setTooltip(tooltipFor(unread));
      } catch {
        /* no tray */
      }
    })();
    const text = badgeText(unread);
    void withWindow(async (w) => {
      if (!text) return w.setOverlayIcon(undefined);
      const rgba = badgeRgba(text);
      if (!rgba) return;
      const { Image } = await import('@tauri-apps/api/image');
      return w.setOverlayIcon(await Image.new(rgba, 32, 32));
    });
  }, [unread]);
}

/** Keep the window hidden from screenshots and recording in step with the preference. */
export function useSecureScreen(on: boolean): void {
  useEffect(() => {
    void withWindow((w) => w.setContentProtected(on));
  }, [on]);
}
