import { useEffect, useRef } from 'react';

/*
 * The three steps as a stack of cards. The list is pinned while you scroll through it: each card slides up from below and
 * lands a little lower than the one before, so every title stays visible, and the cards it covers settle back a touch.
 * When the last card has landed the whole stack scrolls away together. It is plain stacked cards without JavaScript or
 * with reduced motion. Cards are moved with transforms only, from one passive scroll listener.
 */
const DWELL = 0.35; // extra scroll after the last card lands, in steps
const ease = (t) => 1 - Math.pow(1 - t, 3);
const clamp = (v) => Math.min(1, Math.max(0, v));

export default function StepStack({ steps }) {
  const root = useRef(null);
  const pin = useRef(null);
  const cards = useRef([]);

  useEffect(() => {
    const el = root.current;
    const pinEl = pin.current;
    if (!el || !pinEl || typeof matchMedia !== 'function') return undefined;
    const reduce = matchMedia('(prefers-reduced-motion: reduce)');
    if (reduce.matches) return undefined;
    el.classList.add('is-live');
    const n = steps.length;
    let raf = 0;
    const paint = () => {
      raf = 0;
      const rect = el.getBoundingClientRect();
      const pinTop = parseFloat(getComputedStyle(pinEl).top) || 0;
      const step = (el.offsetHeight - pinEl.offsetHeight) / (n - 1 + DWELL);
      const d = pinTop - rect.top; // how far the stack has scrolled past the point where it pins
      const peek = parseFloat(getComputedStyle(el).getPropertyValue('--peek')) || 64;
      const pinH = pinEl.offsetHeight;
      cards.current.forEach((card, k) => {
        if (!card) return;
        const q = k === 0 ? 1 : ease(clamp((d - (k - 1) * step) / step));
        const rise = (1 - q) * (pinH - k * peek + 32);
        const next = k + 1 < n ? ease(clamp((d - k * step) / step)) : 0; // how much of the next card is over this one
        card.style.transform = `translate3d(0, ${rise.toFixed(1)}px, 0) scale(${(1 - 0.035 * next).toFixed(4)})`;
        card.style.filter = next > 0 ? `brightness(${(1 - 0.12 * next).toFixed(3)})` : '';
      });
    };
    const on = () => { if (!raf) raf = requestAnimationFrame(paint); };
    paint();
    addEventListener('scroll', on, { passive: true });
    addEventListener('resize', on);
    return () => {
      removeEventListener('scroll', on);
      removeEventListener('resize', on);
      if (raf) cancelAnimationFrame(raf);
      el.classList.remove('is-live');
      cards.current.forEach((c) => { if (c) { c.style.transform = ''; c.style.filter = ''; } });
    };
  }, [steps]);

  return (
    <ol className="stack" ref={root} style={{ '--count': steps.length }}>
      <div className="stack-pin" ref={pin}>
        {steps.map(([t, p], n) => (
          <li className={`stack-card c${n}`} key={t} style={{ '--n': n }} ref={(e) => { cards.current[n] = e; }}>
            <div className="stack-head"><span className="stack-n" aria-hidden="true">{n + 1}</span><h3>{t}</h3></div>
            <p>{p}</p>
          </li>
        ))}
      </div>
    </ol>
  );
}
