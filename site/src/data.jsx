export const Icon = ({ d }) => (
  <svg viewBox="0 0 24 24" aria-hidden="true" className="ic">
    {d}
  </svg>
);
export const P = {
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

export const FEATURES = [
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

export const STEPS = [
  ['Install and write down 12 words', 'That recovery phrase is your account. It is never stored anywhere else, so keep it safe: the same words restore you on a new device.'],
  ['Add a friend', "Show your QR code or six words. They scan or type them, you tap Add. Both of you now have each other's keys pinned."],
  ['Chat', 'Messages go device to device. If your friend is offline, they wait and send when you reconnect.'],
];

export const FAQ = [
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

export const COMPARE = [
  ['Phone number or email needed', 'No', 'Phone number', 'No', 'No'],
  ['Messages go through company servers', 'No (device to device; optional relays see only sealed blobs)', 'Yes', 'Yes (a decentralized node network)', 'No (Tor, Wi-Fi or Bluetooth)'],
  ['Works over Bluetooth with no internet', 'Yes (Android mesh, experimental)', 'No', 'No', 'Yes'],
  ['One account on several devices', 'Yes', 'Yes', 'Limited', 'Limited'],
  ['Independently audited', 'No, not yet', 'Yes', 'Yes', 'Yes'],
];

