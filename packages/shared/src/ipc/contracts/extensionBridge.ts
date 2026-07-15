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

/**
 * AI-answer-assist opt-in state (extension roadmap PR 9) — a SEPARATE opt-in
 * from {@link ExtensionAutofillSetting}: `answer.assist` is billable provider
 * spend (a materially different consent class from the local/free autofill
 * verbs), so it gets its own desktop-enforced gate, default OFF.
 *
 * `provider`/`model` are the pinned snapshot captured when the opt-in was
 * turned on (see `setAiAssistEnabled`) — present only while `enabled`, so a
 * Settings row can show which provider/model answer drafts will actually
 * use without a separate read. A stale value never lingers: `enabled: false`
 * always carries no snapshot (the desktop clears it on disable).
 */
export interface ExtensionAiAssistSetting {
  enabled: boolean;
  provider?: string;
  model?: string;
}

export interface ExtensionBridgeContract {
  status(): Promise<ExtensionBridgeStatus>;
  regenerateToken(): Promise<ExtensionBridgeTokenResult>;
  /** Read the assisted-autofill opt-in (default OFF). */
  autofillEnabled(): Promise<ExtensionAutofillSetting>;
  /** Set + persist the assisted-autofill opt-in; echoes the stored value. */
  setAutofillEnabled(enabled: boolean): Promise<ExtensionAutofillSetting>;
  /** Read the AI-answer-assist opt-in (default OFF). */
  aiAssistEnabled(): Promise<ExtensionAiAssistSetting>;
  /**
   * Set + persist the AI-answer-assist opt-in; echoes the stored value.
   * `provider`/`model`/`baseUrl` snapshot the renderer's CURRENT active AI
   * provider (see `useGenerateConfig`) at the moment the toggle is turned
   * on — mirrors `Autopilot.assistant_provider`'s pattern: the bridge is a
   * headless background context with no renderer to read the active
   * provider from at answer-time, so the toggle must snapshot it up front.
   * Turning the opt-in OFF clears the snapshot (re-enabling later re-snapshots
   * fresh, never replays a stale provider pick).
   */
  setAiAssistEnabled(
    enabled: boolean,
    provider?: string,
    model?: string,
    baseUrl?: string
  ): Promise<ExtensionAiAssistSetting>;
}

export const EXTENSION_BRIDGE_CHANNELS = {
  status: 'extensionBridge:status',
  regenerateToken: 'extensionBridge:regenerateToken',
  autofillEnabled: 'extensionBridge:autofillEnabled',
  setAutofillEnabled: 'extensionBridge:setAutofillEnabled',
  aiAssistEnabled: 'extensionBridge:aiAssistEnabled',
  setAiAssistEnabled: 'extensionBridge:setAiAssistEnabled',
} as const;
