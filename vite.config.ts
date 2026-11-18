import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';

// Tauri expects a fixed dev port; WebView2 is evergreen so we can target a modern baseline.
export default defineConfig({
  plugins: [svelte()],
  clearScreen: false,
  server: { port: 1420, strictPort: true, host: 'localhost' },
  build: { target: 'chrome110', sourcemap: false }
});
