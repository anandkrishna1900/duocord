import { createRoot } from 'react-dom/client';
import App from './App';
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

createRoot(container).render(<App />);
