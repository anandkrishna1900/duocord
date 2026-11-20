import Gallery, { SHOTS } from './Gallery.jsx';
import DotGrid from './rb/DotGrid.jsx';
import Magnet from './rb/Magnet.jsx';
import StarBorder from './rb/StarBorder.jsx';
import { Icon, P, FAQ, COMPARE } from './data.jsx';

const INK = '#0a0a0a';
const YELLOW = '#ffe500';

/* The menu pages: each one goes deeper than its short section on the home page. */

function Head({ eyebrow, title, lead }) {
  return (
    <section className="page-head">
      <div className="wrap">
        <p className="eyebrow">{eyebrow}</p>
        <h1>{title}</h1>
        <p className="lead">{lead}</p>
      </div>
    </section>
  );
}

function Next({ Go, items }) {
  return (
    <section className="page-next">
      <div className="wrap">
        <p className="eyebrow">Keep going</p>
        <ul>{items.map(([to, t]) => <li key={to}><Go className="more" to={to}>{t} →</Go></li>)}</ul>
      </div>
    </section>
  );
}

const FEATURE_GROUPS = [
  { t: 'Private by design', i: 'lock', items: [
    ['Direct, end-to-end encrypted', "Messages are encrypted on your device and travel straight to your friend's over a WebRTC link. The handshake mixes ML-KEM-768 with P-256 so it is built to hold up against future quantum computers, every message uses its own key, and messages are padded so their length tells little."],
    ['No account, no phone number', 'Your identity is a 12-word recovery phrase made on your device. There is nothing to sign up for and nobody holds your account.'],
    ['Keys you pinned', 'When you add a friend, both of you pin each other\'s keys. Every later link is checked against them, and each chat shows whether it is sealed that way.'],
    ['Hide my IP and Tor mode', 'Friends normally see the address your link comes from. Turn on "Hide my IP from friends" to go through a relay only, or use Tor mode, at the cost of speed.'],
  ] },
  { t: 'You stay in control', i: 'key', items: [
    ['Lock and wipe', 'Lock the app with Windows Hello or your fingerprint, block screenshots on a device, and wipe everything in one tap.'],
    ['Passphrase-locked backups', 'Backups are encrypted with a key derived from your passphrase (Argon2id, 12 characters minimum), including files and old groups.'],
    ['Privacy checkup', 'One screen lists what is weak (no lock, previews showing, anyone can message you) and fixes each in a tap.'],
    ['Network transparency', 'A plain list of every host the app is allowed to talk to, so you can check it yourself.'],
    ['Diagnostics that never carry content', 'The export holds counts and yes/no answers only: no text, names, keys or ids.'],
  ] },
  { t: 'Everyday chat', i: 'users', items: [
    ['Groups of up to 10', 'Add Ben, Ben adds Cat, and you and Cat connect directly without sharing any code. Only an admin changes the group.'],
    ['Replies, reactions, search, delete for everyone', 'The things you expect, kept on your device.'],
    ['Files, photos and voice notes', 'Up to 8 MB, sent straight to the other device. Photos and videos preview in the chat, voice notes show a waveform and play at your speed.'],
    ['Undo send and send later', 'A few seconds to take a message back. Schedule one with quick picks (in an hour, this evening, tomorrow morning) in your own time zone.'],
    ['Pin, archive, mute, quiet hours', 'Pin chats, archive the noisy ones, mute for an hour, 8 hours, until tomorrow or for good, mark a chat silent, and set quiet hours. They follow you to every device.'],
    ['Delivery details', 'See, per device, whether a message was delivered or read, and which route it took.'],
  ] },
  { t: 'Many devices, any network', i: 'laptop', items: [
    ['One account on your phone and PC', 'Link a new device: it shows a QR code, your phone scans it, and you type six digits on the new device. A photo of the code alone is useless. The account key never leaves your first device.'],
    ['Lose a device safely', 'Remove it from another device (takes effect in 24 hours, any device can cancel) or instantly with your recovery phrase. Keys rotate afterwards.'],
    ['Works on bad networks', 'Optional signaling relays, STUN and TURN find a path for you, and devices on the same Wi-Fi link directly.'],
    ['Bluetooth with no internet (Android, experimental)', 'An optional mesh passes sealed messages through nearby phones, up to 7 hops. A banner tells you when a chat is going that way. Tested in simulation so far, not on real radios.'],
  ] },
  { t: 'On Windows and Android', i: 'mon', items: [
    ['Windows', 'A quick-send window on Ctrl+Alt+D (it stays locked while the app lock is on), an unread badge on the tray icon and taskbar, and drop files anywhere to choose who gets them.'],
    ['Android', 'Biometric lock, reply from the notification, and background delivery tuned for phones that stop apps aggressively.'],
  ] },
];

export function FeaturesPage({ Go }) {
  return (
    <>
      <Head eyebrow="Features" title="Everything PrivCord does, in detail" lead="The short list is on the home page. This is the longer one: what each feature does, and where it stops." />
      {FEATURE_GROUPS.map((g) => (
        <section className="page-sec" key={g.t}>
          <div className="wrap">
            <h2 className="group-h"><Icon d={P[g.i]} />{g.t}</h2>
            <div className="fgrid">
              {g.items.map(([t, p]) => (
                <article className="fcard plain" key={t}><h3>{t}</h3><p>{p}</p></article>
              ))}
            </div>
          </div>
        </section>
      ))}
      <section className="page-sec">
        <div className="wrap">
          <h2 className="group-h">Not there yet</h2>
          <p className="sub">So you know before you install: no iPhone, macOS or Linux app; no independent audit; no resumable transfers after a restart; silent chats still play the default sound in a Windows toast; and the Bluetooth mesh is experimental.</p>
        </div>
      </section>
      <Next Go={Go} items={[['how', 'How it works'], ['screens', 'Screens'], ['download', 'Download']]} />
    </>
  );
}

const TRAVEL = [
  ['You add a friend', 'A QR code or six words carries each side\'s public key. Both of you pin the other\'s key; from now on only that key counts.'],
  ['Your devices find each other', 'They post small sealed notes through optional relays (public Nostr relays or your own) saying how to connect. The relay cannot read them.'],
  ['A direct link opens', 'The two devices connect over WebRTC. STUN helps each learn its public address; on strict networks TURN relays the already-encrypted traffic. Then messages flow device to device.'],
  ['If there is no route', 'Messages wait on your device and send when you reconnect. On Android, nearby phones can carry sealed messages over Bluetooth with no internet.'],
];
const SEES = [
  ['Your friend', 'Your messages, and (unless you hide it) the address your link comes from'],
  ['A signaling relay', 'Sealed blobs, and the addresses of devices that talk to it'],
  ['A STUN server', 'The public address you ask it about'],
  ['A TURN server', 'Encrypted traffic and who is talking to whom, never the content'],
  ['Anyone else', 'Nothing readable'],
];

export function HowPage({ Go }) {
  return (
    <>
      <Head eyebrow="How it works" title="From install to your first message" lead="Three steps to get going, and then what actually happens when you press send." />
      <section className="page-sec">
        <div className="wrap">
          <h2 className="group-h">Three steps</h2>
          <ol className="longsteps">
            <li><b>1</b><div><h3>Install and write down 12 words</h3><p>The recovery phrase is your account. It is never stored anywhere else, so keep it safe: the same words restore you on a new device. The app asks you to confirm you wrote it down.</p></div></li>
            <li><b>2</b><div><h3>Add a friend</h3><p>Show your QR code or read out six words. They scan or type them, you tap Add. Both of you now have each other's keys pinned. Groups work the same way, and members connect to each other automatically.</p></div></li>
            <li><b>3</b><div><h3>Chat</h3><p>Messages go device to device. If your friend is offline they wait, and send when you reconnect. Files, photos and voice notes go the same way.</p></div></li>
          </ol>
        </div>
      </section>
      <section className="page-sec">
        <div className="wrap">
          <h2 className="group-h">How a message travels</h2>
          <ol className="longsteps">
            {TRAVEL.map(([t, p], i) => (<li key={t}><b>{i + 1}</b><div><h3>{t}</h3><p>{p}</p></div></li>))}
          </ol>
        </div>
      </section>
      <section className="page-sec">
        <div className="wrap">
          <h2 className="group-h">What each helper can see</h2>
          <div className="tablewrap"><table>
            <thead><tr><th>Who</th><th>What they can see</th></tr></thead>
            <tbody>{SEES.map(([w, s]) => <tr key={w}><td>{w}</td><td>{s}</td></tr>)}</tbody>
          </table></div>
          <p className="fine">That is why "no server reading along" is about content. Connection details such as your address can be seen by the helpers above; you can run your own, use none on a local network, or hide your address with the IP options.</p>
        </div>
      </section>
      <section className="page-sec">
        <div className="wrap">
          <h2 className="group-h">Using more than one device</h2>
          <ol className="longsteps">
            <li><b>1</b><div><h3>The new device shows a QR code</h3><p>It has no account yet, so it asks to join.</p></div></li>
            <li><b>2</b><div><h3>Your phone scans it</h3><p>The phone holds the account key and shows six digits.</p></div></li>
            <li><b>3</b><div><h3>You type the six digits on the new device</h3><p>It checks them (three tries) before accepting. Someone who only photographed the QR code gets nowhere.</p></div></li>
          </ol>
        </div>
      </section>
      <Next Go={Go} items={[['features', 'Every feature'], ['faq', 'Questions'], ['download', 'Download']]} />
    </>
  );
}

export function ScreensPage({ Go, asset }) {
  return (
    <>
      <Gallery asset={asset} level="h1" />
      <section className="page-sec">
        <div className="wrap">
          <h2 className="group-h">What each screen shows</h2>
          <div className="fgrid">
            {SHOTS.map((s) => (<article className="fcard plain" key={s.src}><h3>{s.title}</h3><p>{s.note}</p></article>))}
          </div>
        </div>
      </section>
      <Next Go={Go} items={[['features', 'Every feature'], ['download', 'Download']]} />
    </>
  );
}

const WEAKER = [
  'It has not been independently audited yet, and Signal, Session and Briar have.',
  'It is young and maintained by one person.',
  'Only Windows and Android today.',
  'Far fewer people use it, so there is a smaller network of friends to reach.',
  'The Bluetooth mesh is experimental and tested in simulation only.',
  'Direct links reveal connection details to relays, STUN and TURN servers (never content).',
];

export function ComparePage({ Go }) {
  return (
    <>
      <Head eyebrow="Compare" title="PrivCord next to Signal, Session and Briar" lead="A good-faith summary. It can go out of date, so check each project for its current state." />
      <section className="page-sec">
        <div className="wrap">
          <div className="tablewrap"><table>
            <thead><tr><th></th><th>PrivCord</th><th>Signal</th><th>Session</th><th>Briar</th></tr></thead>
            <tbody>{COMPARE.map((r) => <tr key={r[0]}>{r.map((c, i) => <td key={i}>{c}</td>)}</tr>)}</tbody>
          </table></div>
        </div>
      </section>
      <section className="page-sec">
        <div className="wrap">
          <h2 className="group-h">Where PrivCord is weaker</h2>
          <ul className="plainlist">{WEAKER.map((w) => <li key={w}>{w}</li>)}</ul>
          <p className="sub">If you need a battle-tested, audited messenger today, use one of the audited ones. PrivCord is for people who want to try a different design: no phone number, device-to-device links, and an offline mesh.</p>
        </div>
      </section>
      <Next Go={Go} items={[['how', 'How it works'], ['faq', 'Questions'], ['about', 'About']]} />
    </>
  );
}

const MORE_FAQ = [
  ['How do I add a friend?', 'Open Add friend. Show your QR code or read out the six words, and they scan or type them. You both tap Add.'],
  ['What happens to messages when my friend is offline?', 'They wait on your device and send when you reconnect. Nothing is stored for you on a server.'],
  ['Can I use it on my phone and my PC?', 'Yes. Link a new device: it shows a QR code, your phone scans it, and you type six digits on the new device.'],
  ['What do backups contain, and who can open them?', 'Your chats, files and old groups, encrypted with a key made from your passphrase. Without the passphrase they cannot be opened.'],
  ['Does it cost anything?', 'No. It is free. The source is public under the PC-NC licence, which does not allow commercial use without permission.'],
];

export function FaqPage({ Go }) {
  const all = [...FAQ, ...MORE_FAQ];
  return (
    <>
      <Head eyebrow="FAQ" title="Questions people ask" lead="Straight answers, including the uncomfortable ones." />
      <section className="page-sec">
        <div className="wrap">
          <div className="faq" style={{ marginTop: 0 }}>
            {all.map(([q, a]) => (<details key={q}><summary>{q}</summary><p>{a}</p></details>))}
          </div>
        </div>
      </section>
      <Next Go={Go} items={[['features', 'Every feature'], ['compare', 'Compare'], ['about', 'About']]} />
    </>
  );
}

export function DownloadPage({ Go, repo, reduce, dlRef, dlOn }) {
  const link = (label) => (
    <Magnet padding={60} magnetStrength={3} disabled={reduce}>
      <StarBorder as="a" href={`${repo}/releases/latest`} color={YELLOW} trailColor="#ffffff" backgroundColor={YELLOW} textColor={INK} borderColor="rgba(0,0,0,0)" duration={5} radius={10} className="cta-star">{label}</StarBorder>
    </Magnet>
  );
  return (
    <>
      <section id="download" className="download page-dl" ref={dlRef}>
        {!reduce && <div className="dl-bg" aria-hidden="true"><DotGrid dotSize={4} gap={22} baseColor="#2b2b2b" activeColor={YELLOW} proximity={150} shockRadius={340} shockStrength={5} glow={0} paused={!dlOn} /></div>}
        <div className="wrap">
          <p className="eyebrow">Download</p>
          <h1>Get PrivCord</h1>
          <p className="sub">Free. Pick your platform.</p>
          <div className="dl">
            <div className="dl-row"><div><h3><Icon d={P.mon} />Windows</h3><p>Installer for Windows 10 and 11. The latest build is attached to the newest release.</p></div>{link('Download for Windows')}</div>
            <div className="dl-row"><div><h3><Icon d={P.ph} />Android</h3><p>Install the arm64 APK (almost every phone); the armeabi-v7a one is for old 32-bit phones.</p></div>{link('Download for Android')}</div>
          </div>
        </div>
      </section>
      <section className="page-sec">
        <div className="wrap">
          <h2 className="group-h">Install notes</h2>
          <div className="fgrid">
            <article className="fcard plain"><h3>Windows</h3><p>The installer is not code-signed yet, so Windows SmartScreen will warn you. Choose <em>More info</em>, then <em>Run anyway</em>.</p></article>
            <article className="fcard plain"><h3>Android</h3><p>You need to allow installing from the app or browser you downloaded it with. Pick the arm64 file unless your phone is very old.</p></article>
            <article className="fcard plain"><h3>First run</h3><p>The app makes your 12 recovery words. Write them down before you continue: they are the only way back to your account.</p></article>
            <article className="fcard plain"><h3>Before you rely on it</h3><p>PrivCord has not been independently audited. Read the <a href={`${repo}/blob/main/docs/KNOWN_LIMITATIONS.md`}>known limitations</a> first.</p></article>
          </div>
        </div>
      </section>
      <Next Go={Go} items={[['features', 'Every feature'], ['how', 'How it works'], ['faq', 'Questions']]} />
    </>
  );
}

// Edit these lines to change what the About page says.
const ABOUT = {
  name: 'Anand',
  github: 'https://github.com/anandks2006',
  githubName: 'anandks2006',
  x: 'https://x.com/AnandKrishnaK1',
  xName: '@AnandKrishnaK1',
};

export function AboutPage({ Go, repo }) {
  return (
    <>
      <Head eyebrow="About" title={`Made by ${ABOUT.name}`} lead="PrivCord is built and maintained by one independent developer, in the open." />
      <section className="page-sec">
        <div className="wrap">
          <div className="fgrid">
            <article className="fcard plain">
              <h3>Maintainer</h3>
              <ul className="plainlist links">
                <li><a href={ABOUT.x}>X: {ABOUT.xName}</a> (licensing and contact)</li>
                <li><a href={repo}>The PrivCord source</a></li>
              </ul>
            </article>
            <article className="fcard plain">
              <h3>Contributors</h3>
              <ul className="plainlist links">
                <li><a href={ABOUT.github}>{ABOUT.githubName}</a>: contributor to PrivCord, who reviewed and merged pull requests #42, #64, #65, #66 and #68 in November 2026.</li>
              </ul>
            </article>
            <article className="fcard plain">
              <h3>How the project is run</h3>
              <p>The source is public and free for personal, educational and research use under the PC-NC licence; commercial use needs written permission. There is no company behind it and no data collected.</p>
            </article>
            <article className="fcard plain">
              <h3>Found a problem?</h3>
              <p>Please <a href={`${repo}/issues`}>open an issue</a>. For security problems, read the <a href={`${repo}/blob/main/SECURITY.md`}>security policy</a> first.</p>
            </article>
            <article className="fcard plain">
              <h3>Credits</h3>
              <p>Fonts: Bricolage Grotesque and Figtree (OFL). Motion components from <a href="https://reactbits.dev">React Bits</a>.</p>
            </article>
          </div>
        </div>
      </section>
      <Next Go={Go} items={[['features', 'Every feature'], ['compare', 'Compare'], ['download', 'Download']]} />
    </>
  );
}
