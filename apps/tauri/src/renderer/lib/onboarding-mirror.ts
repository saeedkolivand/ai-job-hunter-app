import { Store } from '@tauri-apps/plugin-store';

// ponytail: localStorage stays primary; this disk mirror is a read-only
// fallback surviving a webview-data clear (e.g. factory reset / data wipe).
const STORE_FILE = 'flags.json';
const KEY = 'onboardingCompleted';

// security: this module deliberately uses a FIXED allowlist of keys (no generic
// set(key,value) helper) so the `store:default` grant can't be abused for
// arbitrary-key writes.

export async function markOnboardingComplete(): Promise<void> {
  try {
    const store = await Store.load(STORE_FILE);
    await store.set(KEY, true);
    await store.save();
  } catch (err) {
    console.warn('[onboarding-mirror] markOnboardingComplete failed (non-fatal):', err);
  }
}

export async function clearOnboardingMirror(): Promise<void> {
  try {
    const store = await Store.load(STORE_FILE);
    await store.delete(KEY);
    await store.save();
  } catch (err) {
    console.warn('[onboarding-mirror] clearOnboardingMirror failed (non-fatal):', err);
  }
}

export async function readOnboardingComplete(): Promise<boolean> {
  try {
    const store = await Store.load(STORE_FILE);
    return (await store.get<boolean>(KEY)) === true;
  } catch {
    return false;
  }
}
