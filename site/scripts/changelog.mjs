// Builds src/changelog.json from the git history of main: one entry per merged pull request (squash merges, and
// older "Merge pull request" commits), newest first. It runs before every site build (npm "prebuild"/"predev"), and
// pages.yml runs on every push to main with the full history, so the Changelog page updates by itself after each merge.
// Without git history (a shallow clone, no .git) an existing file is kept, so a build never fails because of it.
import { execFileSync } from 'node:child_process';
import { existsSync, writeFileSync } from 'node:fs';

const OUT = new URL('../src/changelog.json', import.meta.url);
const SEP = '\x1e';
const FIELD = '\x1f';

function history() {
  const root = execFileSync('git', ['rev-parse', '--show-toplevel'], { encoding: 'utf8', cwd: new URL('.', import.meta.url) }).trim();
  return execFileSync('git', ['log', '--first-parent', `--format=%H${FIELD}%cI${FIELD}%s${FIELD}%b${SEP}`], { encoding: 'utf8', cwd: root, maxBuffer: 64 * 1024 * 1024 })
    .split(SEP)
    .map((r) => r.replace(/^\n+/, ''))
    .filter(Boolean);
}

const TRAILER = /^(co-authored-by|claude-session|signed-off-by)\b/i;
const clean = (s) => s.replace(/\s+/g, ' ').trim();
const clip = (s, n) => (s.length > n ? `${s.slice(0, n - 1).trimEnd()}…` : s);

export function parse(record) {
  const [sha, date, subject0, body = ''] = record.split(FIELD);
  let subject = subject0.trim();
  const lines = body.split('\n').map((l) => l.trimEnd());
  // Older merge commits: "Merge pull request #12 from x/y" with the pull request title as the first body line.
  const merge = /^Merge pull request #(\d+)\b/.exec(subject);
  let pr = merge ? merge[1] : null;
  if (merge) subject = (lines.find((l) => l.trim()) ?? subject).trim();
  const tail = /\s*\(#(\d+)\)\s*$/.exec(subject);
  if (tail) { pr = tail[1]; subject = subject.slice(0, tail.index); }
  const title = clean(subject);

  const bullets = [];
  const paragraphs = [];
  let para = [];
  for (const l of lines) {
    if (!l.trim()) { if (para.length) paragraphs.push(para.join(' ')); para = []; continue; }
    if (TRAILER.test(l.trim())) continue;
    if (/^\*\s+/.test(l)) { if (para.length) paragraphs.push(para.join(' ')); para = []; bullets.push(clean(l.replace(/^\*\s+/, ''))); continue; }
    if (/^---+$/.test(l.trim())) continue;
    para.push(l.trim());
  }
  if (para.length) paragraphs.push(para.join(' '));
  const details = [...new Set(bullets)].filter((b) => b && b !== title).map((b) => clip(b, 160)).slice(0, 8);
  const summary = clip(clean(paragraphs.find((p) => p !== title && p.length > 20) ?? ''), 420);

  const text = `${title} ${details.join(' ')}`.toLowerCase();
  const areas = [];
  if (/\bandroid\b|kotlin|\bapk\b|jetpack|compose|\bphone/.test(text)) areas.push('android');
  if (/\bdesktop\b|windows|installer|tauri|\btray\b|\bexe\b/.test(text)) areas.push('windows');
  if (/rust|\bengine\b|\bcrates?\b|protocol|storage|sync log|ffi|uniffi/.test(text)) areas.push('engine');
  if (/secur|encrypt|\bsealed?\b|codeql|cryptograph|post-quantum|\baudit|privacy|hide my ip|\btor\b|kill switch|panic|cleartext|plain text|signing|signed/.test(text)) areas.push('security');
  if (/^site\b|landing page|website|\bsite:/i.test(title)) areas.push('site');
  const kind = /^bump |dependabot|dependency (bumps|updates)/i.test(title) ? 'maintenance' : /(^|[\s:(])(fix|fixes|fixed)\b/i.test(title) ? 'fix' : 'change';
  return { sha, date, title, pr, summary, details, areas, kind };
}

let entries = null;
try {
  entries = history().map(parse).filter((e) => e.title);
} catch (error) {
  console.warn(`changelog: no git history (${String(error.message).split('\n')[0]})`);
}
if (entries) {
  writeFileSync(OUT, `${JSON.stringify({ generated: new Date().toISOString(), entries }, null, 1)}\n`);
  console.log(`changelog: ${entries.length} entries`);
} else if (!existsSync(OUT)) {
  writeFileSync(OUT, `${JSON.stringify({ generated: new Date().toISOString(), entries: [] })}\n`);
}
