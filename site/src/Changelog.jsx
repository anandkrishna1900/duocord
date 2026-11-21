import { useMemo, useState } from 'react';
import { Head, Next } from './Pages.jsx';
import log from './changelog.json';

// The list is written by scripts/changelog.mjs from the git history of main and rebuilt on every merge (pages.yml).
const FILTERS = [['all', 'All'], ['android', 'Android'], ['windows', 'Windows'], ['engine', 'Engine'], ['security', 'Security'], ['site', 'Website'], ['fix', 'Fixes']];
const LABEL = { android: 'Android', windows: 'Windows', engine: 'Engine', security: 'Security', site: 'Website', fix: 'Fix', maintenance: 'Dependencies' };
const PAGE = 30;

const day = (iso) => iso.slice(0, 10);
const pretty = (d) => new Date(`${d}T12:00:00Z`).toLocaleDateString('en-GB', { day: 'numeric', month: 'long', year: 'numeric', timeZone: 'UTC' });
const tagsOf = (e) => [...e.areas, ...(e.kind === 'fix' || e.kind === 'maintenance' ? [e.kind] : [])];

export function ChangelogPage({ Go, repo }) {
  const [filter, setFilter] = useState('all');
  const [deps, setDeps] = useState(false);
  const [shown, setShown] = useState(PAGE);

  const rows = useMemo(() => log.entries.filter((e) => {
    if (e.kind === 'maintenance' && !deps) return false;
    if (filter === 'all') return true;
    return filter === 'fix' ? e.kind === 'fix' : e.areas.includes(filter);
  }), [filter, deps]);

  const visible = rows.slice(0, shown);
  const days = [];
  for (const e of visible) {
    const d = day(e.date);
    if (!days.length || days[days.length - 1].d !== d) days.push({ d, items: [] });
    days[days.length - 1].items.push(e);
  }
  const newest = log.entries[0];

  return (
    <>
      <Head eyebrow="Changelog" title="What changed, and when" lead="Every change that reaches the project, newest first. This page rebuilds itself after each merge, so it is never behind, and the full history stays here." />
      <section className="page-sec">
        <div className="wrap narrow">
          <div className="cl-bar">
            <div className="cl-filters" role="group" aria-label="Show changes for">
              {FILTERS.map(([id, t]) => (
                <button key={id} type="button" className="chip" aria-pressed={filter === id} onClick={() => { setFilter(id); setShown(PAGE); }}>{t}</button>
              ))}
            </div>
            <label className="cl-toggle"><input type="checkbox" checked={deps} onChange={(e) => { setDeps(e.target.checked); setShown(PAGE); }} /> Show dependency updates</label>
          </div>
          <p className="fine cl-count" role="status">
            {rows.length} {rows.length === 1 ? 'change' : 'changes'}{newest ? <> · latest {pretty(day(newest.date))}</> : null}
          </p>

          {days.length === 0 ? <p className="fine">Nothing here yet.</p> : (
            <ol className="cl-days">
              {days.map(({ d, items }, i) => (
                <li key={d} className="cl-day">
                  <h2 className="cl-date"><time dateTime={d}>{pretty(d)}</time>{i === 0 && shown === PAGE && filter === 'all' ? <span className="cl-new">Latest</span> : null}</h2>
                  <ul className="cl-items">
                    {items.map((e) => (
                      <li key={e.sha} className="cl-item">
                        <h3>{e.title}</h3>
                        {tagsOf(e).length ? <p className="cl-tags">{tagsOf(e).map((t) => <span key={t} className={`cl-tag t-${t}`}>{LABEL[t]}</span>)}</p> : null}
                        {e.summary ? <p className="cl-sum">{e.summary}</p> : null}
                        {e.details.length ? (
                          <details className="cl-more">
                            <summary>{e.details.length} {e.details.length === 1 ? 'step' : 'steps'} in this change</summary>
                            <ul>{e.details.map((x) => <li key={x}>{x}</li>)}</ul>
                          </details>
                        ) : null}
                        <p className="cl-links">
                          {e.pr ? <a href={`${repo}/pull/${e.pr}`}>Pull request #{e.pr}</a> : null}
                          <a href={`${repo}/commit/${e.sha}`}>Commit {e.sha.slice(0, 7)}</a>
                        </p>
                      </li>
                    ))}
                  </ul>
                </li>
              ))}
            </ol>
          )}
          {rows.length > shown ? <p className="more-row"><button type="button" className="chip" onClick={() => setShown(shown + PAGE)}>Show older changes ({rows.length - shown} more)</button></p> : null}
        </div>
      </section>
      <Next Go={Go} items={[['download', 'Get the latest build'], ['features', 'Every feature'], ['architecture', 'How it is built']]} />
    </>
  );
}
