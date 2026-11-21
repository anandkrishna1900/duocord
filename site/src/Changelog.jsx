import { useMemo, useState } from 'react';
import { Head, Next } from './Pages.jsx';
import log from './changelog.json';

// The list is written by scripts/changelog.mjs from the git history of main and rebuilt on every merge (pages.yml).
const FILTERS = [['all', 'All'], ['android', 'Android'], ['windows', 'Windows'], ['engine', 'Engine'], ['security', 'Security'], ['site', 'Website'], ['fix', 'Fixes']];
const LABEL = { android: 'Android', windows: 'Windows', engine: 'Engine', security: 'Security', site: 'Website', fix: 'Fix', maintenance: 'Dependencies' };
const DAYS_PAGE = 18;

const day = (iso) => iso.slice(0, 10);
const at = (d) => new Date(`${d}T12:00:00Z`);
const tile = (d) => at(d).toLocaleDateString('en-GB', { day: 'numeric', month: 'short', year: 'numeric', timeZone: 'UTC' });
const month = (d) => at(d).toLocaleDateString('en-GB', { month: 'long', year: 'numeric', timeZone: 'UTC' });
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

          {months.length === 0 ? <p className="fine">Nothing matches. Try another filter or clear the dates.</p> : months.map(({ m, days: list }) => (
            <section key={m} className="cl-month" aria-label={m}>
              <h2 className="cl-mhead">{m}</h2>
              <div className="cl-grid">
                {list.map(({ d, items }) => {
                  const head = headlineOf(items);
                  const more = items.length - 1;
                  return (
                    <details key={d} className="cl-tile">
                      <summary>
                        <time dateTime={d} className="cl-tdate">{tile(d)}{d === last && filter === 'all' && !from && !to ? <span className="cl-new">Latest</span> : null}</time>
                        <span className="cl-head">{head.title}</span>
                        <span className="cl-meta"><Tags e={head} /><span>{more > 0 ? `+${more} more · ${items.length} changes` : '1 change'}</span></span>
                      </summary>
                      <div className="cl-body" tabIndex={0} aria-label={`Everything on ${tile(d)}`}>
                        <ul>
                          {items.map((e) => (
                            <li key={e.sha}>
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
                    </details>
                  );
                })}
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
