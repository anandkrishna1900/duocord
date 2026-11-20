import Gallery, { openShot, WindowFrame, PhoneFrame } from './Gallery.jsx';
import { createContext, useContext, useEffect, useRef, useState } from 'react';
// Components below are from React Bits (https://reactbits.dev, MIT + Commons Clause), copied into src/rb. See THIRD_PARTY.md.
import DotGrid from './rb/DotGrid.jsx';
import SplitText from './rb/SplitText.jsx';
import ShinyText from './rb/ShinyText.jsx';
import ScrollVelocity from './rb/ScrollVelocity.jsx';
import ScrollReveal from './rb/ScrollReveal.jsx';
import ScrollFloat from './rb/ScrollFloat.jsx';
import SpotlightCard from './rb/SpotlightCard.jsx';
import GlareHover from './rb/GlareHover.jsx';
import ClickSpark from './rb/ClickSpark.jsx';
import Magnet from './rb/Magnet.jsx';
import StarBorder from './rb/StarBorder.jsx';
import AnimatedContent from './rb/AnimatedContent.jsx';
import FadeContent from './rb/FadeContent.jsx';

const REPO = 'https://github.com/__REPO__';
const asset = (n) => `${import.meta.env.BASE_URL}assets/${n}`;

function useReducedMotion() {
  const [r, setR] = useState(() => typeof matchMedia === 'function' && matchMedia('(prefers-reduced-motion: reduce)').matches);
  useEffect(() => {
    const m = matchMedia('(prefers-reduced-motion: reduce)');
    const f = () => setR(m.matches);
    m.addEventListener('change', f);
    return () => m.removeEventListener('change', f);
  }, []);
  return r;
}

/** Pauses a canvas background while it is off screen. */
function useOnScreen(ref, page) {
  const [on, setOn] = useState(true);
  useEffect(() => {
    if (!ref.current) return;
    const io = new IntersectionObserver(([e]) => setOn(e.isIntersecting), { rootMargin: '120px' });
    io.observe(ref.current);
    return () => io.disconnect();
  }, [ref, page]);
  return on;
}

const Icon = ({ d }) => (
  <svg viewBox="0 0 24 24" aria-hidden="true" className="ic">
    {d}
  </svg>
);
const P = {
  lock: <><rect x="4" y="11" width="16" height="10" rx="2" /><path d="M8 11V8a4 4 0 0 1 8 0v3" /></>,
  qr: <><rect x="3" y="3" width="7" height="7" rx="1" /><rect x="14" y="3" width="7" height="7" rx="1" /><rect x="3" y="14" width="7" height="7" rx="1" /><path d="M14 14h3v3h-3zM20 14v.01M14 20h.01M20 20h-3v-3" /></>,
  users: <><circle cx="9" cy="8" r="3.5" /><path d="M2.5 20a6.5 6.5 0 0 1 13 0" /><circle cx="17" cy="9" r="2.5" /><path d="M17 14a5 5 0 0 1 5 5" /></>,
  laptop: <><rect x="4" y="5" width="16" height="11" rx="2" /><path d="M2 20h20" /></>,
  clip: <path d="M21 11.5l-8.6 8.6a5 5 0 0 1-7-7L14 4.5a3.4 3.4 0 0 1 5 5l-8.6 8.6a1.8 1.8 0 0 1-2.5-2.5L15 9" />,
  bt: <path d="M7 7l10 10-5 5V2l5 5L7 17" />,
  key: <><circle cx="8" cy="15" r="4" /><path d="M11 12l9-9M16 7l3 3M14 9l2 2" /></>,
  clock: <><circle cx="12" cy="12" r="9" /><path d="M12 7v5l3 2" /></>,
  pal: <><circle cx="12" cy="12" r="9" /><circle cx="8" cy="10" r="1" /><circle cx="12" cy="7.5" r="1" /><circle cx="16" cy="10" r="1" /></>,
  globe: <><circle cx="12" cy="12" r="9" /><path d="M3 12h18M12 3c3 3 3 15 0 18M12 3c-3 3-3 15 0 18" /></>,
  mon: <><rect x="3" y="4" width="18" height="12" rx="2" /><path d="M8 20h8M12 16v4" /></>,
  ph: <><rect x="7" y="2" width="10" height="20" rx="2" /><path d="M11 18h2" /></>,
};

const FEATURES = [
  { cls: 'f-wide', i: 'lock', t: 'Direct, end-to-end encrypted chats', p: "Messages are encrypted on your device and go straight to your friend's over WebRTC. The handshake mixes ML-KEM-768 with P-256, so it is built to hold up against future quantum computers too. Optional relays only ever see sealed blobs." },
  { cls: 'f-wide', i: 'key', t: 'You hold the keys', p: 'A 12-word recovery phrase is your account. Lock the app with Windows Hello or your fingerprint, make passphrase-locked backups, and wipe everything in one tap.' },
  { cls: '', i: 'qr', t: 'Add a friend in seconds', p: 'Show a QR code or read out six words. No long strings to copy.' },
  { cls: '', i: 'users', t: 'Groups of up to 10', p: 'Add Ben, Ben adds Cat, and you and Cat connect automatically.' },
  { cls: '', i: 'laptop', t: 'One account, many devices', p: 'Link your phone and PC with a QR code. The account key never leaves your first device.' },
  { cls: 'f-wide', i: 'bt', t: 'Keeps going with no internet', tag: 'Android, experimental', p: 'An optional Bluetooth mesh passes sealed messages through nearby phones, up to 7 hops. So far tested in simulation, not on real radios.' },
  { cls: 'f-wide', i: 'clip', t: 'Files, photos, voice notes', p: "Up to 8 MB, sent straight to the other device with no upload to anyone's server." },
  { cls: '', i: 'clock', t: 'Send later', p: 'Write now, schedule it for the time you pick.' },
  { cls: '', i: 'pal', t: '16 themes', p: 'Midnight, AMOLED, Nord, Dracula, Blossom and more, on both apps.' },
  { cls: '', i: 'globe', t: 'Public channels', p: 'Join rooms like #general with no account. Messages are signed, but anyone who knows the name can read them.' },
];

const STEPS = [
  ['Install and write down 12 words', 'That recovery phrase is your account. It is never stored anywhere else, so keep it safe: the same words restore you on a new device.'],
  ['Add a friend', "Show your QR code or six words. They scan or type them, you tap Add. Both of you now have each other's keys pinned."],
  ['Chat', 'Messages go device to device. If your friend is offline, they wait and send when you reconnect.'],
];

const FAQ = [
  ['Does PrivCord need a phone number, email or account?', 'No. Your identity is a 12-word recovery phrase that you hold. There is nothing to sign up for.'],
  ['Is PrivCord end-to-end encrypted?', 'Yes. Messages are encrypted on your device and only the friend they are meant for can open them. Every link is authenticated against the keys you pinned when you added the friend, and the key exchange uses a post-quantum hybrid (ML-KEM-768 with P-256).'],
  ['Is it really serverless?', 'Not completely, and it says so. To find a friend across the internet it uses optional signaling relays (public Nostr relays or your own), STUN, and on strict networks TURN. None of them can read your messages, though they can see connection metadata such as your IP address. You can run your own, or use none on a local network or over Bluetooth.'],
  ['Can I use it without internet?', 'On Android, an optional Bluetooth mesh carries sealed messages through nearby phones, and linked devices on the same Wi-Fi sync directly. The mesh is experimental and has only been tested in simulation, not on real radios.'],
  ['Which platforms are supported?', 'Windows and Android today. There is no iPhone, macOS or Linux app yet.'],
  ['What if I lose my phone?', 'Remove the device from another device of yours: it takes effect in 24 hours (any device can cancel it), or at once with your recovery phrase. Then restore your account on a new device with the 12 words.'],
  ['Is PrivCord open source?', 'The full source is public and free for personal, educational and research use. The PC-NC license does not allow commercial use without written permission, so it is source-available rather than OSI open source.'],
  ['Has it been independently audited?', 'Not yet. The protocol is covered by tests, shared test vectors and fuzzing in CI, and the security policy is in the repository. Please read the known limitations before relying on it.'],
  ['Is PrivCord a Signal, Session or Briar alternative?', 'It covers similar ground (private, encrypted chat) with a different design: no phone number, device-to-device links instead of a central server, and an offline Bluetooth mesh.'],
];

const COMPARE = [
  ['Phone number or email needed', 'No', 'Phone number', 'No', 'No'],
  ['Messages go through company servers', 'No (device to device; optional relays see only sealed blobs)', 'Yes', 'Yes (a decentralized node network)', 'No (Tor, Wi-Fi or Bluetooth)'],
  ['Works over Bluetooth with no internet', 'Yes (Android mesh, experimental)', 'No', 'No', 'Yes'],
  ['One account on several devices', 'Yes', 'Yes', 'Limited', 'Limited'],
  ['Independently audited', 'No, not yet', 'Yes', 'Yes', 'Yes'],
];

const NAV = [['features', 'Features'], ['how', 'How it works'], ['screens', 'Screens'], ['compare', 'Compare'], ['faq', 'FAQ']];
const TITLES = { home: 'PrivCord: private encrypted messenger with no phone number or account', features: 'Features: PrivCord', how: 'How it works: PrivCord', screens: 'Screens: PrivCord', compare: 'How PrivCord compares', faq: 'Questions people ask: PrivCord', download: 'Get PrivCord for Windows and Android' };
const DOORS = [
  ['features', 'Features', 'Encryption, groups, many devices, files, Bluetooth with no internet.'],
  ['how', 'How it works', 'Three steps from install to your first message.'],
  ['screens', 'Screens', 'Real screenshots of both apps.'],
  ['compare', 'How it compares', 'Next to Signal, Session and Briar, honestly.'],
  ['faq', 'Questions', 'Accounts, servers, audits, losing your phone.'],
  ['download', 'Download', 'Windows and Android.'],
];
// Pages are hash routes (#/features) so the site works from any GitHub Pages path with no server rewrites. Old anchors like #faq still work.
const pageFromHash = () => {
  const h = location.hash.replace(/^#\/?/, '');
  return h === 'top' ? 'home' : TITLES[h] ? h : null;
};
function usePage() {
  const [route, setRoute] = useState(() => pageFromHash() || 'home');
  useEffect(() => {
    const on = () => {
      const r = pageFromHash();
      if (r) setRoute(r);
      else if (!location.hash || location.hash === '#/') setRoute('home');
    };
    addEventListener('hashchange', on);
    return () => removeEventListener('hashchange', on);
  }, []);
  return route;
}

const ReduceCtx = createContext(false);
// Wrappers that render plain content when the visitor asked for reduced motion. They live outside App so they keep their identity.
function Anim({ children, ...p }) {
  return useContext(ReduceCtx) ? <div className={p.className}>{children}</div> : <AnimatedContent {...p}>{children}</AnimatedContent>;
}
function Fade({ children, ...p }) {
  return useContext(ReduceCtx) ? <div className={p.className}>{children}</div> : <FadeContent {...p}>{children}</FadeContent>;
}
const INK = '#0a0a0a';
const YELLOW = '#ffe500';

export default function App() {
  const reduce = useReducedMotion();
  const route = usePage();
  const mainRef = useRef(null);
  const first = useRef(true);
  useEffect(() => {
    document.title = TITLES[route];
    if (first.current) { first.current = false; return; }
    window.scrollTo({ top: 0, behavior: 'instant' });
    mainRef.current?.focus({ preventScroll: true });
  }, [route]);
  const heroRef = useRef(null);
  const dlRef = useRef(null);
  const heroOn = useOnScreen(heroRef, route);
  const dlOn = useOnScreen(dlRef, route);
  const split = (text, extra = {}) =>
    reduce ? text : <SplitText tag="span" className="line" text={text} splitType="chars" delay={22} duration={0.9} textAlign="left" from={{ opacity: 0, y: 60 }} to={{ opacity: 1, y: 0 }} {...extra} />;

  return (
    <ReduceCtx.Provider value={reduce}>
      <ClickSpark sparkColor={YELLOW} sparkCount={10} sparkRadius={26} duration={600} disabled={reduce}>
        <div className="progress" aria-hidden="true" />
        <a className="skip" href="#main">Skip to content</a>

        <header className="top">
          <div className="wrap">
            <a className="brand" href="#/home"><img src={asset('icon.png')} alt="" width="30" height="30" />PrivCord</a>
            <nav className="links" aria-label="Main">
              {NAV.map(([to, t]) => <a key={to} href={`#/${to}`} aria-current={route === to ? 'page' : undefined}>{t}</a>)}
              <a className="btn primary small" href="#/download">Download</a>
            </nav>
            <details className="mnav" key={route}>
              <summary aria-label="Menu"><span aria-hidden="true">Menu</span></summary>
              <div className="mnav-panel">
                {[['home', 'Home'], ...NAV].map(([to, t]) => <a key={to} href={`#/${to}`} aria-current={route === to ? 'page' : undefined}>{t}</a>)}
              </div>
            </details>
          </div>
        </header>

        <main id="main" tabIndex={-1} ref={mainRef}>
          {route === 'home' && (
            <>
{/* ---------- hero: ink, with a springy dot grid the cursor flicks around ---------- */}
          <section className="hero" id="top" ref={heroRef}>
            {!reduce && (
              <div className="hero-bg" aria-hidden="true">
                <DotGrid dotSize={4} gap={22} baseColor="#2b2b2b" activeColor={YELLOW} proximity={150} shockRadius={340} shockStrength={5} glow={0} paused={!heroOn} />
              </div>
            )}
            <div className="wrap hero-grid">
              <div className="hero-copy">
                <h1 className="display">
                  <span className="row-line">{split('Chat privately.')}</span>
                  <span className="row-line"><mark>{split('No number, no account,')}</mark></span>
                  <span className="row-line">{split('no server reading along.', { delay: 18 })}</span>
                </h1>
                <p className="lead">PrivCord is a peer-to-peer messenger for Windows and Android. Chats go straight from your device to your friend's over an encrypted link, you add people with a QR code or six words, and it can keep going over Bluetooth with no internet.</p>
                <div className="cta">
                  <Magnet padding={80} magnetStrength={3} disabled={reduce}>
                    <StarBorder as="a" href="#/download" color={YELLOW} trailColor="#ffffff" backgroundColor={YELLOW} textColor={INK} borderColor="rgba(0,0,0,0)" duration={5} radius={10} className="cta-star">Download for Windows or Android</StarBorder>
                  </Magnet>
                  <a className="btn ghost" href={REPO}>View the source</a>
                </div>
                <p className="note">
                  {reduce ? 'End-to-end encrypted · source-available · not yet independently audited' : <ShinyText text="End-to-end encrypted · source-available · not yet independently audited" color="#8f8b82" shineColor="#ffffff" speed={3.5} />}
                </p>
              </div>
              <div className="hero-visual">
                <button type="button" className="win-btn" onClick={(e) => openShot('group-chat-dark.webp', e.currentTarget)} aria-label="Open the Windows screenshot full size">
                  <WindowFrame src={asset('group-chat-dark.webp')} alt="PrivCord on Windows: an encrypted group chat in the dark theme" w={1280} h={800} enlarge eager />
                </button>
                <button type="button" className="phone-btn" onClick={(e) => openShot('mobile-group.webp', e.currentTarget)} aria-label="Open the Android screenshot full size">
                  <PhoneFrame src={asset('mobile-group.webp')} alt="PrivCord on Android: a group chat" w={390} h={800} />
                </button>
              </div>
            </div>
          </section>

{/* ---------- yellow marquee that speeds up and skews with scroll ---------- */}
          <section className="band" aria-label="What PrivCord does not ask of you">
            {reduce ? (
              <ul className="band-static"><li>No phone number</li><li>No account</li><li>No server reading along</li></ul>
            ) : (
              <ScrollVelocity texts={['NO PHONE NUMBER • NO ACCOUNT •', 'NO SERVER READING ALONG • NO TRACKING •']} velocity={60} className="vel" numCopies={5} skewWithVelocity={4} outlineAlternate />
            )}
          </section>

{/* ---------- statement on paper ---------- */}
          <section className="statement">
            <div className="wrap narrow">
              {reduce ? (
                <p className="big">Your messages go from your device straight to your friend's. There is no company in the middle, no phone number to hand over, and no account to lose.</p>
              ) : (
                <ScrollReveal baseOpacity={0.14} enableBlur blurStrength={6} baseRotation={2} textClassName="big" containerClassName="statement-box">
                  Your messages go from your device straight to your friend's. There is no company in the middle, no phone number to hand over, and no account to lose.
                </ScrollReveal>
              )}
            </div>
          </section>

          {/* ---------- home: a short taste, then doors to the other pages ---------- */}
          <section className="doors" aria-labelledby="doors-h">
            <div className="wrap">
              <h2 id="doors-h">Find what you came for</h2>
              <ul className="door-grid">
                {DOORS.map(([to, t, p]) => (
                  <li key={to}><a className="door" href={`#/${to}`}><strong>{t}</strong><span>{p}</span><b aria-hidden="true">→</b></a></li>
                ))}
              </ul>
            </div>
          </section>
            </>
          )}
          {route === 'features' && (
            <>
{/* ---------- features ---------- */}
          <section id="features" className="features">
            <div className="wrap">
              <div className="sec-head">
                {reduce ? <h1>Everything a chat app needs, nothing that watches you</h1> : <ScrollFloat containerClassName="sf" textClassName="sf-text" splitBy="words" stagger={0.06}>Everything a chat app needs, nothing that watches you</ScrollFloat>}
                <p className="sub">Built for people who want private conversations without handing over an identity.</p>
              </div>
              <div className="bento">
                {FEATURES.map((f, n) => (
                  <Anim key={f.t} className={`bcell ${f.cls}`} distance={70} direction="vertical" duration={0.9} delay={(n % 3) * 0.08} scale={0.96} blur={6}>
                    <SpotlightCard className="fcard" spotlightColor="rgba(255, 229, 0, 0.2)" spotlightSize={260} intensity={0.25} theme="dark">
                      <h3><Icon d={P[f.i]} />{f.t}{f.tag ? <span className="tag">{f.tag}</span> : null}</h3>
                      <p>{f.p}</p>
                    </SpotlightCard>
                  </Anim>
                ))}
              </div>
            </div>
          </section>

            </>
          )}
          {route === 'how' && (
            <>
{/* ---------- how it works: sticky stacking cards on paper ---------- */}
          <section id="how" className="how">
            <div className="wrap how-grid">
              <div className="how-lhs">
                <h1>Up and running in three steps</h1>
                <p className="sub">No sign-up form. Your identity is made on your device.</p>
              </div>
              <ol className="stack">
                {STEPS.map(([t, p], n) => (
                  <li className={`stack-card c${n}`} key={t} style={{ '--n': n }}>
                    <div className="stack-head"><span className="stack-n" aria-hidden="true">{n + 1}</span><h3>{t}</h3></div>
                    <p>{p}</p>
                  </li>
                ))}
              </ol>
            </div>
          </section>

            </>
          )}
          {route === 'screens' && (
            <>
{/* ---------- screens: a rail of real screenshots and a lightbox ---------- */}
          <Gallery asset={asset} />

            </>
          )}
          {route === 'compare' && (
            <>
{/* ---------- compare (paper) ---------- */}
          <section id="compare" className="compare">
            <div className="wrap">
              <h1>How PrivCord compares</h1>
              <p className="sub">A short, good-faith summary. It can go out of date, so check each project for its current state.</p>
              <Fade blur duration={1} className="tablewrap-wrap">
                <div className="tablewrap">
                  <table>
                    <thead><tr><th></th><th>PrivCord</th><th>Signal</th><th>Session</th><th>Briar</th></tr></thead>
                    <tbody>{COMPARE.map((r) => <tr key={r[0]}>{r.map((c, i) => <td key={i}>{c}</td>)}</tr>)}</tbody>
                  </table>
                </div>
              </Fade>
              <p className="fine">PrivCord is a young project by one maintainer. If you need a battle-tested, audited messenger today, use one of the audited ones.</p>
            </div>
          </section>

            </>
          )}
          {route === 'faq' && (
            <>
{/* ---------- faq ---------- */}
          <section id="faq" className="faq-sec">
            <div className="wrap">
              <h1>Questions people ask</h1>
              <div className="faq">
                {FAQ.map(([q, a], n) => (
                  <Anim key={q} distance={30} duration={0.6} delay={Math.min(n, 3) * 0.05}>
                    <details><summary>{q}</summary><p>{a}</p></details>
                  </Anim>
                ))}
              </div>
            </div>
          </section>

            </>
          )}
          {route === 'download' && (
            <>
{/* ---------- download: yellow ---------- */}
          <section id="download" className="download" ref={dlRef}>
            {!reduce && <div className="dl-bg" aria-hidden="true"><DotGrid dotSize={4} gap={22} baseColor="#2b2b2b" activeColor={YELLOW} proximity={150} shockRadius={340} shockStrength={5} glow={0} paused={!dlOn} /></div>}
            <div className="wrap">
              <h1>Get PrivCord</h1>
              <p className="sub">Free. Pick your platform.</p>
              <div className="dl">
                <div className="dl-row">
                  <div><h3><Icon d={P.mon} />Windows</h3><p>Installer for Windows 10 and 11. The latest build is attached to the newest release.</p></div>
                  <Magnet padding={60} magnetStrength={3} disabled={reduce}><StarBorder as="a" href={`${REPO}/releases/latest`} color={YELLOW} trailColor="#ffffff" backgroundColor={YELLOW} textColor={INK} borderColor="rgba(0,0,0,0)" duration={5} radius={10} className="cta-star">Download the installer</StarBorder></Magnet>
                </div>
                <div className="dl-row">
                  <div><h3><Icon d={P.ph} />Android</h3><p>Install the arm64 APK (almost every phone); the armeabi-v7a one is for old 32-bit phones.</p></div>
                  <Magnet padding={60} magnetStrength={3} disabled={reduce}><StarBorder as="a" href={`${REPO}/releases/latest`} color={YELLOW} trailColor="#ffffff" backgroundColor={YELLOW} textColor={INK} borderColor="rgba(0,0,0,0)" duration={5} radius={10} className="cta-star">Download the APK</StarBorder></Magnet>
                </div>
              </div>
              <p className="warn"><strong>Heads up:</strong> the Windows installer is not code-signed yet, so Windows SmartScreen will warn you: choose <em>More info</em>, then <em>Run anyway</em>. On Android you need to allow installing apps from this source. Windows will also ask to allow the firewall the first time, which lets PrivCord find friends on your local network.</p>
            </div>
          </section>
            </>
          )}
        </main>

        <footer>
          <div className="wrap foot">
            <div>© 2026 PrivCord · <a href={`${REPO}/blob/main/LICENSE`}>PC-NC licence (source-available)</a> · Fonts: Bricolage Grotesque, Figtree (OFL) · Motion components: <a href="https://reactbits.dev">React Bits</a></div>
            <nav aria-label="Footer">
              <a href={REPO}>GitHub</a>
              <a href={`${REPO}/blob/main/SECURITY.md`}>Security</a>
              <a href={`${REPO}/blob/main/docs/KNOWN_LIMITATIONS.md`}>Known limitations</a>
              <a href={`${REPO}/issues`}>Report an issue</a>
            </nav>
          </div>
        </footer>
      </ClickSpark>
    </ReduceCtx.Provider>
  );
}
