/**
 * Electron type extensions for APIs that exist but aren't in the official type definitions.
 */

declare namespace Electron {
  interface Session {
    /**
     * Sets the WebRTC IP handling policy.
     * This method exists in Electron but isn't in the official type definitions.
     */
    setWebRTCIPHandlingPolicy(
      policy:
        | 'default'
        | 'default_public_interface_only'
        | 'disable_non_proxied_udp'
        | 'disable_non_proxied_udp'
    ): void;
  }
}
