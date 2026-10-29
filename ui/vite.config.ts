import { svelte } from '@sveltejs/vite-plugin-svelte';
import { defineConfig } from 'vitest/config';

export default defineConfig({
  plugins: [svelte()],
  clearScreen: false,
  // Avoid Vite's newer browser baseline so the shell can target macOS 13 WebKit.
  build: { target: ['es2022', 'safari16', 'chrome105', 'firefox102'] },
  test: { environment: 'node', include: ['tests/**/*.test.ts'] },
});
