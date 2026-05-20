import { defineConfig, externalizeDepsPlugin } from 'electron-vite';
import react from '@vitejs/plugin-react';
import tailwindcss from '@tailwindcss/vite';
import { TanStackRouterVite } from '@tanstack/router-vite-plugin';
import path from 'node:path';

export default defineConfig({
  main: {
    plugins: [externalizeDepsPlugin()],
    build: {
      rollupOptions: {
        input: { index: path.resolve(__dirname, 'src/main/index.ts') },
        output: {
          format: 'es',
          entryFileNames: '[name].mjs',
        },
      },
    },
  },
  preload: {
    build: {
      rollupOptions: {
        input: { index: path.resolve(__dirname, 'src/preload/index.ts') },
        output: {
          format: 'cjs',
          entryFileNames: '[name].js',
        },
        external: ['electron'],
      },
    },
  },
  renderer: {
    root: path.resolve(__dirname, 'src/renderer'),
    resolve: {
      alias: { '@': path.resolve(__dirname, 'src/renderer') },
    },
    plugins: [
      TanStackRouterVite({ routesDirectory: 'routes', generatedRouteTree: 'routeTree.gen.ts' }),
      react(),
      tailwindcss(),
    ],
    build: {
      rollupOptions: { input: { index: path.resolve(__dirname, 'src/renderer/index.html') } },
    },
    server: {
      headers: {
        'Content-Security-Policy':
          "default-src 'self'; script-src 'self' 'unsafe-inline' 'unsafe-eval'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; font-src 'self' data:; connect-src 'self' http://127.0.0.1:11434 ws://127.0.0.1:* http://localhost:*",
      },
    },
  },
});
