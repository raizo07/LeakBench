import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

// `base: './'` matters: the built bundle is opened from disk as often as it is
// served, and `leakbench serve` mounts it at the root.
export default defineConfig({
  base: './',
  plugins: [react()],
  build: { outDir: 'dist', emptyOutDir: true },
});
