// Defense-in-depth CSP for /mission-control, rendered as a <meta http-equiv>.
// React 19 hoists <meta> into <head>. The page is a static export whose ONLY
// off-origin traffic is api.github.com (data) — fonts are self-hosted (public/
// fonts/), so no font CDN is trusted here. script/style stay 'unsafe-inline'
// because Next's static export inlines its hydration bootstrap and the shell CSS
// is inlined per route — a nonce is impossible without a server. object/base/form
// are hard-denied. No external images are rendered, so img-src stays `'self'
// data:` (dropping `https:` closes an Image()-beacon token-exfil path).
//
// ORIGIN INVARIANT (load-bearing, whole site): there must be NO third-party
// JavaScript anywhere on this origin, ever. Any foreign script on ANY page of
// this origin can read the mission-control PAT out of localStorage; a per-page
// CSP cannot protect it. Keep the entire origin first-party-JS-only.
//
// NOTE: `frame-ancestors 'none'` below is INERT in a <meta> CSP — CSP3 only
// honors it via an HTTP response header, which GitHub Pages cannot set. The
// actual clickjacking mitigation is the JS frame-buster in MissionControl.tsx.
const CSP = [
  "default-src 'self'",
  "connect-src 'self' https://api.github.com",
  "img-src 'self' data:",
  "style-src 'self' 'unsafe-inline'",
  "font-src 'self'",
  "script-src 'self' 'unsafe-inline'",
  "object-src 'none'",
  "base-uri 'self'",
  "form-action 'none'",
  "frame-ancestors 'none'",
].join('; ');

export function CspMeta() {
  return <meta httpEquiv="Content-Security-Policy" content={CSP} />;
}
