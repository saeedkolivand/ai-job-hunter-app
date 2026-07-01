/// <reference types="vite/client" />

// In the Tauri shell window.api is NOT present (there is no Electron preload).
// We declare it as `never` so the type system accepts app-client.ts's
// createDesktopIpcClient (which is never called from Tauri code) while making
// accidental usage a compile error.
declare global {
  interface Window {
    api: never;
  }
}

export {};
