import { useId, useMemo, useState } from 'react';
import { Head, Next } from './Pages.jsx';
import log from './changelog.json';

// The list is written by scripts/changelog.mjs from the git history of main and rebuilt on every merge (pages.yml).
const FILTERS = [['all', 'All'], ['android', 'Android'], ['windows', 'Windows'], ['engine', 'Engine'], ['security', 'Security'], ['site', 'Website'], ['fix', 'Fixes']];
const LABEL = { android: 'Android', windows: 'Windows', engine: 'Engine', security: 'Security', site: 'Website', fix: 'Fix', maintenance: 'Dependencies' };
const DAYS_PAGE = 18;

// Every moment is stored in UTC and shown in the visitor's own time zone, so a "day" is the visitor's day.
const pad = (n) => String(n).padStart(2, '0');
const day = (iso) => { const d = new Date(iso); return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`; };
const at = (k) => { const [y, m, d] = k.split('-').map(Number); return new Date(y, m - 1, d, 12); };
const tile = (k) => at(k).toLocaleDateString('en-GB', { day: 'numeric', month: 'short', year: 'numeric' });
const month = (k) => at(k).toLocaleDateString('en-GB', { month: 'long', year: 'numeric' });
const clock = (iso) => new Date(iso).toLocaleTimeString('en-GB', { hour: '2-digit', minute: '2-digit' });
const zone = (iso) => new Intl.DateTimeFormat('en-US', { timeZoneName: 'short' }).formatToParts(new Date(iso)).find((p) => p.type === 'timeZoneName')?.value ?? '';
const stamp = (iso) => `${tile(day(iso))}, ${clock(iso)} ${zone(iso)}`;
const tagsOf = (e) => [...e.areas, ...(e.kind === 'fix' || e.kind === 'maintenance' ? [e.kind] : [])];

/** The most important change of a day: what touches users or security first, bigger changes over small ones, dependency bumps last. */
function weight(e) {
  let w = 1 + Math.min(e.details.length, 5) * 0.3;
  if (e.areas.includes('security')) w += 3;
  for (const a of ['android', 'windows', 'engine']) if (e.areas.includes(a)) w += 2;
  if (e.areas.includes('site')) w += 1;
  if (e.kind === 'fix') w += 0.5;
  if (e.kind === 'maintenance') w -= 4;
  return w;
}
const headlineOf = (items) => items.reduce((best, e) => (weight(e) > weight(best) ? e : best), items[0]);

function Tags({ e }) {
  const tags = tagsOf(e);
  return tags.length ? <span className="cl-tags">{tags.map((t) => <span key={t} className={`cl-tag t-${t}`}>{LABEL[t]}</span>)}</span> : null;
}

/** One day. A button opens it; the body slides open (and the steps rise in one by one) unless the visitor asked for less motion. */
function DayTile({ d, items, last, plain, repo }) {
  const [open, setOpen] = useState(false);
  const id = useId();
  const head = headlineOf(items);
  const more = items.length - 1;
  return (
    <div className="cl-tile" data-open={open ? '' : undefined}>
      <button type="button" className="cl-open" aria-expanded={open} aria-controls={id} onClick={() => setOpen(!open)}>
        <time dateTime={d} className="cl-tdate">{tile(d)}<span className="cl-time">{clock(items[0].date)}</span>{d === last && plain ? <span className="cl-new">Latest</span> : null}</time>
        <span className="cl-head">{head.title}</span>
        <span className="cl-meta"><Tags e={head} /><span>{more > 0 ? `+${more} more · ${items.length} changes` : '1 change'}</span></span>
      </button>
      <div className="cl-reveal" id={id} role="region" aria-label={`Everything on ${tile(d)}`} inert={open ? undefined : ''} aria-hidden={!open}>
        <div className="cl-clip">
          <div className="cl-body" tabIndex={open ? 0 : -1}>
            <ul>
              {items.map((e, i) => (
                <li key={e.sha} style={{ '--i': Math.min(i, 8) }}>
                  <p className="cl-at"><time dateTime={e.date}>{clock(e.date)} {zone(e.date)}</time></p>
                  <h3>{e.title}</h3>
                  <Tags e={e} />
                  {e.summary ? <p className="cl-sum">{e.summary}</p> : null}
                  {e.details.length ? <ul className="cl-steps">{e.details.map((x) => <li key={x}>{x}</li>)}</ul> : null}
                  <p className="cl-links">
                    {e.pr ? <a href={`${repo}/pull/${e.pr}`}>Pull request #{e.pr}</a> : null}
                    <a href={`${repo}/commit/${e.sha}`}>Commit {e.sha.slice(0, 7)}</a>
                  </p>
                </li>
              ))}
            </ul>
          </div>
        </div>
      </div>
    </div>
  );
}

export function ChangelogPage({ Go, repo }) {
  const [filter, setFilter] = useState('all');
  const [deps, setDeps] = useState(false);
  const [from, setFrom] = useState('');
  const [to, setTo] = useState('');
  const [shown, setShown] = useState(DAYS_PAGE);

  const all = log.entries;
  const first = all.length ? day(all[all.length - 1].date) : '';
  const last = all.length ? day(all[0].date) : '';
  const reset = (fn) => (v) => { fn(v); setShown(DAYS_PAGE); };

  const days = useMemo(() => {
    const out = [];
    for (const e of all) {
      if (e.kind === 'maintenance' && !deps) continue;
      if (filter !== 'all' && !(filter === 'fix' ? e.kind === 'fix' : e.areas.includes(filter))) continue;
      const d = day(e.date);
      if ((from && d < from) || (to && d > to)) continue;
      if (!out.length || out[out.length - 1].d !== d) out.push({ d, items: [] });
      out[out.length - 1].items.push(e);
    }
    return out;
  }, [all, filter, deps, from, to]);

  const count = days.reduce((n, x) => n + x.items.length, 0);
  const visible = days.slice(0, shown);
  const months = [];
  for (const x of visible) {
    const m = month(x.d);
    if (!months.length || months[months.length - 1].m !== m) months.push({ m, days: [] });
    months[months.length - 1].days.push(x);
  }

  return (
    <>
      <Head eyebrow="Changelog" title="What changed, and when" lead="One tile for each day. Open a tile to see everything that happened that day. This page rebuilds itself after every merge, and the full history stays here." />
      <section className="page-sec">
        <div className="wrap">
          <div className="cl-bar">
            <div className="cl-filters" role="group" aria-label="Show changes for">
              {FILTERS.map(([id, t]) => <button key={id} type="button" className="chip" aria-pressed={filter === id} onClick={() => reset(setFilter)(id)}>{t}</button>)}
            </div>
            <div className="cl-dates">
              <label>From <input type="date" value={from} min={first} max={to || last} onChange={(e) => reset(setFrom)(e.target.value)} /></label>
              <label>To <input type="date" value={to} min={from || first} max={last} onChange={(e) => reset(setTo)(e.target.value)} /></label>
              {from || to ? <button type="button" className="chip" onClick={() => { setFrom(''); setTo(''); setShown(DAYS_PAGE); }}>Clear dates</button> : null}
            </div>
            <label className="cl-toggle"><input type="checkbox" checked={deps} onChange={(e) => reset(setDeps)(e.target.checked)} /> Show dependency updates</label>
          </div>
          <p className="fine cl-count" role="status">{count} {count === 1 ? 'change' : 'changes'} on {days.length} {days.length === 1 ? 'day' : 'days'}{last ? <> · latest {tile(last)}</> : null}</p>
          <p className="fine cl-tz">Times are shown in your time zone ({zone(log.generated)}). {log.generated ? <>Page updated <time dateTime={log.generated}>{stamp(log.generated)}</time>.</> : null}</p>

          {months.length === 0 ? <p className="fine">Nothing matches. Try another filter or clear the dates.</p> : months.map(({ m, days: list }) => (
            <section key={m} className="cl-month" aria-label={m}>
              <h2 className="cl-mhead">{m}</h2>
              <div className="cl-grid">
                {list.map(({ d, items }) => <DayTile key={d} d={d} items={items} last={last} plain={filter === 'all' && !from && !to} repo={repo} />)}
              </div>
            </section>
          ))}
          {days.length > shown ? <p className="more-row"><button type="button" className="chip" onClick={() => setShown(shown + DAYS_PAGE)}>Show older days ({days.length - shown} more)</button></p> : null}
        </div>
      </section>
      <Next Go={Go} items={[['download', 'Get the latest build'], ['features', 'Every feature'], ['architecture', 'How it is built']]} />
    </>
  );
}
