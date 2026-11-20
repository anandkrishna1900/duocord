import { loadNetworkConfig } from '../lib/network';
import { networkHosts } from '../lib/networkHosts';

/** Every internet host this device may contact, and why, straight from the settings that decide it. */
export function NetworkTransparency() {
  const net = loadNetworkConfig();
  const rows = networkHosts(net);
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
