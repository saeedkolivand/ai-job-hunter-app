import { invoke } from '@tauri-apps/api/core';

import type { ExtensionBridgeStatus, ExtensionBridgeTokenResult } from '@ajh/shared';

export const extensionBridge = {
  status: () => invoke<ExtensionBridgeStatus>('extension_bridge_status'),
  regenerateToken: () => invoke<ExtensionBridgeTokenResult>('extension_bridge_regenerate_token'),
};
