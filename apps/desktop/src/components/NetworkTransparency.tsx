import { useEffect, useState } from 'react';
import type { ChatEngineApi } from '../lib/engineApi';
import { loadNetworkConfig } from '../lib/network';
import { networkHosts, type HostRow } from '../lib/networkHosts';

/** Every internet host this device may contact, and why, straight from the settings that decide it. */
export function NetworkTransparency({ engine }: { engine: ChatEngineApi | null }) {
  const net = loadNetworkConfig();
  const [rows, setRows] = useState<HostRow[]>(() => networkHosts(net));
  useEffect(() => {
    let live = true;
    if (!engine?.insight) {
      setRows(networkHosts(net));
      return;
    }
    // The shared engine works the list out, so it is the same on Windows and Android.
    const input = {
      mode: net.transportMode ?? 'AUTOMATIC',
      force_relay: net.forceRelay,
      nostr: net.signaling.flatMap((s) => (s.kind === 'nostr' ? s.relays : [])),
      relays: net.signaling.flatMap((s) => (s.kind === 'relay' ? [s.url] : [])),
      stun: net.stun,
      turn: net.turn.flatMap((t) => t.urls),
    };
    void engine.insight('hosts', input).then(
      (raw) => {
        if (live && Array.isArray(raw)) setRows(raw as HostRow[]);
      },
      () => undefined,
    );
    return () => {
      live = false;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [engine, JSON.stringify(net)]);
  return (
    <div className="card-box">
      <div className="card-title">
        <strong>What this app connects to</strong>
      </div>
      {rows.length === 0 ? (
        <p className="muted small">Nothing. In this mode DuoCord only uses Bluetooth and the local network.</p>
      ) : (
        <ul className="host-list">
          {rows.map((r) => (
            <li key={`${r.host}|${r.why}`}>
              <code>{r.host}</code>
              <span className="muted small">{r.why}</span>
            </li>
          ))}
        </ul>
      )}
      <p className="muted small">
        That is the whole list: no analytics, no update check, no ads, no account server. Your messages go device to device;
        the hosts above only help two devices find each other, and see sealed data they cannot read.
      </p>
    </div>
  );
}
