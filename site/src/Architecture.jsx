import { createPortal } from 'react-dom';
import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react';
import { Head, Next } from './Pages.jsx';
import DotGrid from './rb/DotGrid.jsx';

/*
 * The architecture page: an interactive map of how PrivCord is built. Layers as rows, parts as buttons, edges drawn between
 * them from measured positions (so it works at any width), a stepper that sends a "packet" along the real path of a message,
 * the crates with their size and tests, the steps taken to get here, and the design decisions. Everything is plain React and
 * SVG, honours prefers-reduced-motion, and every control is a native button.
 */

const LAYERS = [
  { id: 'ui', label: 'Apps you see', hue: 48 },
  { id: 'bridge', label: 'Bridges', hue: 198 },
  { id: 'engine', label: 'The shared Rust engine', hue: 150 },
  { id: 'ports', label: 'Ports', hue: 280 },
  { id: 'adapters', label: 'Platform adapters', hue: 24 },
  { id: 'world', label: 'The outside world', hue: 0 },
];

const NODES = [
  { id: 'win', layer: 'ui', label: 'Windows app', sub: 'Tauri + React', what: 'The Windows app is a small Tauri shell around a React screen layer. It draws what the engine reports and sends what you press. It keeps no business state of its own.', where: 'apps/desktop (screens, plain CSS) and apps/desktop/src-tauri (tray, hotkey, notifications)', facts: ['Screens only: chat list, chat, settings, onboarding', 'App lock with a Windows Hello passkey', 'Closing the window keeps the engine running in the tray'], limit: 'The hotkey, taskbar badge and toasts have been built and tested in CI, not yet on real Windows hardware.' },
  { id: 'and', layer: 'ui', label: 'Android app', sub: 'Jetpack Compose', what: 'The Android app is Jetpack Compose screens on top of one adapter (RustChatEngine) that talks to the same engine. Notifications, biometrics and the background service are Android code.', where: 'apps/android', facts: ['Biometric app lock', 'Foreground service keeps delivery alive on strict phones', 'Voice notes, camera QR scanning, Bluetooth mesh'], limit: 'Kotlin compiles only in CI, and the app has not been run on many real phones yet.' },
  { id: 'host', layer: 'bridge', label: 'Desktop host', sub: 'JSON commands and events', what: 'The engine runs inside the Tauri shell. The page sends JSON commands (send a message, take a snapshot, set a preference) and receives numbered events back. Native signaling, LAN discovery and automatic backups live here.', where: 'crates/privcord-desktop-host', facts: ['About 1,650 lines of Rust', 'A two-host test boots two engines and talks between them', 'Commands are checked by name; an unknown one is an error'], limit: 'Windows only. Linux and macOS are not targets.' },
  { id: 'ffi', layer: 'bridge', label: 'UniFFI facade', sub: 'Kotlin bindings', what: 'A narrow facade of plain data, stable error codes and callback traits. The Kotlin bindings are generated from it and committed. CI regenerates them and fails if they differ, so the two sides cannot drift apart.', where: 'crates/privcord-ffi', facts: ['About 2,270 lines of Rust', 'The only crate allowed to use unsafe code, and only for the FFI boundary', 'Callbacks carry notifications and platform requests'], limit: 'A binding mismatch shows up only when Android builds, which is why CI builds it every time.' },
  { id: 'core', layer: 'engine', label: 'Engine core', sub: 'identity, delivery, sync', what: 'One implementation of PrivCord’s rules for both apps: identity and contacts, authenticated links, messaging with an outbox and retries, groups, files, channels, linked devices, and preferences that sync between your devices. It owns its runtime thread and talks in numbered events through bounded queues. A reader that falls behind gets "Lagged" and asks for a snapshot.', where: 'crates/privcord-engine', facts: ['About 13,900 lines of source and 8,200 lines of tests', 'Mock-adapter integration tests, crash and restart tests, three-engine tests', 'Settings are last-writer-wins entries in a sync log'], limit: 'A snapshot returns every message, so its cost grows with history (about 4 ms at 4,000 messages). Paging is the next step.' },
  { id: 'crypto', layer: 'engine', label: 'Crypto', sub: 'ML-KEM-768 + P-256', what: 'Identity from a 12-word phrase, signatures, key agreement, sealed blobs, contact cards, safety numbers, invites, backups, and the secure channel: a hybrid ML-KEM-768 plus P-256 handshake, a per-frame key ratchet and padding. Wire-compatible with the older TypeScript through shared test vectors.', where: 'crates/privcord-crypto', facts: ['About 10,400 lines of source and 2,400 lines of tests', 'Backups use Argon2id with a 12-character minimum', 'Fuzz targets cover everything that reads untrusted input'], limit: 'Not independently audited yet. This is the biggest open gap.' },
  { id: 'domain', layer: 'engine', label: 'Domain rules', sub: 'pure, no platform', what: 'Pure types and rules with no platform code: message ordering, delivery states, the connection state machine, privacy rules, file chunking, mesh relay logic, channels and network settings. Cargo enforces that it depends on nothing platform-specific.', where: 'crates/privcord-domain', facts: ['About 3,700 lines of source and 1,400 lines of tests', 'No unwrap, expect or panic in library code (lint rule)', 'Shared by crypto, ports and the engine'], limit: 'None known beyond the project-wide ones.' },
  { id: 'ports', layer: 'ports', label: 'Ports', sub: 'five small traits', what: 'Everything the engine needs from outside is a trait: Storage, SecureStore, Transport, Clock and Notifier. The engine never imports a database or a radio. Tests plug in in-memory fakes, so the real identity, crypto and delivery logic run with only the disk and the network faked.', where: 'crates/privcord-ports', facts: ['About 700 lines', 'Fakes live behind a "testing" feature', 'This is the seam that makes the engine testable and portable'], limit: 'The test storage fake copies its data on every write, so it is not a performance guide.' },
  { id: 'sqlite', layer: 'adapters', label: 'SQLite store', sub: 'encrypted, versioned', what: 'The Storage port on SQLite with SQLCipher: every page is encrypted, the schema is versioned, writes are atomic batches, the file is integrity-checked on open and copied before a migration. Deleted rows are overwritten (secure_delete).', where: 'crates/privcord-storage', facts: ['Measured: 20,000 messages inserted in 0.33 s', 'Writing one message takes about 0.18 ms', 'Loading 20,000 messages at startup takes about 30 ms'], limit: 'Durability is set to FULL (an fsync per commit): safe, slightly slower on some phones.' },
  { id: 'keystore', layer: 'adapters', label: 'Secure store', sub: 'Keystore / Credential Manager', what: 'The SecureStore port on the operating system’s own vault: Android Keystore and the Windows Credential Manager hold the database key and other secrets, so they are never written next to the data they protect.', where: 'platform code in apps/android and apps/desktop/src-tauri', facts: ['The database key never leaves the vault in plain form on disk', 'Windows Hello and Android biometrics gate the app, not the key'], limit: 'App lock keeps other people out of an unlocked device. It is not protection against someone who copies the files and the vault.' },
  { id: 'webrtc', layer: 'adapters', label: 'WebRTC + signaling', sub: 'direct links', what: 'The Transport port for the internet: WebRTC data channels between devices, set up by sealed offers and answers that travel through signaling relays. STUN finds your address, TURN relays encrypted traffic when a direct path fails.', where: 'apps/desktop/src (browser WebRTC) and apps/android (libwebrtc)', facts: ['Messages go device to device once a link is up', 'Every link is checked against the keys pinned when you added the friend', 'Files up to 8 MB go over the same link'], limit: 'Relays, STUN and TURN servers can see connection details such as addresses, never message content.' },
  { id: 'ble', layer: 'adapters', label: 'Bluetooth LE', sub: 'Android, experimental', what: 'A Bluetooth Low Energy transport with its own forward-secret channel, and an optional mesh that passes sealed messages through nearby phones, up to 7 hops, so chats can continue with no internet.', where: 'apps/android and crates/privcord-domain (mesh rules)', facts: ['Mesh relay logic is pure and simulated at scale in tests', 'Channel messages carry a hop count shown in the chat'], limit: 'Tested in simulation only. It has never run on real radios.' },
  { id: 'relay', layer: 'world', label: 'Signaling relays', sub: 'Nostr or your own', what: 'Public Nostr relays or a relay you run carry sealed, signed blobs to hashed mailboxes so two devices can find each other. An optional store-and-forward mode lets a sealed copy of a message wait for an offline friend.', where: 'relay/ (a small self-hostable relay) and the Nostr adapter', facts: ['A relay sees sizes, timing and the addresses that talk to it', 'It never sees names, keys or message text'], limit: 'Without any relay, two devices on different networks cannot find each other (same Wi-Fi and Bluetooth still work).' },
  { id: 'stun', layer: 'world', label: 'STUN / TURN', sub: 'optional helpers', what: 'STUN tells a device its public address. TURN relays already-encrypted traffic when two networks cannot connect directly, which is common on mobile carriers.', where: 'docs/turn-server has a ready kit', facts: ['TURN sees who talks to whom, never content', '"Hide my IP" and Tor mode send links through a relay only'], limit: 'Strict networks need a TURN server you configure.' },
  { id: 'peer', layer: 'world', label: 'Another device', sub: 'a friend, or your own', what: 'The other end of every link: a friend’s device or your own second device. Each side proves its keys match the ones pinned when you added each other. Nobody else holds your messages.', where: 'the same engine, on their device', facts: ['Messages wait on your device while they are offline', 'Linking your own second device uses a QR code and six digits'], limit: 'A removed device keeps what it already received.' },
];

const PAIRS = [['win', 'host'], ['and', 'ffi'], ['host', 'core'], ['ffi', 'core'], ['core', 'crypto'], ['core', 'domain'], ['core', 'ports'], ['ports', 'sqlite'], ['ports', 'keystore'], ['ports', 'webrtc'], ['ports', 'ble'], ['webrtc', 'relay'], ['webrtc', 'stun'], ['webrtc', 'peer'], ['relay', 'peer'], ['ble', 'peer']];

const SCENARIOS = [
  { id: 'send', title: 'Send a message', steps: [
    ['win', 'You press send in the app.'],
    ['host', 'The host turns it into one JSON command.'],
    ['core', 'The engine stores the message and queues it for each recipient.'],
    ['crypto', 'The link’s key seals it, and it is padded to hide its length.'],
    ['core', 'The engine hands the sealed frame to the transport.'],
    ['ports', 'Through the Transport port. The engine does not know what carries it.'],
    ['webrtc', 'It crosses the open WebRTC link.'],
    ['peer', 'Your friend’s engine opens it, checks it came from the pinned key, and acknowledges.'],
  ] },
  { id: 'friend', title: 'Add a friend', steps: [
    ['and', 'You show a QR code or read out six words.'],
    ['ffi', 'The app calls the engine through the UniFFI facade.'],
    ['core', 'The engine makes your contact card and a short-lived mailbox from the words.'],
    ['crypto', 'The card is sealed so only the friend’s device can read it.'],
    ['core', 'The engine asks the transport to publish it.'],
    ['ports', 'Through the Transport port.'],
    ['webrtc', 'The adapter posts the sealed blob.'],
    ['relay', 'A relay holds it for the hashed mailbox. It cannot read it.'],
    ['peer', 'Your friend fetches it, answers, and you confirm who answered with safety numbers.'],
  ] },
  { id: 'link', title: 'Link a second device', steps: [
    ['win', 'The new device shows a QR code. It has no account yet.'],
    ['host', 'The host passes the request to the engine.'],
    ['core', 'The engine on your phone holds the account key and shows six digits after scanning.'],
    ['crypto', 'The grant is sealed to the new device and held until the digits match.'],
    ['core', 'You type the six digits on the new device. Three tries.'],
    ['ports', 'The sealed grant travels through the Transport port.'],
    ['webrtc', 'The adapter delivers it.'],
    ['relay', 'Via the relay, which sees only a sealed blob.'],
    ['peer', 'Your phone confirms. The account key never left your first device.'],
  ] },
  { id: 'save', title: 'Keep it safe on disk', steps: [
    ['core', 'The engine writes every change in one atomic batch.'],
    ['ports', 'Through the Storage port.'],
    ['keystore', 'The operating system’s vault gives the database key.'],
    ['ports', 'Back to the port with the key.'],
    ['sqlite', 'Every page is encrypted. The file is checked on open and copied before a migration.'],
  ] },
  { id: 'ble', title: 'No internet', steps: [
    ['and', 'You send a message with no connection.'],
    ['ffi', 'The same facade, the same engine.'],
    ['core', 'The engine queues it and notices a nearby device.'],
    ['ports', 'The Transport port offers the Bluetooth link.'],
    ['ble', 'Sealed frames hop through nearby phones, up to 7 hops (experimental).'],
    ['peer', 'It arrives with no server involved.'],
  ] },
];

const CRATES = [
  { name: 'privcord-engine', src: 13909, tests: 8173, what: 'The engine: identity, delivery, groups, files, channels, devices, sync.' },
  { name: 'privcord-crypto', src: 10411, tests: 2430, what: 'Keys, signatures, sealed blobs, the secure channel, backups.' },
  { name: 'privcord-domain', src: 3725, tests: 1383, what: 'Pure rules and types with no platform code.' },
  { name: 'privcord-ffi', src: 2273, tests: 501, what: 'The narrow facade the Android app calls.' },
  { name: 'privcord-desktop-host', src: 1649, tests: 426, what: 'The engine inside the Windows shell.' },
  { name: 'privcord-import', src: 1151, tests: 738, what: 'Imports the older desktop and Android databases.' },
  { name: 'privcord-storage', src: 824, tests: 559, what: 'The encrypted SQLite store.' },
  { name: 'privcord-ports', src: 705, tests: 0, what: 'The five traits and their test fakes.' },
];

const STEPS_TAKEN = [
  { t: 'Cut the cloud', d: 'Removed Firebase and every account concept. Identity became a 12-word phrase made on the device, so there is nothing to sign up for and nobody holds your account.', why: 'The point of the product: no company in the middle.' },
  { t: 'Find each other without a server that reads', d: 'Sealed, signed offers and answers through signaling relays (Nostr or your own), STUN and TURN for hard networks, and an honest "how is this connected" report in the chat.', why: 'Direct links need a way to meet. The meeting point must not see content.' },
  { t: 'Groups and introductions', d: 'A group is a signed event log every member holds. Add Ben, Ben adds Cat, and you and Cat connect automatically. Capped at 10 so every member can link to every other.', why: 'No coordinator, so it has to be a mesh of direct links.' },
  { t: 'Everyday messaging', d: 'Replies, reactions, search, delete for everyone, files up to 8 MB, image and video previews, voice notes, six-word invites, app lock.', why: 'A private app still has to be pleasant.' },
  { t: 'Offline', d: 'Open channels and a Bluetooth mesh that hops through nearby phones, up to 7 hops.', why: 'Chats should survive having no internet. It is experimental and simulated only.' },
  { t: 'Post-quantum channel', d: 'A hybrid ML-KEM-768 plus P-256 handshake, a per-frame key ratchet, padding, and signed one-time prekeys so offline messages are safe.', why: 'Messages recorded today should not be readable by tomorrow’s computers.' },
  { t: 'Audit, fix, fuzz', d: 'A five-reviewer audit, fixes for each finding, fuzz targets in CI for every parser, "hide my IP" and Tor modes, Argon2id backups.', why: 'Find the problems ourselves before anyone else does, and keep finding them.' },
  { t: 'One engine in Rust', d: 'Moved the rules out of duplicated TypeScript and Kotlin into one Rust engine: domain, crypto, ports, engine, storage. The Windows app hosts it through JSON, Android calls it through UniFFI, shared vectors keep the wire format identical, importers carried existing data over.', why: 'Two copies of the rules drift apart. One copy cannot.' },
  { t: 'Many devices', d: 'Per-device addresses and keys, device lists, removal with a 24-hour delay (or at once with the phrase), key rotation, and linking by scan: QR on the new device, six digits typed on it.', why: 'People own more than one device, and losing one must not lose the account.' },
  { t: 'Polish in the shared engine', d: 'Pins, archive, mute-for-a-while, quiet hours, silent chats, undo send, privacy checkup, delivery details per device: all computed once in the engine and drawn by both apps.', why: 'Build a feature once and it arrives on both platforms identical.' },
  { t: 'Tell the truth', d: 'A known-limitations file, a website that says what is not done, a pipeline that fails loudly, and an independent audit as the next big step.', why: 'Trust comes from admitting what is unfinished.' },
];

const DECISIONS = [
  ['One engine, many screens', 'All rules live in one Rust engine. The screens are thin. A fix or feature lands on both platforms at once.'],
  ['Ports and adapters', 'The engine asks for storage, transport, a clock, a vault and notifications through five traits, so tests run the real logic with fakes and a new platform needs only adapters.'],
  ['Seal everything that leaves', 'Relays, STUN and TURN are optional helpers that see addresses and timing but never content or names.'],
  ['Wire compatibility is tested', 'Shared test vectors prove the Rust and the older TypeScript and Kotlin agree byte for byte.'],
  ['Rules enforced by tooling', 'No unsafe except at the FFI edge, no unwrap or panic in library code, dependency direction checked by cargo, bindings and the migration map checked in CI.'],
  ['Slow readers never block', 'Events are numbered and queues are bounded. A reader that falls behind asks for a snapshot instead of stalling the engine.'],
  ['Be honest', 'Known limitations are written down, the Bluetooth mesh is labelled experimental, and "not audited" is on the front page.'],
];

const CHECKS = [
  ['Rust engine', 'format, strict lint, all tests, shared vectors, dependency audit, on Linux and Windows'],
  ['Fuzzing', 'every parser of untrusted input, a minute each'],
  ['End-to-end', 'real browsers, relay, Nostr and TURN, 21 scenarios'],
  ['Android', 'unit tests, release APK, and a launch on an emulator'],
  ['Windows', 'the installer builds, including the Bluetooth code'],
  ['Bindings and map', 'generated Kotlin bindings and the migration map must match what is committed'],
];

const reduced = () => typeof matchMedia === 'function' && matchMedia('(prefers-reduced-motion: reduce)').matches;
const NODE = Object.fromEntries(NODES.map((n) => [n.id, n]));
const edgeKey = (a, b) => (a < b ? `${a}|${b}` : `${b}|${a}`);

function Graph() {
  const wrap = useRef(null);
  const nodeEls = useRef({});
  const pathEls = useRef({});
  const info = useRef({});
  const dot = useRef(null);
  const svg = useRef(null);
  const cardRef = useRef(null);
  const [sel, setSel] = useState('core');
  const [sc, setSc] = useState(0);
  const [step, setStep] = useState(0);
  const [play, setPlay] = useState(() => !reduced());
  const [seen, setSeen] = useState(false);
  const [hold, setHold] = useState(false);
  const [pop, setPop] = useState(null);
  const [modal, setModal] = useState(null); // { id, from: {x,y} of the clicked part }
  const [closing, setClosing] = useState(false);
  const scenario = SCENARIOS[sc];
  const current = scenario.steps[step][0];
  const prev = step > 0 ? scenario.steps[step - 1][0] : null;
  const running = play && seen && !hold;

  // Edges are redrawn from the measured positions of the parts, so they follow the parts as they float and drift.
  const measure = useCallback(() => {
    const box = wrap.current?.getBoundingClientRect();
    if (!box) return;
    if (svg.current) {
      svg.current.setAttribute('width', box.width.toFixed(0));
      svg.current.setAttribute('height', box.height.toFixed(0));
    }
    const rect = (id) => {
      const r = nodeEls.current[id]?.getBoundingClientRect();
      return r ? { x: r.left - box.left + r.width / 2, top: r.top - box.top, bottom: r.bottom - box.top, mid: r.top - box.top + r.height / 2 } : null;
    };
    for (const [a, b] of PAIRS) {
      const ra = rect(a);
      const rb = rect(b);
      const el = pathEls.current[edgeKey(a, b)];
      if (!ra || !rb || !el) continue;
      const aFirst = ra.mid <= rb.mid;
      const [ua, ura, ub, urb] = aFirst ? [a, ra, b, rb] : [b, rb, a, ra];
      const dy = Math.max(24, (urb.top - ura.bottom) / 2);
      info.current[edgeKey(a, b)] = { a: ua, b: ub };
      el.setAttribute('d', `M ${ura.x.toFixed(1)} ${ura.bottom.toFixed(1)} C ${ura.x.toFixed(1)} ${(ura.bottom + dy).toFixed(1)}, ${urb.x.toFixed(1)} ${(urb.top - dy).toFixed(1)}, ${urb.x.toFixed(1)} ${urb.top.toFixed(1)}`);
    }
  }, []);

  useLayoutEffect(() => {
    measure();
    const ro = typeof ResizeObserver === 'function' ? new ResizeObserver(measure) : null;
    if (ro && wrap.current) ro.observe(wrap.current);
    addEventListener('resize', measure);
    return () => { ro?.disconnect(); removeEventListener('resize', measure); };
  }, [measure]);

  useEffect(() => {
    const el = wrap.current;
    if (!el || typeof IntersectionObserver !== 'function') { setSeen(true); return undefined; }
    const io = new IntersectionObserver(([e]) => setSeen(e.isIntersecting), { threshold: 0.05 });
    io.observe(el);
    return () => io.disconnect();
  }, []);

  // While the map is on screen the parts float, so the edges are redrawn every frame (15 rectangles, cheap).
  useEffect(() => {
    if (!seen || reduced()) return undefined;
    let raf = 0;
    const loop = () => { measure(); raf = requestAnimationFrame(loop); };
    raf = requestAnimationFrame(loop);
    return () => cancelAnimationFrame(raf);
  }, [seen, measure]);

  // Near the pointer the parts lean towards it a little and grow; away from it they settle back.
  useEffect(() => {
    const box = wrap.current;
    if (!box || reduced()) return undefined;
    const move = (e) => {
      if (e.pointerType !== 'mouse') return;
      for (const el of Object.values(nodeEls.current)) {
        if (!el) continue;
        const r = el.getBoundingClientRect();
        const dx = e.clientX - (r.left + r.width / 2);
        const dy = e.clientY - (r.top + r.height / 2);
        const d = Math.hypot(dx, dy);
        const k = Math.max(0, 1 - d / 190);
        el.style.setProperty('--mx', `${(dx * 0.16 * k).toFixed(1)}px`);
        el.style.setProperty('--my', `${(dy * 0.16 * k).toFixed(1)}px`);
        el.style.setProperty('--s', (1 + k * Math.min(0.07, 14 / r.width)).toFixed(3));
      }
    };
    const leave = () => {
      for (const el of Object.values(nodeEls.current)) {
        el?.style.setProperty('--mx', '0px');
        el?.style.setProperty('--my', '0px');
        el?.style.setProperty('--s', '1');
      }
    };
    box.addEventListener('pointermove', move);
    box.addEventListener('pointerleave', leave);
    return () => { box.removeEventListener('pointermove', move); box.removeEventListener('pointerleave', leave); };
  }, []);

  // Advance the stepper while it is playing, on screen and not being touched.
  useEffect(() => {
    if (!running) return undefined;
    const t = setTimeout(() => {
      if (step + 1 < scenario.steps.length) setStep(step + 1);
      else { setSc((sc + 1) % SCENARIOS.length); setStep(0); }
    }, 2600);
    return () => clearTimeout(t);
  }, [running, step, sc, scenario.steps.length]);

  // The panel follows the packet while it plays.
  useEffect(() => { if (running) setSel(current); }, [running, current]);

  // Move the packet along the real edge between the previous and the current part.
  useEffect(() => {
    const d = dot.current;
    if (!d) return undefined;
    d.style.opacity = '0';
    if (!prev || reduced()) return undefined;
    const key = edgeKey(prev, current);
    const t0 = performance.now();
    let raf = 0;
    const tick = (now) => {
      const p = pathEls.current[key];
      const edge = info.current[key];
      if (!p || !edge || typeof p.getTotalLength !== 'function') return;
      const len = p.getTotalLength();
      const t = Math.min(1, (now - t0) / 1100);
      const e = 1 - Math.pow(1 - t, 3);
      const pt = p.getPointAtLength((edge.a === prev ? e : 1 - e) * len);
      d.setAttribute('cx', pt.x.toFixed(1));
      d.setAttribute('cy', pt.y.toFixed(1));
      d.style.opacity = '1';
      if (t < 1) raf = requestAnimationFrame(tick);
    };
    raf = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(raf);
  }, [prev, current]);

  const pick = (id, el) => {
    setSel(id);
    setPlay(false);
    setPop(id);
    setTimeout(() => setPop((p) => (p === id ? null : p)), 700);
    const r = el?.getBoundingClientRect();
    setClosing(false);
    setModal({ id, from: r ? { x: r.left + r.width / 2, y: r.top + r.height / 2 } : null });
  };
  const closeModal = useCallback(() => {
    if (reduced()) { setModal(null); return; }
    setClosing(true);
    setTimeout(() => { setModal(null); setClosing(false); }, 240);
  }, []);
  const openFrom = modal?.id;

  // The card grows out of the part that was clicked: its transform origin is that part's centre.
  useLayoutEffect(() => {
    const c = cardRef.current;
    if (!c || !modal?.from) return;
    const r = c.getBoundingClientRect();
    c.style.transformOrigin = `${(modal.from.x - r.left).toFixed(0)}px ${(modal.from.y - r.top).toFixed(0)}px`;
  }, [openFrom]); // eslint-disable-line react-hooks/exhaustive-deps

  useEffect(() => {
    if (!modal) return undefined;
    const key = (e) => { if (e.key === 'Escape') closeModal(); };
    addEventListener('keydown', key);
    const prevOverflow = document.body.style.overflow;
    document.body.style.overflow = 'hidden';
    cardRef.current?.querySelector('.close')?.focus();
    return () => { removeEventListener('keydown', key); document.body.style.overflow = prevOverflow; };
  }, [!!modal, closeModal]); // eslint-disable-line react-hooks/exhaustive-deps

  const go = (i) => { setStep(i); setPlay(false); };
  const node = NODE[modal?.id || sel];
  const linked = useMemo(() => new Set(PAIRS.filter(([a, b]) => a === sel || b === sel).flatMap(([a, b]) => [a, b])), [sel]);
  const visited = useMemo(() => new Set(scenario.steps.slice(0, step + 1).map((s) => s[0])), [scenario, step]);

  return (
    <div className="arch" onPointerEnter={(e) => { if (e.pointerType === 'mouse') setHold(true); }} onPointerLeave={() => setHold(false)}>
      <div className="arch-body">
        <div className="arch-graph" ref={wrap}>
          <svg className="arch-edges" ref={svg} width="1" height="1" aria-hidden="true">
            {PAIRS.map(([a, b]) => {
              const key = edgeKey(a, b);
              const onStep = prev && key === edgeKey(prev, current);
              const touched = a === sel || b === sel;
              return <path key={key} ref={(e) => { pathEls.current[key] = e; }} d="M0 0" className={`edge${touched ? ' touched' : ''}${onStep ? ' live' : ''}`} />;
            })}
            <circle ref={dot} className="packet" r="7" cx="0" cy="0" style={{ opacity: 0 }} />
          </svg>
          {LAYERS.map((l) => (
            <section key={l.id} className="arch-layer" style={{ '--h': l.hue }} aria-label={l.label}>
              <h3><i aria-hidden="true" />{l.label}</h3>
              <div className="arch-nodes">
                {NODES.filter((n) => n.layer === l.id).map((n, i) => (
                  <button
                    key={n.id}
                    type="button"
                    ref={(e) => { nodeEls.current[n.id] = e; }}
                    className={`arch-node${sel === n.id ? ' sel' : ''}${linked.has(n.id) && sel !== n.id ? ' linked' : ''}${current === n.id ? ' now' : ''}${visited.has(n.id) ? ' seen' : ''}${pop === n.id ? ' pop' : ''}`}
                    style={{ '--dur': `${4.6 + ((n.id.length * 7 + i * 3) % 9) * 0.35}s`, '--delay': `${-((n.id.charCodeAt(0) + i * 5) % 11) * 0.45}s` }}
                    aria-pressed={sel === n.id}
                    onClick={(e) => pick(n.id, e.currentTarget)}
                  >
                    <strong>{n.label}</strong>
                    <span>{n.sub}</span>
                  </button>
                ))}
              </div>
            </section>
          ))}
        </div>

      </div>

      <div className="arch-steps">
        <div className="arch-stepbar">
          <button type="button" className="round" aria-label="Previous step" onClick={() => go((step + scenario.steps.length - 1) % scenario.steps.length)}>←</button>
          <button type="button" className="round" aria-label="Next step" onClick={() => go((step + 1) % scenario.steps.length)}>→</button>
          <span className="arch-count">{scenario.title}: step {step + 1} of {scenario.steps.length}</span>
        </div>
        <ol className="arch-caps">
          {scenario.steps.map(([id, text], i) => (
            <li key={`${i}-${id}`} className={i === step ? 'on' : i < step ? 'done' : ''}>
              <button type="button" onClick={() => go(i)} aria-current={i === step ? 'step' : undefined}><b>{i + 1}</b><span><em>{NODE[id].label}</em>{text}</span></button>
            </li>
          ))}
        </ol>
      </div>

      {modal && createPortal(
        <div className={`arch-modal${closing ? ' out' : ''}`} onClick={closeModal}>
          <aside className="arch-card" ref={cardRef} role="dialog" aria-modal="true" aria-label={node.label} style={{ '--h': LAYERS.find((l) => l.id === node.layer)?.hue }} onClick={(e) => e.stopPropagation()}>
            <button type="button" className="close round" aria-label="Close" onClick={closeModal}>×</button>
            <div key={node.id} className="card-in">
              <p className="eyebrow">{LAYERS.find((l) => l.id === node.layer)?.label}</p>
              <h3>{node.label}</h3>
              <p>{node.what}</p>
              <ul>{node.facts.map((f) => <li key={f}>{f}</li>)}</ul>
              <p className="where"><b>Where:</b> {node.where}</p>
              <p className="limit"><b>Honest limit:</b> {node.limit}</p>
              <div className="hop">
                <span>Connected to</span>
                {[...linked].filter((id) => id !== node.id).map((id) => (
                  <button key={id} type="button" className="chip" onClick={() => { setSel(id); setModal((m) => ({ ...m, id })); }}>{NODE[id].label}</button>
                ))}
              </div>
              <button type="button" className="btn-close" onClick={closeModal}>Close</button>
            </div>
          </aside>
        </div>,
        document.body,
      )}
    </div>
  );
}

function Crates() {
  const ref = useRef(null);
  const [on, setOn] = useState(() => reduced() || typeof IntersectionObserver !== 'function');
  useEffect(() => {
    const el = ref.current;
    if (!el || on) return undefined;
    const io = new IntersectionObserver(([e]) => { if (e.isIntersecting) { setOn(true); io.disconnect(); } }, { threshold: 0.2 });
    io.observe(el);
    return () => io.disconnect();
  }, [on]);
  const max = Math.max(...CRATES.map((c) => c.src));
  const total = CRATES.reduce((s, c) => s + c.src, 0);
  const tests = CRATES.reduce((s, c) => s + c.tests, 0);
  return (
    <div className={`crates${on ? ' in' : ''}`} ref={ref}>
      <p className="sub">{total.toLocaleString('en-US')} lines of Rust, with {tests.toLocaleString('en-US')} lines of tests beside them. Bars show source (solid) and tests (outline).</p>
      <ul>
        {CRATES.map((c, i) => (
          <li key={c.name} style={{ '--i': i }}>
            <div className="crate-top"><code>{c.name}</code><span>{c.src.toLocaleString('en-US')} + {c.tests.toLocaleString('en-US')}</span></div>
            <div className="crate-bar" role="img" aria-label={`${c.name}: ${c.src} lines of source, ${c.tests} lines of tests`}>
              <i className="src" style={{ '--w': `${(c.src / max) * 100}%` }} />
              <i className="tst" style={{ '--w': `${(c.tests / max) * 100}%` }} />
            </div>
            <p>{c.what}</p>
          </li>
        ))}
      </ul>
    </div>
  );
}

function Timeline() {
  const [open, setOpen] = useState(0);
  const refs = useRef([]);
  const [shown, setShown] = useState(() => (reduced() || typeof IntersectionObserver !== 'function' ? new Set(STEPS_TAKEN.map((_, i) => i)) : new Set()));
  useEffect(() => {
    if (typeof IntersectionObserver !== 'function' || reduced()) return undefined;
    const io = new IntersectionObserver((entries) => {
      setShown((s) => { const n = new Set(s); for (const e of entries) if (e.isIntersecting) n.add(Number(e.target.dataset.i)); return n; });
    }, { threshold: 0.2 });
    refs.current.forEach((el) => el && io.observe(el));
    return () => io.disconnect();
  }, []);
  return (
    <ol className="timeline">
      {STEPS_TAKEN.map((s, i) => (
        <li key={s.t} data-i={i} ref={(e) => { refs.current[i] = e; }} className={`${shown.has(i) ? 'in' : ''}${open === i ? ' open' : ''}`}>
          <button type="button" aria-expanded={open === i} aria-controls={`step-${i}`} onClick={() => setOpen(open === i ? -1 : i)}>
            <b>{i + 1}</b><span>{s.t}</span><i aria-hidden="true">{open === i ? '−' : '+'}</i>
          </button>
          <div id={`step-${i}`} className="tl-wrap" inert={open !== i ? "" : undefined} aria-hidden={open !== i}><div className="tl-body"><p>{s.d}</p><p className="why"><b>Why:</b> {s.why}</p></div></div>
        </li>
      ))}
    </ol>
  );
}

/** Drifting dots across the whole page, with two slow light pools behind them (React Bits DotGrid plus CSS). */
function Backdrop() {
  const [on, setOn] = useState(true);
  useEffect(() => {
    const f = () => setOn(!document.hidden);
    document.addEventListener('visibilitychange', f);
    return () => document.removeEventListener('visibilitychange', f);
  }, []);
  const small = typeof matchMedia === 'function' && matchMedia('(max-width: 700px)').matches;
  return (
    <div className="arch-bg" aria-hidden="true">
      <i className="pool p1" /><i className="pool p2" />
      {!reduced() && <DotGrid dotSize={small ? 3 : 4} gap={small ? 30 : 24} baseColor="#2b2b2b" activeColor="#ffe500" proximity={150} shockRadius={340} shockStrength={5} glow={0} paused={!on} />}
    </div>
  );
}

export function ArchitecturePage({ Go, repo }) {
  return (
    <div className="arch-page">
      <Backdrop />
      <Head eyebrow="Architecture" title="How PrivCord is built" lead="One Rust engine, thin screens on top, five small ports underneath, and a handful of optional helpers in the world. Click a part, or let the packet show you the way." />
      <section className="page-sec">
        <div className="wrap">
          <h2 className="group-h">The map</h2>
          <Graph />
        </div>
      </section>
      <section className="page-sec">
        <div className="wrap">
          <h2 className="group-h">The crates</h2>
          <Crates />
        </div>
      </section>
      <section className="page-sec">
        <div className="wrap">
          <h2 className="group-h">The steps we took</h2>
          <Timeline />
        </div>
      </section>
      <section className="page-sec">
        <div className="wrap">
          <h2 className="group-h">Design decisions</h2>
          <div className="fgrid">
            {DECISIONS.map(([t, p]) => <article className="fcard plain" key={t}><h3>{t}</h3><p>{p}</p></article>)}
          </div>
        </div>
      </section>
      <section className="page-sec">
        <div className="wrap">
          <h2 className="group-h">What checks it</h2>
          <div className="fgrid">
            {CHECKS.map(([t, p]) => <article className="fcard plain" key={t}><h3>{t}</h3><p>{p}</p></article>)}
          </div>
          <p className="fine">More detail lives in the repository: <a href={`${repo}/blob/main/docs/ARCHITECTURE.md`}>docs/ARCHITECTURE.md</a>, <a href={`${repo}/tree/main/docs/rust`}>docs/rust</a> (including the decision records) and <a href={`${repo}/blob/main/docs/KNOWN_LIMITATIONS.md`}>the known limitations</a>.</p>
        </div>
      </section>
      <Next Go={Go} items={[['how', 'How it works'], ['features', 'Features'], ['faq', 'Questions']]} />
    </div>
  );
}
