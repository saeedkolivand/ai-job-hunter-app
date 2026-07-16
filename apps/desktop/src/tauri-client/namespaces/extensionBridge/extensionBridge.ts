import { invoke } from '@tauri-apps/api/core';

import type {
  ExtensionAiAssistSetting,
  ExtensionAutofillSetting,
  ExtensionAutoTrackSetting,
  ExtensionBridgeStatus,
  ExtensionBridgeTokenResult,
} from '@ajh/shared';

export const extensionBridge = {
  status: () => invoke<ExtensionBridgeStatus>('extension_bridge_status'),
  regenerateToken: () => invoke<ExtensionBridgeTokenResult>('extension_bridge_regenerate_token'),
  autofillEnabled: () => invoke<ExtensionAutofillSetting>('extension_bridge_autofill_enabled'),
  setAutofillEnabled: (enabled: boolean) =>
    invoke<ExtensionAutofillSetting>('extension_bridge_set_autofill_enabled', { enabled }),
  aiAssistEnabled: () => invoke<ExtensionAiAssistSetting>('extension_bridge_ai_assist_enabled'),
  setAiAssistEnabled: (enabled: boolean) =>
    invoke<ExtensionAiAssistSetting>('extension_bridge_set_ai_assist_enabled', { enabled }),
  autoTrackEnabled: () => invoke<ExtensionAutoTrackSetting>('extension_bridge_auto_track_enabled'),
  setAutoTrackEnabled: (enabled: boolean) =>
    invoke<ExtensionAutoTrackSetting>('extension_bridge_set_auto_track_enabled', { enabled }),
};
