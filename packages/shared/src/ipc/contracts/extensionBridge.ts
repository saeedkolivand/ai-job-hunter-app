/**
 * Extension-bridge control capability (Feature 2).
 *
 * Read the local WebSocket bridge's status (bound port, whether an extension is
 * currently paired/connected, and the current pairing token) and regenerate the
 * pairing token. The bridge itself is a loopback WS server in
 * `apps/tauri/src-tauri/src/extension_bridge`; this namespace is only the
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

export interface ExtensionBridgeContract {
  status(): Promise<ExtensionBridgeStatus>;
  regenerateToken(): Promise<ExtensionBridgeTokenResult>;
}

export const EXTENSION_BRIDGE_CHANNELS = {
  status: 'extensionBridge:status',
  regenerateToken: 'extensionBridge:regenerateToken',
} as const;
