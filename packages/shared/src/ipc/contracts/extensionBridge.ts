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
 * A bare boolean flag: no provider/model snapshot. A draft resolves the active
 * provider from the backend-owned active-provider store at answer-time (task
 * #16), so a Settings row reads that store (`aiActiveConfig`) for its
 * "Using: X · Y" label rather than a field echoed back here.
 */
export interface ExtensionAiAssistSetting {
  enabled: boolean;
}

/**
 * Auto-track opt-in state (Task #22, auto-track Layer A) — a SEPARATE opt-in
 * from {@link ExtensionAutofillSetting}/{@link ExtensionAiAssistSetting}. When
 * on, the extension arms a gesture submit-watcher that, on a detected form
 * submit, auto-marks the matched `saved` application `applied` (or nudges you
 * to import an untracked one). Default OFF, desktop-enforced: the desktop also
 * re-checks it before honoring an automated write, so a compromised extension
 * can't auto-mark applied without this consent.
 */
export interface ExtensionAutoTrackSetting {
  enabled: boolean;
}

/**
 * Pushed on a 0→1 or →0 transition in the live paired-connection COUNT (not a
 * per-socket event) — the desktop supports multiple browsers sharing one
 * pairing token, each with its own socket, so this only fires when the last
 * one disconnects or the first one (re)connects, never on an intermediate
 * pairing/close while at least one other socket stays open.
 */
export interface ExtensionBridgeChangedEvent {
  connected: boolean;
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
   * Set + persist the AI-answer-assist opt-in; echoes the stored value. A bare
   * boolean — the billable-AI consent gate. It no longer snapshots a provider:
   * a draft resolves the active provider from the backend active-provider store
   * at answer-time (task #16), so nothing more needs capturing here.
   */
  setAiAssistEnabled(enabled: boolean): Promise<ExtensionAiAssistSetting>;
  /** Read the auto-track opt-in (default OFF). */
  autoTrackEnabled(): Promise<ExtensionAutoTrackSetting>;
  /** Set + persist the auto-track opt-in; echoes the stored value. */
  setAutoTrackEnabled(enabled: boolean): Promise<ExtensionAutoTrackSetting>;
  /** Subscribe to a live connection-count transition (0→1 / →0). Returns a
   *  sync unsubscribe handle — mirrors `ApplicationsContract.onChanged`. */
  onChanged(handler: (event: ExtensionBridgeChangedEvent) => void): () => void;
}

export const EXTENSION_BRIDGE_CHANNELS = {
  status: 'extensionBridge:status',
  regenerateToken: 'extensionBridge:regenerateToken',
  autofillEnabled: 'extensionBridge:autofillEnabled',
  setAutofillEnabled: 'extensionBridge:setAutofillEnabled',
  aiAssistEnabled: 'extensionBridge:aiAssistEnabled',
  setAiAssistEnabled: 'extensionBridge:setAiAssistEnabled',
  autoTrackEnabled: 'extensionBridge:autoTrackEnabled',
  setAutoTrackEnabled: 'extensionBridge:setAutoTrackEnabled',
} as const;
