// After `vite build`: write one HTML file per page (dist/features/index.html, ...) so every page has a real address that
// GitHub Pages serves directly, with its own title, description and canonical link. The app inside is the same bundle.
import { readFileSync, writeFileSync, mkdirSync } from 'node:fs';

const PAGES = {
  features: ['Features: PrivCord', 'Direct end-to-end encrypted chats, groups of up to 10, one account on many devices, files and voice notes, and an experimental Bluetooth mesh.'],
  how: ['How it works: PrivCord', 'Install, write down 12 words, add a friend with a QR code or six words, and chat device to device.'],
  screens: ['Screens: PrivCord', 'Real screenshots of PrivCord on Windows and Android.'],
  compare: ['How PrivCord compares', 'PrivCord next to Signal, Session and Briar: accounts, servers, Bluetooth and audits.'],
  faq: ['Questions people ask: PrivCord', 'Accounts, servers, offline use, platforms, lost phones, licence and audits.'],
  download: ['Get PrivCord for Windows and Android', 'Download PrivCord for Windows or Android. Free.'],
};
const dist = new URL('../dist/', import.meta.url);
const home = readFileSync(new URL('index.html', dist), 'utf8');

for (const [slug, [title, desc]] of Object.entries(PAGES)) {
  let h = home
    // the page lives one folder down, so relative links climb out of it
    .replace(/(\s(?:href|src))="(?!https?:|#|\/|data:|__)([^"]+)"/g, '$1="../$2"')
    .replace(/<title>[^<]*<\/title>/, `<title>${title}</title>`)
    .replace(/(<meta name="description" content=")[^"]*"/, `$1${desc}"`)
    .replace(/(<meta property="og:title" content=")[^"]*"/, `$1${title}"`)
    .replace(/(<meta property="og:description" content=")[^"]*"/, `$1${desc}"`)
    .replace(/(<link rel="canonical" href=")__SITE_URL__\/"/, `$1__SITE_URL__/${slug}/"`)
    .replace(/(<meta property="og:url" content=")__SITE_URL__\/"/, `$1__SITE_URL__/${slug}/"`);
  mkdirSync(new URL(`${slug}/`, dist), { recursive: true });
  writeFileSync(new URL(`${slug}/index.html`, dist), h);
}

const sm = readFileSync(new URL('sitemap.xml', dist), 'utf8');
const extra = Object.keys(PAGES).map((s) => `  <url><loc>__SITE_URL__/${s}/</loc><changefreq>monthly</changefreq><priority>0.7</priority></url>\n`).join('');
writeFileSync(new URL('sitemap.xml', dist), sm.replace('</urlset>', `${extra}</urlset>`));
console.log(`pages: wrote ${Object.keys(PAGES).length} page files and updated the sitemap`);
