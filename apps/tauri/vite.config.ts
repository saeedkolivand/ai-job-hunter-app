import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import tailwindcss from '@tailwindcss/vite';
import { TanStackRouterVite } from '@tanstack/router-vite-plugin';
import path from 'node:path';

// Tauri expects a fixed port in dev mode.
const TAURI_DEV_PORT = 5174;

export default defineConfig({
  // Point alias at the desktop renderer source so the same feature code,
  // routes, components, and service hooks are reused without copying.
  resolve: {
    alias: { '@': path.resolve(__dirname, '../desktop/src/renderer') },
  },
  plugins: [
    TanStackRouterVite({
      routesDirectory: path.resolve(__dirname, '../desktop/src/renderer/routes'),
      generatedRouteTree: path.resolve(__dirname, '../desktop/src/renderer/routeTree.gen.ts'),
    }),
    react(),
    tailwindcss(),
  ],
  // Prevent Vite from obscuring Rust errors in the terminal.
  clearScreen: false,
  server: {
    port: TAURI_DEV_PORT,
    strictPort: true,
    headers: {
      // Mirrors the Electron CSP but permits Tauri IPC (tauri://) and Ollama.
      'Content-Security-Policy': [
        "default-src 'self' tauri: asset: https://asset.localhost",
        "script-src 'self' 'unsafe-inline' 'unsafe-eval'",
        "style-src 'self' 'unsafe-inline'",
        "img-src 'self' data: blob: asset: https://asset.localhost",
        "font-src 'self' data: asset: https://asset.localhost",
        "connect-src 'self' http://127.0.0.1:11434 http://127.0.0.1:* ws://127.0.0.1:* tauri: ipc: http://ipc.localhost",
      ].join('; '),
    },
  },
  build: {
    // Tauri uses ES modules; suppress the 500 kB warning since the renderer
    // is already structured this way in the Electron build.
    target: 'esnext',
    minify: false,
    outDir: 'dist',
  },
});
