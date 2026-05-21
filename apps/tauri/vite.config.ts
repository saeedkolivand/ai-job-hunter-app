import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import tailwindcss from '@tailwindcss/vite';
import { TanStackRouterVite } from '@tanstack/router-vite-plugin';
import path from 'node:path';

// Tauri expects a fixed port in dev mode.
const TAURI_DEV_PORT = 5174;

export default defineConfig({
  resolve: {
    alias: { '@': path.resolve(__dirname, 'src/renderer') },
  },
  plugins: [
    TanStackRouterVite({
      routesDirectory: path.resolve(__dirname, 'src/renderer/routes'),
      generatedRouteTree: path.resolve(__dirname, 'src/renderer/routeTree.gen.ts'),
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
    target: 'esnext',
    minify: false,
    outDir: 'dist',
  },
});
