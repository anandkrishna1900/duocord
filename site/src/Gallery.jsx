import { useCallback, useEffect, useRef, useState } from 'react';

/*
 * Screens gallery: a swipeable rail of real screenshots (the App Store pattern: big cards, a peek of the next one,
 * keyboard and button controls) and a lightbox that opens any of them full size. The lightbox is a native <dialog>
 * (focus trap, Esc, inert page behind it), with arrows, swipe, a zoom to natural size for reading small text on a phone,
 * and focus returned to the card you opened. Plain CSS, no library; honours prefers-reduced-motion.
 * Pattern references: React Bits (Carousel, Stack), Magic UI (device frames), Untitled UI (lightbox).
 */
export const SHOTS = [
  { src: 'group-chat-dark.webp', w: 1280, h: 800, phone: false, title: 'A group chat', note: 'Everyone in the group is connected directly. Messages, photos, files and voice notes, end-to-end encrypted.' },
  { src: 'add-friend.webp', w: 1280, h: 800, phone: false, title: 'Add a friend', note: 'Show a QR code or six words. No phone number, no email, no account.' },
  { src: 'group-details.webp', w: 1280, h: 800, phone: false, title: 'Group details', note: 'See who is in the group and who added them. Only an admin can change it.' },
  { src: 'mobile-home.webp', w: 390, h: 800, phone: true, title: 'On Android: your chats', note: 'The same chats on your phone, with the same keys.' },
  { src: 'mobile-group.webp', w: 390, h: 800, phone: true, title: 'On Android: a group', note: 'Reply, react, share photos and voice notes.' },
  { src: 'home-light.webp', w: 1280, h: 800, phone: false, title: 'Light theme', note: 'Pick a colour theme, light or dark, on both apps.' },
  { src: 'welcome.webp', w: 1280, h: 800, phone: false, title: 'Welcome', note: 'You start with a 12-word recovery phrase that only you hold.' },
];

/** Lets any part of the page (the hero) open a screen in the lightbox. */
export function openShot(src, from) {
  window.dispatchEvent(new CustomEvent('privcord:open-shot', { detail: { src, from } }));
}

export default function Gallery({ asset }) {
  const railRef = useRef(null);
  const dialogRef = useRef(null);
  const opener = useRef(null);
  const [open, setOpen] = useState(null); // index of the open screen, or null
  const [zoom, setZoom] = useState(false);
  const touch = useRef(null);

  const scrollRail = (dir) => {
    const rail = railRef.current;
    if (!rail) return;
    const card = rail.querySelector('li');
    const step = (card ? card.getBoundingClientRect().width : 320) + 20;
    const reduce = typeof matchMedia === 'function' && matchMedia('(prefers-reduced-motion: reduce)').matches;
    rail.scrollBy({ left: dir * step, behavior: reduce ? 'auto' : 'smooth' });
  };

  const show = (i, from) => {
    opener.current = from || opener.current;
    setZoom(false);
    setOpen((i + SHOTS.length) % SHOTS.length);
  };
  const close = useCallback(() => {
    setOpen(null);
    setZoom(false);
  }, []);

  useEffect(() => {
    const onOpen = (e) => {
      const i = SHOTS.findIndex((s) => s.src === e.detail.src);
      if (i >= 0) show(i, e.detail.from);
    };
    addEventListener('privcord:open-shot', onOpen);
    return () => removeEventListener('privcord:open-shot', onOpen);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // The dialog is opened and closed with the state, so the page behind it stays still and focus comes back to the card.
  useEffect(() => {
    const d = dialogRef.current;
    if (!d) return undefined;
    if (open !== null && !d.open) {
      d.showModal();
      document.documentElement.style.overflow = 'hidden';
    }
    if (open === null && d.open) {
      d.close();
      document.documentElement.style.overflow = '';
      opener.current?.focus?.();
    }
    return undefined;
  }, [open]);
  useEffect(() => () => { document.documentElement.style.overflow = ''; }, []);

  // Arrow keys while the lightbox is open; Esc is handled by the dialog's own cancel event.
  useEffect(() => {
    if (open === null) return undefined;
    const onKey = (e) => {
      if (e.key === 'ArrowRight') show(open + 1);
      else if (e.key === 'ArrowLeft') show(open - 1);
    };
    addEventListener('keydown', onKey);
    return () => removeEventListener('keydown', onKey);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open]);

  // Warm the neighbours so the arrows feel instant.
  useEffect(() => {
    if (open === null) return;
    for (const d of [-1, 1]) new Image().src = asset(SHOTS[(open + d + SHOTS.length) % SHOTS.length].src);
  }, [open, asset]);

  const shot = open === null ? null : SHOTS[open];

  return (
    <section className="screens" id="screens" aria-labelledby="screens-h">
      <div className="wrap">
        <div className="screens-head">
          <div>
            <p className="eyebrow">Screens</p>
            <h2 id="screens-h">Take a look around</h2>
            <p className="sub">Real screenshots. Tap any screen to see it full size.</p>
          </div>
          <div className="rail-btns" aria-label="Scroll the screens">
            <button type="button" className="round" onClick={() => scrollRail(-1)} aria-label="Previous screens">←</button>
            <button type="button" className="round" onClick={() => scrollRail(1)} aria-label="Next screens">→</button>
          </div>
        </div>
      </div>

      <ul className="rail" ref={railRef} aria-label="PrivCord screens" tabIndex={-1}>
        {SHOTS.map((s, i) => (
          <li key={s.src} className={s.phone ? 'phone' : 'desk'}>
            <button type="button" className="card" onClick={(e) => show(i, e.currentTarget)} aria-label={`Open full size: ${s.title}`}>
              <img src={asset(s.src)} alt="" width={s.w} height={s.h} loading="lazy" draggable="false" />
              <span className="expand" aria-hidden="true">⤢</span>
            </button>
            <p className="cap"><strong>{s.title}</strong></p>
          </li>
        ))}
      </ul>

      <p className="swipe-hint" aria-hidden="true">Swipe for more →</p>

      <dialog
        ref={dialogRef}
        className="lightbox"
        aria-label={shot ? `${shot.title}, screen ${open + 1} of ${SHOTS.length}` : 'Screen'}
        onCancel={(e) => { e.preventDefault(); close(); }}
        onClick={(e) => { if (e.target === e.currentTarget) close(); }}
      >
        {shot ? (
          <div
            className="lb-inner"
            onTouchStart={(e) => { touch.current = e.touches[0].clientX; }}
            onTouchEnd={(e) => {
              const start = touch.current;
              touch.current = null;
              if (start == null || zoom) return;
              const dx = e.changedTouches[0].clientX - start;
              if (Math.abs(dx) > 60) show(open + (dx < 0 ? 1 : -1));
            }}
          >
            <div className="lb-bar">
              <span className="lb-count" aria-live="polite">{open + 1} / {SHOTS.length}</span>
              <span className="lb-spacer" />
              <button type="button" className="round" onClick={() => setZoom((z) => !z)} aria-pressed={zoom} aria-label={zoom ? 'Fit to screen' : 'Zoom to full size'}>{zoom ? '−' : '+'}</button>
              <button type="button" className="round" onClick={close} aria-label="Close" autoFocus>✕</button>
            </div>
            <figure className={`lb-fig${zoom ? ' zoomed' : ''}${shot.phone ? ' is-phone' : ''}`}>
              <div className="lb-scroll">
                <img key={shot.src} src={asset(shot.src)} alt={`${shot.title}. ${shot.note}`} width={shot.w} height={shot.h} onClick={() => setZoom((z) => !z)} />
              </div>
              <figcaption><strong>{shot.title}</strong><span>{shot.note}</span><span className="lb-hint">{zoom ? 'Drag to look around. Tap to fit again.' : 'Tap the screen to zoom in.'}</span></figcaption>
            </figure>
            <button type="button" className="round lb-prev" onClick={() => show(open - 1)} aria-label="Previous screen">←</button>
            <button type="button" className="round lb-next" onClick={() => show(open + 1)} aria-label="Next screen">→</button>
          </div>
        ) : null}
      </dialog>
    </section>
  );
}
