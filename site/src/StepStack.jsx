import { useEffect, useRef } from 'react';

/*
 * The three steps as a stack of cards on a pinned stage. While you scroll through it the stage stays on screen: on a phone it
 * holds the heading, the stack and a progress row, so the screen is never left empty; on a wide screen the heading sits beside
 * it. Each card slides up from below and lands a little lower than the one before so every title stays visible, and the cards
 * it covers settle back slightly. When the last card has landed the whole stage scrolls away together. Without JavaScript or
 * with reduced motion it is a plain list of cards. Cards move with transforms only, from one passive scroll listener.
 */
const DWELL = 0.35; // extra scroll after the last card lands, in steps
const ease = (t) => 1 - Math.pow(1 - t, 3);
const clamp = (v) => Math.min(1, Math.max(0, v));

export default function StepStack({ steps, intro, more }) {
  const root = useRef(null);
  const pin = useRef(null);
  const stage = useRef(null);
  const count = useRef(null);
  const cards = useRef([]);

  useEffect(() => {
    const el = root.current;
    const pinEl = pin.current;
    const stageEl = stage.current;
    if (!el || !pinEl || !stageEl || typeof matchMedia !== 'function') return undefined;
    if (matchMedia('(prefers-reduced-motion: reduce)').matches) return undefined;
    el.classList.add('is-live');
    const n = steps.length;
    let raf = 0;
    let shown = 0;
    const paint = () => {
      raf = 0;
      const rect = el.getBoundingClientRect();
      const pinTop = parseFloat(getComputedStyle(pinEl).top) || 0;
      const step = (el.offsetHeight - pinEl.offsetHeight) / (n - 1 + DWELL);
      const d = pinTop - rect.top; // how far the stack has scrolled past the point where it pins
      const peek = parseFloat(getComputedStyle(el).getPropertyValue('--peek')) || 64;
      const stageH = stageEl.offsetHeight;
      let landed = 1;
      cards.current.forEach((card, k) => {
        if (!card) return;
        const raw = k === 0 ? 1 : clamp((d - (k - 1) * step) / step);
        if (k > 0 && raw >= 0.5) landed = k + 1;
        const q = ease(raw);
        const rise = (1 - q) * (stageH - k * peek + 32);
        const next = k + 1 < n ? ease(clamp((d - k * step) / step)) : 0; // how much of the next card is over this one
        card.style.transform = `translate3d(0, ${rise.toFixed(1)}px, 0) scale(${(1 - 0.035 * next).toFixed(4)})`;
        card.style.filter = next > 0 ? `brightness(${(1 - 0.12 * next).toFixed(3)})` : '';
      });
      if (landed !== shown) {
        shown = landed;
        el.dataset.step = String(landed);
        if (count.current) count.current.textContent = `Step ${landed} of ${n}`;
      }
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
      delete el.dataset.step;
      cards.current.forEach((c) => { if (c) { c.style.transform = ''; c.style.filter = ''; } });
    };
  }, [steps]);

  return (
    <div className="stack" ref={root} data-step="1" style={{ '--count': steps.length }}>
      <div className="stack-pin" ref={pin}>
        {intro ? <div className="stack-intro">{intro}</div> : null}
        <ol className="stack-stage" ref={stage}>
          {steps.map(([t, p], n) => (
            <li className={`stack-card c${n}`} key={t} style={{ '--n': n }} ref={(e) => { cards.current[n] = e; }}>
              <div className="stack-head"><span className="stack-n" aria-hidden="true">{n + 1}</span><h3>{t}</h3></div>
              <p>{p}</p>
            </li>
          ))}
        </ol>
        <div className="stack-foot">
          <div className="stack-progress" aria-hidden="true">
            <span className="stack-count" ref={count}>Step 1 of {steps.length}</span>
            <span className="stack-dots">{steps.map((s, i) => <i key={s[0]} className={`d${i}`} />)}</span>
          </div>
          {more}
        </div>
      </div>
    </div>
  );
}
