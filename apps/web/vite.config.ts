/// <reference types="vitest/config" />
import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import { fileURLToPath, URL } from 'node:url';

export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: { '@': fileURLToPath(new URL('./src', import.meta.url)) },
  },
  // The solver runs in a module worker; `wasm-pack --target web` emits an ES
  // module whose .wasm is resolved with `new URL(..., import.meta.url)`, which
  // Vite handles as an asset in both the main bundle and workers.
  worker: { format: 'es' },
  build: { target: 'es2022', sourcemap: true },
  server: { port: 5173, strictPort: false },
  test: {
    environment: 'jsdom',
    globals: true,
    setupFiles: ['./src/test/setup.ts'],
    include: ['src/**/*.test.{ts,tsx}'],
    exclude: ['e2e/**', 'node_modules/**'],
  },
});
