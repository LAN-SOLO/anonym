import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

// Tauri erwartet einen festen Dev-Port (siehe src-tauri/tauri.conf.json devUrl)
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 1435,
    strictPort: true,
  },
  build: {
    target: 'es2021',
    outDir: 'dist',
  },
});
