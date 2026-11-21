# PrivCord — Known Limitations & Threat Model

**Version**: 1.1  
**Date**: November 2026  
(The app was called DuoChat earlier; some screens and sections below still use that name.)

---

## 1. Remote Participant Erasure
Local kill switch activation operates strictly within the local device boundary:
- **No Remote Erasure**: Activating "NUKE DUOCHAT" or "Clear Everything" deletes data on the current device only. It cannot and does not erase copies of messages, images, or media already delivered to the other participant's device.
- **Remote Revocation**: Any future remote deletion mechanism would require signed requests that depend on network connectivity and cannot guarantee deletion on offline, compromised, or modified clients.

---

## 2. Storage & Hardware Realities
- **Wear Leveling on Solid State / Flash Memory**: Flash memory (eMMC, UFS, NVMe SSDs) utilizes controller-level wear leveling. Cryptographic deletion (deleting the key required to decrypt the ciphertext) is the only reliable erasure method on modern solid-state drives.
- **Operating System Backups**: If the user has OS-level cloud backups enabled (e.g., full device images), external backups created prior to deletion may retain historical states.
- **Screenshots & Screen Recorders**: Operating system level screen capture, third-party screen recorders, or hardware captures cannot be detected or deleted by DuoChat.
- **OS Keyboard / IME Caches**: Some third-party Android keyboards log predictive text outside the application's sandbox. Users in high-security environments should use open-source keyboards with network permissions disabled.

---

## 3. Network & Transport Constraints
- **Direct P2P NAT Traversal**: In symmetrical NAT environments where STUN cannot resolve a direct route, WebRTC falls back to encrypted TURN relays. While traffic is end-to-end encrypted, metadata (IP address, packet timing) is visible to the relay operator.

---

## 4. Groups
- **At most 10 members** (including the creator), enforced by every device identically. A group of N
  means N-1 links per device (a full mesh), so 10 means 9. The limit is tested in protocol simulation
  and in the browser suite; real-phone behaviour at 10 is not tested.
- **Introductions are trust.** Someone added by a member you trust becomes a contact you can message
  directly (labelled "Introduced by NAME"). A malicious or compromised member could introduce an
  impostor; use the safety numbers for people who matter.
- **Group-only opt-out.** With "Stay group-only with people I meet in groups" on (Settings > Security),
  or if the other person has it on, you cannot start a direct chat with someone you only know through a
  group. You still exchange group messages, replies and files, and you can pair manually by swapping
  invites. It does not hide you from the group: members still learn your identity key and connect to
  you to deliver group messages, so a direct (non-relayed) link exposes your IP address to them. A
  modified client could ignore the setting; it is a courtesy between honest clients, not a guarantee.
- **Removal is forward-looking.** A removed member keeps what they already received, and nothing can
  delete it.
- **New members get no history.** They see messages sent after they joined.
- **Admin is the creator.** Only the creator can remove others or rename the group.
- **Large groups and offline members.** Messages for a member you cannot reach yet are queued and sent
  when the link opens.

## 5. Six-word invites
- The six words are a **meeting point**, not the identity itself (an identity card is far too big for six
  words): 66 random bits from which both phones derive a relay mailbox and a key. The person showing the
  words publishes their card there, encrypted; the other types the words, fetches it and answers with their
  own card; the first person then **confirms who answered** with the safety numbers.
- They need a connection service (a Nostr relay or your own relay) and the person showing the words must
  keep the app open: the words stop working after 15 minutes or when the app closes. The QR code and the
  full invite text (under "Full invite text") work with no service at all.
- Anyone who learns the words while they are valid can read the card (public information) and can answer
  it, which is why the answer is always confirmed by a person. A relay sees only a hash and encrypted bytes;
  guessing 66 bits offline is not practical (each guess costs a 150,000-round key derivation).

## 6. App lock
- **Android** uses the system prompt (fingerprint, face or screen lock) every time the app opens and after
  30 seconds out of sight. **Windows** uses a Windows Hello passkey (WebAuthn, user verification required,
  the signature is checked against the stored public key) when the app opens, after 5 minutes out of
  sight, and on "Lock now". Turning it off needs the same proof.
- It keeps other people at an unlocked device out. It is **not** protection against someone who copies the
  files off the disk (they are encrypted with a separate key) and it does not stop the app from receiving
  messages while locked: notifications can still show message text unless you turn previews off.
- Not tested on a real Windows Hello prompt or a real Android biometric prompt; the desktop flow is tested
  against a virtual authenticator and the Android decision rules by unit tests.
- Video and image previews are decoded by the system's media components. Other file types are never
  opened by PrivCord, only saved.

## 7. Not available
- Files are limited to **8 MB**, go **device to device** (never through a server or the offline
  relay) and need a WebRTC link, so they are not sent over Bluetooth. A file for someone offline waits
  on your device and is sent when they reconnect. In a group a file is sent separately to every member
  (up to 9 copies), so it uses your upload each time. Received files are saved encrypted on the
  device; "Save" writes an ordinary file. Only PNG, JPEG, GIF and WebP images and MP4, WebM, QuickTime and Ogg videos get a preview, never SVG, and no other file type is opened.
  File saving from the packaged desktop app has only been exercised in the browser suite.
- No push notifications (there is no server to send them). Background delivery depends on the operating system letting the app run; some phone makers stop apps aggressively.
- A file transfer that was cut off (the link dropped, or the app closed) goes on from where the receiver last saved its progress, not from the start: the receiver keeps what it has every 768 KiB and when the link goes, and tells the sender where it is, so the sender only sends the rest. A crash can lose up to 768 KiB of progress. It works on direct links between two current versions; through the group mesh, or with an older version on the other side, the whole file is sent again. A saved part is erased with "Erase messages", "Erase attachments" and "Cancel transfers".
- No iPhone, macOS or Linux app, and no dragging a file onto the tray icon (the framework does not support it; dropping files into the open window works).
- **Long chats load a window first.** Both apps ask the engine for the newest 200 messages of each chat and show "Load earlier messages (N)" at the top of a chat to fetch 100 more at a time, so opening a chat takes the same time however long its history is. A reply to a message that is not loaded yet shows no quoted text until it is, and a search hit older than what is loaded cannot be scrolled to until you load back to it (the Android screens have been built but not run on a device).
- **What is kept where (memory and disk audit).**
  - *On disk:* the history, files, settings and unfinished file transfers live only in the SQLCipher database (AES-256 over every page); its key is in the operating system's credential store. Backups are passphrase-encrypted. The desktop app's saved network settings file (`network.json`: relay addresses and a TURN password) is AES-256-GCM encrypted under a key in the credential store; without a credential store it is not written at all, and an older plain-text copy is replaced when first read. The apps write no message text to logs; Android no longer logs peer ids.
  - *In engine memory, sealed:* message text and quoted replies of the newest 256 messages of each chat (older ones are read back from the database when needed), and the whole multi-device sync log (every entry's body, the merged messages, edits, contacts, settings, group events, reactions and snapshot notices, including the unfinished entries waiting for an earlier one). They are AES-256-GCM encrypted under a key made when the app starts and never saved, and opened only for the moment something uses them. This protects memory dumps and swap files that do not include the running process; it does not stop someone who can read the running app itself, and what a screen shows has to be readable.
  - *In memory, not sealed:* a small index of every message (chat, sender, time, delivery state, read time), message ids, contact names, nicknames and group names, the names people use in channels, whole files while they are being sent or received (up to 8 MB), and whatever the screens are showing. Starting the app reads the whole history once to build the index.
  - *On disk, not yet encrypted:* the desktop webview keeps a copy of the app's settings, including the relay and TURN settings you typed, in its own local storage, and the Android app keeps its settings in private app storage (not readable by other apps, but not encrypted). Moving those into the credential store / Keystore is the next step.
- The Bluetooth mesh exists on Android only and has never run on real radios; Windows cannot join it ([details](CHANNELS_AND_MESH.md)).

## 8. Several devices
- **Linking** has the new device show a QR code, the phone that holds the account key scan it, and the six digits be typed on the new device. The new device holds the phone's grant until the digits match (three tries) and never shows them itself, and the phone answers only the first request, so a photo of the QR code alone gets nowhere. The decision rules are tested; scanning with a real camera and a real second device has not been tried.
- **Removing a device** takes effect after 24 hours (any device of yours can cancel), or at once with the recovery phrase. A removed device keeps what it already received. If every device is lost and the phrase is lost, the account cannot be recovered; there is no reset, because there is no server.
- **Settings that follow you** (pins, archived chats, mutes, quiet hours, silent chats, screen privacy) are synced between your devices with last writer wins. If two devices change the same setting at nearly the same moment, the later change wins and the other is overwritten.

## 9. Notifications and quiet times
- **Silent chats** are honoured on Android. On Windows the setting is saved and respected everywhere, but the toast from a silent chat still plays the default sound (the notification plugin ignores "silent" on desktop). The toast shown while the window is closed to the tray does not read the silent flag.
- **Quiet hours and mute-for-a-while** use the time zone each app tells the engine. A wrong device time zone means wrong quiet hours.
- Notifications can show message text unless previews are turned off; the app lock does not stop notifications.

## 10. Windows conveniences
- **Quick-send window (Ctrl+Alt+D)** is locked while the app lock is on, so it cannot be used to get around the lock. Another program may already use that shortcut. If the main window is closed, messages from the quick-send window use the relay or queue path only, because the main window owns the direct links.
- **Tray tooltip and taskbar unread badge** and the global shortcut have only been built, not run on real Windows from the development environment.

## 11. Delivery details and nearby mode
- **Per-device delivery and read marks** are kept on the device that sent the message (the newest 2000 messages) and are not synced. For a direct message, your friend's known devices that have not answered show as waiting. For a group message, only devices that answered are listed.
- **Hop counts** are shown only for channel messages that arrived over the Bluetooth mesh (up to 7 radio hops). They count radio hops, not distance, and the mesh has never run on real radios.

## 12. Screen privacy, voice notes, diagnostics
- **Block screenshots** stops ordinary screenshots and screen recording on the device. It does not stop a camera pointed at the screen, another device that mirrors the display, or some capture hardware.
- **Voice-note waveforms** are drawn from the file on each device (the engine cannot decode audio). Very long voice notes are cut off at about three minutes for the waveform on Android; playback is not affected.
- **Diagnostics export** holds counts and yes/no answers only, never text, names, keys or ids, and is checked by a test. Read it before sending it to anyone.

## 13. Project status
- PrivCord has **not been independently audited**. The protocol is covered by tests, shared test vectors and fuzzing in CI, and the code has been reviewed by its maintainers and by AI-assisted review, which is not a substitute for an outside audit.
- It is maintained by two people. Windows and Android only.
- It is source-available under the PC-NC licence, not open source in the OSI sense.
- Relays, STUN and TURN servers you use can see connection metadata (addresses and timing) but not message content. "Hide my IP from friends" and Tor mode reduce what friends and relays learn, at the cost of speed.


## Connecting across cities or mobile networks (e.g. Kerala to Kolkata)

Distance does not matter, the networks do. A direct link needs both phones to find a path to each other:

- Most Indian mobile data (Jio, Airtel, Vi) sits behind carrier-grade NAT. STUN alone often cannot punch through two of these at once. A **TURN relay** is the fix.
- The app now shows a plain-language banner in the chat saying which case you are in, with a one-tap **Deliver via relays** button (store-and-forward) so messages still arrive while a direct link is impossible.
- To get live links across any two networks, run a relay (TURN) server. `docs/turn-server/` has a ready docker-compose and config for a small VPS, and a script that prints one `PRIVCORD-TURN1:` line. Paste that line into Settings > Connection on every device (desktop and Android) and the three relay fields fill in. The relay only carries encrypted traffic, but the line contains its password, so share it only with people you want to let use it. The kit and the line format are tested; coturn itself has not been run against it from the development environment.
- Not verified on real carrier networks from the development environment; the diagnosis logic is unit-tested, the field behaviour is not.
