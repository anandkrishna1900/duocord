/** Tells the CSS where the pointer is inside a card or row, so its soft spotlight can follow it (see polish.css). */
const TARGETS = '.card-box, .chat-row, .sidebar-button';

export function installSpotlight(): void {
  document.addEventListener(
    'pointermove',
    (event) => {
      const el = (event.target as Element | null)?.closest?.(TARGETS) as HTMLElement | null;
      if (!el) return;
      const box = el.getBoundingClientRect();
      el.style.setProperty('--mx', `${event.clientX - box.left}px`);
      el.style.setProperty('--my', `${event.clientY - box.top}px`);
    },
    { passive: true },
  );
}
