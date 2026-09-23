import { ExtensionBridgeSection } from '../ExtensionBridgeSection';

/**
 * Settings → Browser extension. The extension's pairing, permissions and
 * per-feature opt-ins grew past what a sub-section of Accounts could carry
 * (issue #1213), so they get their own nav entry.
 *
 * The anchor id is deliberately unchanged from when this lived under Accounts:
 * `search-index.ts` and the tray's deep link both address it by that id, and
 * renaming it would break links the app already hands out.
 */
export function ExtensionSettingsTab() {
  return (
    <div className="space-y-3">
      <div data-settings-anchor="accounts-extension">
        <ExtensionBridgeSection />
      </div>
    </div>
  );
}
