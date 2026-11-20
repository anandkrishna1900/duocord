import { createRoot } from 'react-dom/client';
import App from './App';
import { QuickSend } from './components/QuickSend';
import { applySettings, loadSettings } from './lib/settings';
import './styles.css';
import './look.css';
import './polish.css';
import './wow.css';
import { installSpotlight } from './lib/spotlight';

applySettings(loadSettings());
installSpotlight();

const container = document.getElementById('root');
if (!container) throw new Error('Missing root element.');

// The Ctrl+Alt+D popup is the same page with `?quick=1`: a tiny window that talks to the host's engine, never its own.
createRoot(container).render(new URLSearchParams(location.search).has('quick') ? <QuickSend /> : <App />);
