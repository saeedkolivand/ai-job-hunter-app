/**
 * Extension-bridge control capability (Feature 2).
 *
 * Read the local WebSocket bridge's status (bound port, whether an extension is
 * currently paired/connected, and the current pairing token) and regenerate the
 * pairing token. The bridge itself is a loopback WS server in
 * `apps/desktop/src-tauri/src/extension_bridge`; this namespace is only the
 * renderer's control surface over it (show the token in Settings, rotate it).
 */

/** Current bridge status. `port` is `null` when the server failed to bind. */
export interface ExtensionBridgeStatus {
  port: number | null;
  connected: boolean;
  token: string;
}

/** Result of rotating the pairing token (existing sockets must re-pair). */
export interface ExtensionBridgeTokenResult {
  token: string;
}

/**
 * Assisted-autofill opt-in state. When `enabled`, a `profile.get` from the
 * extension returns the user's contact profile so it can fill matching empty form
 * fields on the current page; when off, the desktop refuses. Default OFF.
 */
export interface ExtensionAutofillSetting {
  enabled: boolean;
}

export interface ExtensionBridgeContract {
  status(): Promise<ExtensionBridgeStatus>;
  regenerateToken(): Promise<ExtensionBridgeTokenResult>;
  /** Read the assisted-autofill opt-in (default OFF). */
  autofillEnabled(): Promise<ExtensionAutofillSetting>;
  /** Set + persist the assisted-autofill opt-in; echoes the stored value. */
  setAutofillEnabled(enabled: boolean): Promise<ExtensionAutofillSetting>;
}

export const EXTENSION_BRIDGE_CHANNELS = {
  status: 'extensionBridge:status',
  regenerateToken: 'extensionBridge:regenerateToken',
  autofillEnabled: 'extensionBridge:autofillEnabled',
  setAutofillEnabled: 'extensionBridge:setAutofillEnabled',
} as const;
