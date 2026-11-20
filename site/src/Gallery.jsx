import { useCallback, useEffect, useRef, useState } from 'react';

const reduced = () => typeof matchMedia === 'function' && matchMedia('(prefers-reduced-motion: reduce)').matches;

/** A desktop window around a screenshot: three dots, a title, and (optionally) a small enlarge glyph in the bar. */
export function WindowFrame({ src, alt, w, h, enlarge, eager }) {
  return (
    <span className="win">
      <span className="win-bar" aria-hidden="true"><i /><i /><i /><b>PrivCord</b></span>
      <img key={src} src={src} alt={alt} width={w} height={h} loading={eager ? 'eager' : 'lazy'} draggable="false" />
    </span>
  );
}

/** A phone around a screenshot. */
export function PhoneFrame({ src, alt, w, h }) {
  return (
    <span className="ph">
      <img key={src} src={src} alt={alt} width={w} height={h} loading="lazy" draggable="false" />
    </span>
  );
}

/*
 * Screens showcase: a stage that shows one real screenshot inside a window or phone frame, with a numbered chapter
 * list that auto-plays (each chapter has a progress line; hover, focus, a tap or leaving the screen pauses it). Tap the
 * stage to open the lightbox: a native <dialog> (focus trap, Esc, inert page behind it) with arrows, swipe, zoom to natural
 * size and focus returned to where you came from. Plain CSS, no library; honours prefers-reduced-motion.
 * Pattern references: Magic UI (device frames), React Bits (Stack, stepper), Untitled UI (lightbox), Jitter (progress lines).
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
  const dialogRef = useRef(null);
  const showRef = useRef(null);
  const opener = useRef(null);
  const touch = useRef(null);
  const [open, setOpen] = useState(null); // index of the open screen in the lightbox, or null
  const [zoom, setZoom] = useState(false);
  const [cur, setCur] = useState(0); // chapter on the stage
  const [auto, setAuto] = useState(() => !reduced());
  const [hold, setHold] = useState(false); // pointer or focus is on the showcase
  const [seen, setSeen] = useState(false); // showcase is on screen

  const pick = (i, stop = true) => {
    setCur((i + SHOTS.length) % SHOTS.length);
    if (stop) setAuto(false);
  };

  // Only play while the section is on screen.
  useEffect(() => {
    const el = showRef.current;
    if (!el || typeof IntersectionObserver !== 'function') return undefined;
    const io = new IntersectionObserver(([e]) => setSeen(e.isIntersecting), { threshold: 0.35 });
    io.observe(el);
    return () => io.disconnect();
  }, []);

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

  // The dialog is opened and closed with the state, so the page behind it stays still and focus comes back.
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
    const at = open === null ? cur : open;
    for (const d of [-1, 1]) new Image().src = asset(SHOTS[(at + d + SHOTS.length) % SHOTS.length].src);
  }, [open, cur, asset]);

  const shot = open === null ? null : SHOTS[open];
  const now = SHOTS[cur];
  const playing = auto && seen && !hold && open === null;

  // Arrow keys on the chapter list (a tab list).
  const onTabKey = (e) => {
    const k = e.key;
    if (k !== 'ArrowDown' && k !== 'ArrowUp' && k !== 'ArrowRight' && k !== 'ArrowLeft') return;
    e.preventDefault();
    const next = (cur + (k === 'ArrowDown' || k === 'ArrowRight' ? 1 : -1) + SHOTS.length) % SHOTS.length;
    pick(next);
    document.getElementById(`chap-${next}`)?.focus();
  };

  return (
    <section className="screens" id="screens" aria-labelledby="screens-h">
      <div className="wrap">
        <p className="eyebrow">Screens</p>
        <h1 id="screens-h">Take a look around</h1>
        <p className="sub">Real screenshots, no mock-ups. Pick a screen. Click or tap the picture to open it bigger.</p>

        <div
          className={`show${playing ? ' playing' : ''}`}
          ref={showRef}
          onPointerEnter={(e) => { if (e.pointerType === 'mouse') setHold(true); }}
          onPointerLeave={() => setHold(false)}
          onFocus={() => setHold(true)}
          onBlur={(e) => { if (!e.currentTarget.contains(e.relatedTarget)) setHold(false); }}
        >
          <ol className="chapters" role="tablist" aria-label="Screens" aria-orientation="vertical" onKeyDown={onTabKey}>
            {SHOTS.map((s, i) => (
              <li key={s.src} role="presentation">
                <button
                  type="button"
                  role="tab"
                  id={`chap-${i}`}
                  className="chap"
                  aria-selected={i === cur}
                  aria-controls="stage"
                  tabIndex={i === cur ? 0 : -1}
                  onClick={() => pick(i)}
                >
                  <span className="n" aria-hidden="true">{String(i + 1).padStart(2, '0')}</span>
                  <span className="t">{s.title}</span>
                  <span className="d">{s.note}</span>
                  <i className="bar" aria-hidden="true" onAnimationEnd={() => { if (i === cur) pick(cur + 1, false); }} />
                </button>
              </li>
            ))}
          </ol>

          <div
            className="stage"
            id="stage"
            role="tabpanel"
            aria-labelledby={`chap-${cur}`}
            onTouchStart={(e) => { touch.current = e.touches[0].clientX; }}
            onTouchEnd={(e) => {
              const start = touch.current;
              touch.current = null;
              if (start == null) return;
              const dx = e.changedTouches[0].clientX - start;
              if (Math.abs(dx) > 50) pick(cur + (dx < 0 ? 1 : -1));
            }}
          >
            <button type="button" className="stage-open" onClick={(e) => { setAuto(false); show(cur, e.currentTarget); }} aria-label={`Open full size: ${now.title}`}>
              <span className={`stage-frame ${now.phone ? 'is-phone' : 'is-desk'}`} key={now.src}>
                {now.phone
                  ? <PhoneFrame src={asset(now.src)} alt={`${now.title}. ${now.note}`} w={now.w} h={now.h} />
                  : <WindowFrame src={asset(now.src)} alt={`${now.title}. ${now.note}`} w={now.w} h={now.h} />}
              </span>
            </button>
            <p className="stage-cap"><strong>{now.title}</strong> {now.note}</p>
          </div>
        </div>
      </div>

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
