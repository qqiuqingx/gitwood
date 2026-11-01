import { svelte } from '@sveltejs/vite-plugin-svelte';
import { defineConfig } from 'vitest/config';

export default defineConfig({
  plugins: [svelte()],
  clearScreen: false,
  // Avoid Vite's newer browser baseline so the shell can target macOS 13 WebKit.
  // The production CSP allows img-src 'self' only, so never inline assets as data: URLs.
  build: { target: ['es2022', 'safari16', 'chrome105', 'firefox102'], assetsInlineLimit: 0 },
  test: { environment: 'node', include: ['tests/**/*.test.ts'] },
});
