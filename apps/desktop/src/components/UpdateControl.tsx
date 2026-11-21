import { useEffect, useState } from 'react';
import { loadNetworkConfig } from '../lib/network';
import type { AppSettings } from '../lib/settings';
import { SNOOZE_MS, checkForUpdate, installUpdate, mayCheckForUpdates, snoozeActive, type FoundUpdate } from '../lib/updates';

/** Installs a found update: the app closes and comes back as the new version. */
function InstallButton({ found }: { found: FoundUpdate }) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  return (
    <>
      <button
        type="button"
        className="button primary small"
        disabled={busy}
        onClick={() => {
          setBusy(true);
          setError(null);
          installUpdate().catch((e: unknown) => {
            setBusy(false);
            setError(String((e as Error)?.message ?? e));
          });
        }}
      >
        {busy ? 'Downloading…' : `Install ${found.version}`}
      </button>
      {error ? <p className="muted small">{error}</p> : null}
    </>
  );
}

/** Settings (About): "Check for updates now" with its result. The switch itself lives in the dialog. */
export function UpdateCheck() {
  const allowed = mayCheckForUpdates(loadNetworkConfig());
  const [checking, setChecking] = useState(false);
  const [status, setStatus] = useState<string | null>(null);
  const [found, setFound] = useState<FoundUpdate | null>(null);
  if (!allowed) {
    return <p className="muted small">Turn off Hide my IP and Tor mode to check for updates: a check contacts GitHub directly.</p>;
  }
  return (
    <>
      <div className="button-row">
        <button
          type="button"
          className="button secondary small"
          disabled={checking}
          onClick={() => {
            setChecking(true);
            setStatus(null);
            setFound(null);
            checkForUpdate()
              .then((update) => {
                setFound(update);
                setStatus(update ? `Version ${update.version} is available.` : 'You are up to date.');
              })
              .catch(() => setStatus('Could not check for updates. Try again later.'))
              .finally(() => setChecking(false));
          }}
        >
          {checking ? 'Checking…' : 'Check for updates now'}
        </button>
        {found ? <InstallButton found={found} /> : null}
      </div>
      {status ? <p className="muted small">{status}</p> : null}
    </>
  );
}

/** Once at start: a quiet banner when a newer build exists. "Later" is remembered for three days. */
export function UpdateBanner({ settings, updateSettings }: { settings: AppSettings; updateSettings: (patch: Partial<AppSettings>) => void }) {
  const [found, setFound] = useState<FoundUpdate | null>(null);
  const wanted = settings.updateChecks && !snoozeActive(settings.updateSnoozedUntil);
  useEffect(() => {
    if (!wanted || !mayCheckForUpdates(loadNetworkConfig())) return;
    let live = true;
    checkForUpdate()
      .then((update) => live && setFound(update))
      .catch(() => undefined);
    return () => {
      live = false;
    };
  }, [wanted]);
  if (!found) return null;
  return (
    <div className="banner info" role="status">
      <span>PrivCord {found.version} is ready.</span>
      <InstallButton found={found} />
      <button
        type="button"
        className="button secondary small"
        onClick={() => {
          setFound(null);
          updateSettings({ updateSnoozedUntil: Date.now() + SNOOZE_MS });
        }}
      >
        Later
      </button>
    </div>
  );
}
