import { app, session } from 'electron';

export function applySecurityDefaults(): void {
  // Refuse any unknown protocol navigation; renderer only ever sees app:// and file:// to the bundle.
  app.on('web-contents-created', (_e, contents) => {
    contents.setWindowOpenHandler(() => ({ action: 'deny' }));
    contents.on('will-navigate', (event, url) => {
      const u = new URL(url);
      if (u.protocol !== 'http:' || u.hostname !== 'localhost') event.preventDefault();
    });
  });

  app.whenReady().then(() => {
    session.defaultSession.webRequest.onHeadersReceived((details, callback) => {
      const csp = [
        "default-src 'self'",
        "script-src 'self'",
        "style-src 'self' 'unsafe-inline'", // Tailwind v4 injects styles
        "img-src 'self' data: blob:",
        "font-src 'self' data:",
        "connect-src 'self' http://127.0.0.1:11434 ws://127.0.0.1:* http://localhost:*",
        "object-src 'none'",
        "base-uri 'self'",
        "frame-ancestors 'none'",
      ].join('; ');
      callback({
        responseHeaders: { ...details.responseHeaders, 'Content-Security-Policy': [csp] },
      });
    });
  });
}
