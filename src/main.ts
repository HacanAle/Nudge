import { mount } from 'svelte';
import { getCurrentWindow } from '@tauri-apps/api/window';

// Theme fonts. Browsers only download a face when something actually renders in it.
import '@fontsource-variable/geist'; // Slab
import '@fontsource-variable/geist-mono';
import '@fontsource-variable/inter'; // Orchard
import '@fontsource-variable/outfit'; // Vitrine
import '@fontsource-variable/pixelify-sans/wght.css'; // Beige Box
import '@fontsource/anton/latin-400.css'; // Safety Pin
import '@fontsource/courier-prime/latin-400.css';
import '@fontsource/courier-prime/latin-700.css';
import '@fontsource/chakra-petch/latin-400.css'; // Voltage
import '@fontsource/chakra-petch/latin-500.css';
import '@fontsource/chakra-petch/latin-600.css';
import '@fontsource/chakra-petch/latin-700.css';
import '@fontsource/share-tech-mono/latin-400.css';
import '@fontsource/abril-fatface/latin-400.css'; // Handbill
import '@fontsource/libre-caslon-text/latin-400.css';
import '@fontsource/libre-caslon-text/latin-400-italic.css';
import '@fontsource/libre-caslon-text/latin-700.css';

import './themes.css';
import './surface.css';
import './app.css';
import App from './App.svelte';
import { settings } from './lib/prefs.svelte';

// Both windows load this same bundle; the window label decides which UI to render.
const view = getCurrentWindow().label;
document.documentElement.dataset.view = view;

// Apply the last-used theme before the first paint, then confirm it with the backend.
document.documentElement.dataset.theme = settings.theme;
settings.init();

mount(App, { target: document.getElementById('app')!, props: { view } });
