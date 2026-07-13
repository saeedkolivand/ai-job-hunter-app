import { invoke } from '@tauri-apps/api/core';

import type {
  ExtensionAutofillSetting,
  ExtensionBridgeStatus,
  ExtensionBridgeTokenResult,
} from '@ajh/shared';

export const extensionBridge = {
  status: () => invoke<ExtensionBridgeStatus>('extension_bridge_status'),
  regenerateToken: () => invoke<ExtensionBridgeTokenResult>('extension_bridge_regenerate_token'),
  autofillEnabled: () => invoke<ExtensionAutofillSetting>('extension_bridge_autofill_enabled'),
  setAutofillEnabled: (enabled: boolean) =>
    invoke<ExtensionAutofillSetting>('extension_bridge_set_autofill_enabled', { enabled }),
};
