// The external URLs repeated across marketing pages' body.html — kept in one
// place so every TSX conversion links to the exact same destination. Copied
// verbatim from src/content/*/body.html (the Chrome Web Store URL's
// `%E2%80%94` is an encoded em dash — do not "clean up" it).
export const GITHUB_REPO = 'https://github.com/saeedkolivand/ai-job-hunter-app';
export const CHROME_EXT =
  'https://chromewebstore.google.com/detail/ai-job-hunter-%E2%80%94-job-impor/oaoekkgkhmgdfnpmfkpphgiikliaicll';
export const FIREFOX_EXT = 'https://addons.mozilla.org/en-US/firefox/addon/ai-job-hunter/';
// The owner's share link carries `?hl=en-us&gl=DE&ocid=pdpshare` — `gl=DE`
// pins every visitor to the German storefront and `ocid` is share-tracking.
// Neither belongs in a link served to a global audience, so both are
// stripped; this is the canonical, query-param-free listing URL.
export const MS_STORE = 'https://apps.microsoft.com/detail/9nc5kdjv0btm';
export const SPONSOR = 'https://github.com/sponsors/saeedkolivand';
export const KOFI = 'https://ko-fi.com/saeedkolivand';
export const PAYPAL = 'https://paypal.me/saeedkolivand';
